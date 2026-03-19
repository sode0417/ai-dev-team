mod auth;
mod config;
mod db;
mod domains;
mod error;
mod executor;
pub mod github;
mod response;
mod scanner;
mod ws;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::{Json, Router};
use config::Config;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::net::SocketAddr;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use uuid::Uuid;
use github::GitHubClient;
use ws::WsHub;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub ws_hub: WsHub,
    pub github: GitHubClient,
}

async fn health_check() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn dashboard(State(state): State<AppState>) -> Result<Json<Value>, error::AppError> {
    let total_projects: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&state.pool)
            .await?;

    let total_tasks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
            .fetch_one(&state.pool)
            .await?;

    let active_tasks: i64 =
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE status IN ('hearing', 'planning', 'awaiting_approval', 'executing', 'reviewing')",
        )
        .fetch_one(&state.pool)
        .await?;

    let completed_tasks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE status = 'completed'")
            .fetch_one(&state.pool)
            .await?;

    let failed_tasks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE status = 'failed'")
            .fetch_one(&state.pool)
            .await?;

    let recent_tasks: Vec<domains::tasks::model::Task> = sqlx::query_as(
        "SELECT id, project_id, repository_id, title, description, status, priority, \
         depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
         retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, \
         scan_id, proposal_type, sprint_id, revision_count \
         FROM tasks ORDER BY updated_at DESC LIMIT 10",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(json!({
        "data": {
            "total_projects": total_projects,
            "total_tasks": total_tasks,
            "active_tasks": active_tasks,
            "completed_tasks": completed_tasks,
            "failed_tasks": failed_tasks,
            "recent_tasks": recent_tasks,
        }
    })))
}

#[derive(serde::Deserialize)]
struct WsQuery {
    token: Option<String>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<WsQuery>,
) -> Result<impl IntoResponse, error::AppError> {
    // WebSocket 接続時に JWT を検証（認証有効時のみ）
    // NOTE: トークンを URL パラメータで送信するため、サーバーログにトークンが記録されないよう注意
    if state.config.auth_enabled {
        let token = query
            .token
            .ok_or_else(|| error::AppError::Unauthorized("Missing token parameter".to_string()))?;
        auth::decode_token(&token, &state.config.jwt_secret)?;
    }

    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state, id)))
}

async fn handle_ws(mut socket: WebSocket, state: AppState, id: Uuid) {
    let mut rx = state.ws_hub.subscribe(id).await;

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

/// PRマージ済みタスクのワークツリーを自動クリーンアップ
async fn check_merged_prs(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // completed で pr_url があるタスクを取得
    let tasks: Vec<domains::tasks::model::Task> = sqlx::query_as(
        "SELECT id, project_id, repository_id, title, description, status, priority, \
         depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
         retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, \
         scan_id, proposal_type, sprint_id, issue_number, issue_url, revision_count \
         FROM tasks WHERE status IN ('completed', 'failed') AND pr_url IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    for task in tasks {
        let pr_url = match &task.pr_url {
            Some(url) => url.clone(),
            None => continue,
        };

        // 最新セッションの worktree_path を確認
        let sessions = domains::executions::service::list_sessions(pool, task.id).await.unwrap_or_default();
        let session = match sessions.first() {
            Some(s) => s,
            None => continue,
        };

        let wt_path = match &session.worktree_path {
            Some(p) => p.clone(),
            None => continue, // 既にクリーンアップ済み
        };

        // PRがマージまたはクローズされたか確認
        match executor::worktree::check_pr_closed_or_merged(&pr_url).await {
            Ok(true) => {
                tracing::info!("PR closed/merged for task {}: {pr_url} — cleaning up worktree", task.id);
                let worktree_dir = std::path::PathBuf::from(&wt_path);

                // リポジトリのパスを取得（worktree の親の親）
                if let Some(repo_path) = worktree_dir.parent().and_then(|p| p.parent()) {
                    let _ = executor::worktree::cleanup_worktree(
                        repo_path.to_str().unwrap_or_default(),
                        &worktree_dir,
                    ).await;
                }

                // worktree_path を NULL にクリア
                let _ = domains::executions::service::clear_worktree_path(pool, session.id).await;
            }
            Ok(false) => {} // まだマージされていない
            Err(e) => {
                tracing::debug!("Failed to check PR state for {pr_url}: {e}");
            }
        }
    }

    Ok(())
}

fn cors_layer(config: &Config) -> CorsLayer {
    use axum::http::{HeaderValue, Method};

    let origins: Vec<HeaderValue> = config
        .allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers(tower_http::cors::Any)
        .allow_origin(AllowOrigin::list(origins))
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ai_dev_team=info,tower_http=info".parse().unwrap()),
        )
        .init();

    let config = Config::from_env();
    let pool = db::create_pool(&config.database_url).await;
    let ws_hub = WsHub::new();
    let github = GitHubClient::new(config.github_token.clone());

    let state = AppState {
        pool,
        config: config.clone(),
        ws_hub,
        github,
    };

    // 期限切れリフレッシュトークンの定期クリーンアップ（1時間ごと）
    if config.auth_enabled {
        let cleanup_pool = state.pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                match sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < NOW()")
                    .execute(&cleanup_pool)
                    .await
                {
                    Ok(result) => {
                        let count = result.rows_affected();
                        if count > 0 {
                            tracing::info!("Cleaned up {count} expired refresh tokens");
                        }
                    }
                    Err(e) => tracing::warn!("Failed to cleanup expired refresh tokens: {e}"),
                }
            }
        });
    }

    // PRマージ自動検知バックグラウンドタスク（5分間隔）
    {
        let pool = state.pool.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                if let Err(e) = check_merged_prs(&pool).await {
                    tracing::warn!("PR merge check failed: {e}");
                }
            }
        });
    }

    let app = Router::new()
        // 認証不要ルート
        .route("/api/health", axum::routing::get(health_check))
        .nest("/api/auth", domains::auth::handler::public_routes())
        // 認証必要ルート
        .nest("/api/auth", domains::auth::handler::protected_routes())
        .route("/api/dashboard", axum::routing::get(dashboard))
        .nest("/api/projects", domains::projects::handler::routes())
        .merge(Router::new().nest("/api/projects", domains::scans::handler::project_routes()))
        .nest("/api/scans", domains::scans::handler::scan_routes())
        .merge(Router::new().nest("/api/projects", domains::sprints::handler::project_routes()))
        .nest("/api/sprints", domains::sprints::handler::sprint_routes())
        .nest("/api/tasks", domains::tasks::handler::routes())
        .merge(
            Router::new()
                .nest("/api/tasks", domains::executions::handler::task_routes())
                .nest("/api/executions", domains::executions::handler::execution_routes()),
        )
        // WebSocket（認証はハンドラ内で処理）
        .route("/ws/executions/{task_id}", axum::routing::get(ws_handler))
        .route("/ws/scans/{scan_id}", axum::routing::get(ws_handler))
        .route("/ws/sprints/{sprint_id}", axum::routing::get(ws_handler))
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer(&config))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
