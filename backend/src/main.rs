mod auth;
mod config;
mod db;
mod domains;
mod error;
mod executor;
pub mod github;
mod response;
mod ws;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
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
            "SELECT COUNT(*) FROM tasks WHERE status IN ('planning', 'executing', 'reviewing')",
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
         depends_on, execution_order, proposed_by, plan, pr_url, changed_files, diff_stats, \
         retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at \
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

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state, task_id))
}

async fn handle_ws(mut socket: WebSocket, state: AppState, task_id: Uuid) {
    let mut rx = state.ws_hub.subscribe(task_id).await;

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

fn cors_layer() -> CorsLayer {
    use axum::http::{HeaderValue, Method};

    CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers(tower_http::cors::Any)
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            let origin = origin.to_str().unwrap_or_default();
            origin.starts_with("http://localhost:") || origin.contains("sode-ai.com")
        }))
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

    let app = Router::new()
        .route("/api/health", axum::routing::get(health_check))
        .route("/api/dashboard", axum::routing::get(dashboard))
        .nest("/api/projects", domains::projects::handler::routes())
        .nest("/api/tasks", domains::tasks::handler::routes())
        .merge(
            Router::new()
                .nest("/api/tasks", domains::executions::handler::task_routes())
                .nest("/api/executions", domains::executions::handler::execution_routes()),
        )
        .route("/ws/executions/{task_id}", axum::routing::get(ws_handler))
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
