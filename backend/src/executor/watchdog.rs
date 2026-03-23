use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domains::sprints::service as sprint_service;
use crate::ws::WsHub;

const WATCHDOG_INTERVAL_SECS: u64 = 300; // 5分
const MAX_TASK_DURATION_SECS: i64 = 5400; // 90分

/// サーバー起動時に1回だけ呼ばれる。
/// 前回のプロセス終了で中断されたタスク/セッションを failed に復旧する。
pub async fn recover_stuck_on_startup(pool: &PgPool, ws_hub: &WsHub) {
    tracing::info!("Watchdog: startup recovery 開始");

    match recover_stuck_tasks(pool, ws_hub, None).await {
        Ok(count) => {
            if count > 0 {
                tracing::warn!("Watchdog: startup recovery — {count} 件のタスクを failed に更新");
            } else {
                tracing::info!("Watchdog: startup recovery — 復旧対象なし");
            }
        }
        Err(e) => {
            tracing::error!("Watchdog: startup recovery 失敗: {e}");
        }
    }
}

/// 定期的にスタックしたタスクを検出して failed にするループ
pub async fn start_watchdog_loop(pool: PgPool, ws_hub: WsHub) {
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(WATCHDOG_INTERVAL_SECS));

    loop {
        interval.tick().await;

        match recover_stuck_tasks(&pool, &ws_hub, Some(MAX_TASK_DURATION_SECS)).await {
            Ok(count) => {
                if count > 0 {
                    tracing::warn!(
                        "Watchdog: {count} 件のスタックタスクを failed に更新（{MAX_TASK_DURATION_SECS}秒超過）"
                    );
                } else {
                    tracing::debug!("Watchdog: スタックタスクなし");
                }
            }
            Err(e) => {
                tracing::error!("Watchdog: スタックタスク検出失敗: {e}");
            }
        }
    }
}

/// スタックしたタスクを検出して failed に更新する共通ロジック。
/// `min_age_secs` が None の場合は全ての対象タスクを処理（startup recovery 用）。
/// Some の場合は started_at が指定秒数以上前のタスクのみ対象。
async fn recover_stuck_tasks(
    pool: &PgPool,
    ws_hub: &WsHub,
    min_age_secs: Option<i64>,
) -> Result<usize, String> {
    // planning, executing, reviewing 状態のタスクを検索
    let stuck_tasks: Vec<(Uuid, String, String, Option<Uuid>)> = if let Some(age) = min_age_secs {
        sqlx::query_as(
            "SELECT id, title, status::TEXT, sprint_id FROM tasks \
             WHERE status IN ('planning', 'executing', 'reviewing') \
             AND started_at < NOW() - make_interval(secs => $1)",
        )
        .bind(age as f64)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("タスク検索失敗: {e}"))?
    } else {
        sqlx::query_as(
            "SELECT id, title, status::TEXT, sprint_id FROM tasks \
             WHERE status IN ('planning', 'executing', 'reviewing')",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("タスク検索失敗: {e}"))?
    };

    if stuck_tasks.is_empty() {
        return Ok(0);
    }

    let count = stuck_tasks.len();
    let mut affected_sprint_ids: Vec<Uuid> = Vec::new();

    for (task_id, title, status, sprint_id) in &stuck_tasks {
        tracing::warn!(
            "Watchdog: タスク {} ({}) を {} → failed に更新",
            task_id,
            title,
            status
        );

        // タスクを failed に更新
        let result = sqlx::query(
            "UPDATE tasks SET status = 'failed', \
             error_log = COALESCE(error_log || E'\\n', '') || $2, \
             completed_at = NOW(), updated_at = NOW() \
             WHERE id = $1 AND status IN ('planning', 'executing', 'reviewing')",
        )
        .bind(task_id)
        .bind("Watchdog: サーバー再起動またはタイムアウトにより強制終了")
        .execute(pool)
        .await;

        if let Err(e) = result {
            tracing::error!("Watchdog: タスク {} の更新失敗: {e}", task_id);
            continue;
        }

        // 対応する実行中セッションも failed に更新
        let session_result = sqlx::query(
            "UPDATE execution_sessions SET status = 'failed', completed_at = NOW() \
             WHERE task_id = $1 AND status = 'running' AND completed_at IS NULL",
        )
        .bind(task_id)
        .execute(pool)
        .await;

        if let Err(e) = session_result {
            tracing::error!("Watchdog: タスク {} のセッション更新失敗: {e}", task_id);
        }

        // WebSocket 通知
        let payload = json!({
            "type": "watchdog_recovery",
            "task_id": task_id,
            "task_title": title,
            "message": "サーバー再起動またはタイムアウトにより強制終了されました",
        });
        ws_hub.broadcast_global(&payload.to_string());

        if let Some(sid) = sprint_id {
            if !affected_sprint_ids.contains(sid) {
                affected_sprint_ids.push(*sid);
            }
        }
    }

    // 影響を受けたスプリントの自動遷移チェック
    for sprint_id in affected_sprint_ids {
        check_sprint_transition_after_recovery(pool, ws_hub, sprint_id).await;
    }

    Ok(count)
}

/// watchdog でタスクを failed にした後、スプリントが executing かつ全タスク terminal なら
/// 振り返りを自動トリガー
async fn check_sprint_transition_after_recovery(pool: &PgPool, ws_hub: &WsHub, sprint_id: Uuid) {
    let sprint = match sprint_service::get_sprint(pool, sprint_id).await {
        Ok(s) => s,
        Err(_) => return,
    };

    if sprint.status != "executing" {
        return;
    }

    let all_terminal = sprint_service::all_tasks_terminal(pool, sprint_id)
        .await
        .unwrap_or(false);

    if !all_terminal {
        return;
    }

    tracing::info!(
        "Watchdog: Sprint {sprint_id} の全タスクが終了状態 — 振り返りを自動トリガー"
    );

    let pool = pool.clone();
    let ws_hub = ws_hub.clone();
    tokio::spawn(async move {
        let sprint_ws_msg = json!({
            "sprint_id": sprint_id.to_string(),
            "phase": "generating_retro",
            "message": "Watchdog: 全タスク終了 — 振り返りを自動生成中...",
        });
        ws_hub
            .broadcast(sprint_id, &sprint_ws_msg.to_string())
            .await;

        crate::scanner::analyzer::run_sprint_retrospective_only(&pool, &ws_hub, sprint_id).await;
    });
}
