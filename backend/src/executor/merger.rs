use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use tokio::process::Command;

use crate::domains::tasks::model::Task;
use crate::domains::tasks::service as task_service;
use crate::executor::claude_cli;
use crate::executor::worktree;
use crate::ws::WsHub;

const CONFLICT_CHECK_INTERVAL_SECS: u64 = 300; // 5分

/// コンフリクト監視の定期ループを開始
/// GitHub Auto-merge が有効な場合、マージ自体は GitHub が行う。
/// このループはコンフリクト（CONFLICTING）で止まった PR を検出し、
/// Claude Code で自動修復を試みる。
pub async fn start_conflict_watch_loop(pool: PgPool, ws_hub: WsHub) {
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(CONFLICT_CHECK_INTERVAL_SECS));

    loop {
        interval.tick().await;

        let tasks = match task_service::list_mergeable_tasks(&pool).await {
            Ok(tasks) => tasks,
            Err(e) => {
                tracing::error!("コンフリクト監視: タスク取得失敗: {e}");
                continue;
            }
        };

        if tasks.is_empty() {
            tracing::debug!("コンフリクト監視: 監視対象なし");
            continue;
        }

        tracing::info!("コンフリクト監視: {} 件のPRをチェック", tasks.len());

        for task in tasks {
            if let Err(e) = process_pr(&pool, &ws_hub, &task).await {
                tracing::error!(
                    "コンフリクト監視: タスク {} ({}) の処理失敗: {e}",
                    task.id,
                    task.title
                );
            }
        }
    }
}

/// PR の状態をチェックし、コンフリクトがあれば自動修復を試みる
/// マージ自体は GitHub Auto-merge が行うため、このループではマージしない。
async fn process_pr(pool: &PgPool, ws_hub: &WsHub, task: &Task) -> Result<(), String> {
    let pr_url = task
        .pr_url
        .as_deref()
        .ok_or("pr_url is None")?;

    // PR番号を取得
    let pr_number = extract_pr_number(pr_url)
        .ok_or_else(|| format!("PR URLからPR番号を抽出できません: {pr_url}"))?;

    // リポジトリ情報を取得（owner/name 形式）
    let repo_nwo = extract_repo_nwo(pr_url)
        .ok_or_else(|| format!("PR URLからリポジトリ情報を抽出できません: {pr_url}"))?;

    task_service::add_merge_log(pool, task.id, "check", true, Some("PR状態チェック開始"))
        .await
        .map_err(|e| format!("ログ記録失敗: {e}"))?;

    // gh pr view で PR 状態を取得
    let pr_info = get_pr_info(&repo_nwo, pr_number).await?;

    match pr_info.state.as_str() {
        "MERGED" => {
            // 既にマージ済み（GitHub Auto-merge によるマージ完了）
            task_service::update_merge_status(pool, task.id, "merged")
                .await
                .map_err(|e| format!("状態更新失敗: {e}"))?;
            task_service::add_merge_log(pool, task.id, "check", true, Some("GitHub Auto-mergeでマージ済み"))
                .await
                .ok();
            notify_merge_event(ws_hub, task, "merged", "PRはGitHub Auto-mergeでマージされました");
            return Ok(());
        }
        "CLOSED" => {
            task_service::update_merge_status(pool, task.id, "failed")
                .await
                .map_err(|e| format!("状態更新失敗: {e}"))?;
            task_service::add_merge_log(pool, task.id, "check", false, Some("PRはクローズされています"))
                .await
                .ok();
            notify_merge_event(ws_hub, task, "failed", "PRがクローズされています");
            return Ok(());
        }
        _ => {} // OPEN — コンフリクトチェック続行
    }

    // コンフリクトチェック（GitHub Auto-merge はコンフリクトがあると止まる）
    if pr_info.mergeable == "CONFLICTING" {
        tracing::info!(
            "コンフリクト監視: PR #{pr_number} ({}) にコンフリクト、修復を試みます",
            task.title
        );
        task_service::update_merge_status(pool, task.id, "conflict")
            .await
            .map_err(|e| format!("状態更新失敗: {e}"))?;
        notify_merge_event(ws_hub, task, "conflict", "コンフリクトを検出、自動修復を試みています");

        return resolve_conflict(pool, ws_hub, task, &pr_info.head_ref).await;
    }

    // MERGEABLE or UNKNOWN → GitHub Auto-merge に任せる
    tracing::debug!(
        "コンフリクト監視: PR #{pr_number} ({}) は正常 (mergeable={})",
        task.title,
        pr_info.mergeable
    );
    Ok(())
}

/// コンフリクトを Claude Code で解消
async fn resolve_conflict(
    pool: &PgPool,
    ws_hub: &WsHub,
    task: &Task,
    head_ref: &str,
) -> Result<(), String> {
    task_service::add_merge_log(
        pool,
        task.id,
        "resolve_conflict",
        true,
        Some("コンフリクト解消を開始"),
    )
    .await
    .ok();

    // リポジトリのローカルパスを取得
    let repo_path = get_repo_local_path(pool, task).await?;

    // worktree を作成してコンフリクト解消
    let worktree_dir = std::path::Path::new(&repo_path)
        .join(".worktrees")
        .join(format!("merge-{}", task.id));
    let worktree_path_str = worktree_dir.to_string_lossy().to_string();

    // 既存の worktree があればクリーンアップ
    if worktree_dir.exists() {
        worktree::cleanup_worktree(&repo_path, &worktree_dir).await.ok();
    }

    // worktree を PR ブランチからチェックアウト
    let fetch_output = Command::new("git")
        .args(["fetch", "origin", head_ref])
        .current_dir(&repo_path)
        .output()
        .await
        .map_err(|e| format!("git fetch 失敗: {e}"))?;

    if !fetch_output.status.success() {
        let msg = format!(
            "git fetch 失敗: {}",
            String::from_utf8_lossy(&fetch_output.stderr)
        );
        fail_merge(pool, ws_hub, task, &msg).await;
        return Err(msg);
    }

    // worktree を PR ブランチで作成
    tokio::fs::create_dir_all(worktree_dir.parent().unwrap())
        .await
        .map_err(|e| format!("ディレクトリ作成失敗: {e}"))?;

    let wt_output = Command::new("git")
        .args([
            "worktree",
            "add",
            worktree_dir.to_str().unwrap(),
            &format!("origin/{head_ref}"),
            "--detach",
        ])
        .current_dir(&repo_path)
        .output()
        .await
        .map_err(|e| format!("git worktree add 失敗: {e}"))?;

    if !wt_output.status.success() {
        let msg = format!(
            "worktree 作成失敗: {}",
            String::from_utf8_lossy(&wt_output.stderr)
        );
        fail_merge(pool, ws_hub, task, &msg).await;
        return Err(msg);
    }

    // PR ブランチにチェックアウト
    let checkout = Command::new("git")
        .args(["checkout", head_ref])
        .current_dir(&worktree_path_str)
        .output()
        .await
        .map_err(|e| format!("git checkout 失敗: {e}"))?;

    if !checkout.status.success() {
        // detached HEAD のまま新しいブランチとして設定
        let checkout_b = Command::new("git")
            .args(["checkout", "-b", head_ref])
            .current_dir(&worktree_path_str)
            .output()
            .await
            .map_err(|e| format!("git checkout -b 失敗: {e}"))?;

        if !checkout_b.status.success() {
            let msg = format!(
                "ブランチチェックアウト失敗: {}",
                String::from_utf8_lossy(&checkout_b.stderr)
            );
            cleanup_and_fail(pool, ws_hub, task, &repo_path, &worktree_dir, &msg).await;
            return Err(msg);
        }
    }

    // main をマージ（コンフリクトマーカー生成）
    let merge = Command::new("git")
        .args(["merge", "origin/main", "--no-edit"])
        .current_dir(&worktree_path_str)
        .output()
        .await
        .map_err(|e| format!("git merge 失敗: {e}"))?;

    // マージが正常完了した場合はコンフリクトなし（既に解消済み）
    if merge.status.success() {
        // push して再チェック
        let push = Command::new("git")
            .args(["push", "origin", head_ref])
            .current_dir(&worktree_path_str)
            .output()
            .await
            .map_err(|e| format!("git push 失敗: {e}"))?;

        if push.status.success() {
            worktree::cleanup_worktree(&repo_path, &worktree_dir).await.ok();
            // コンフリクト解消済み → pending に戻して GitHub Auto-merge に任せる
            task_service::update_merge_status(pool, task.id, "pending")
                .await
                .map_err(|e| format!("状態更新失敗: {e}"))?;
            task_service::add_merge_log(pool, task.id, "resolve_conflict", true, Some("コンフリクト解消済み、Auto-mergeに委譲"))
                .await
                .ok();
            notify_merge_event(ws_hub, task, "conflict_resolved", "コンフリクトを解消しました。GitHub Auto-mergeでマージされます。");
            return Ok(());
        }
    }

    // コンフリクトあり → Claude Code で解消
    let prompt = "このブランチには main とのマージコンフリクトがあります。\n\
        コンフリクトマーカー（<<<<<<<, =======, >>>>>>>）を解消してください。\n\
        既存の機能を壊さないよう注意してください。\n\
        `git diff` でコンフリクト箇所を確認し、適切に解消してください。\n\
        解消後、`git add` でステージングしてください。";

    let claude_result =
        claude_cli::run_claude_autonomous(prompt, &worktree_path_str, 600).await;

    match claude_result {
        Ok(result) if result.exit_code == 0 => {
            // コミット＋プッシュ
            let commit = Command::new("git")
                .args(["commit", "-m", "コンフリクト解消"])
                .current_dir(&worktree_path_str)
                .output()
                .await;

            let push = Command::new("git")
                .args(["push", "origin", head_ref])
                .current_dir(&worktree_path_str)
                .output()
                .await;

            worktree::cleanup_worktree(&repo_path, &worktree_dir).await.ok();

            match (commit, push) {
                (Ok(c), Ok(p)) if c.status.success() && p.status.success() => {
                    task_service::add_merge_log(
                        pool,
                        task.id,
                        "resolve_conflict",
                        true,
                        Some("コンフリクト解消成功、マージ再試行"),
                    )
                    .await
                    .ok();

                    // CI が再度通るまで待つ必要があるので、pending に戻す
                    task_service::update_merge_status(pool, task.id, "pending")
                        .await
                        .map_err(|e| format!("状態更新失敗: {e}"))?;
                    notify_merge_event(
                        ws_hub,
                        task,
                        "conflict_resolved",
                        "コンフリクトを解消しました。CIの完了を待ってマージします。",
                    );
                    Ok(())
                }
                _ => {
                    let msg = "コンフリクト解消後の commit/push に失敗";
                    fail_merge(pool, ws_hub, task, msg).await;
                    Err(msg.to_string())
                }
            }
        }
        Ok(result) => {
            worktree::cleanup_worktree(&repo_path, &worktree_dir).await.ok();
            let msg = format!(
                "Claude Code によるコンフリクト解消失敗 (exit={}): {}",
                result.exit_code,
                result.stderr.chars().take(500).collect::<String>()
            );
            fail_merge(pool, ws_hub, task, &msg).await;
            Err(msg)
        }
        Err(e) => {
            worktree::cleanup_worktree(&repo_path, &worktree_dir).await.ok();
            let msg = format!("Claude Code 実行エラー: {e}");
            fail_merge(pool, ws_hub, task, &msg).await;
            Err(msg)
        }
    }
}

/// マージ失敗の共通処理
async fn fail_merge(pool: &PgPool, ws_hub: &WsHub, task: &Task, msg: &str) {
    task_service::update_merge_status(pool, task.id, "failed").await.ok();
    task_service::add_merge_log(pool, task.id, "resolve_conflict", false, Some(msg))
        .await
        .ok();
    notify_merge_event(ws_hub, task, "failed", msg);
}

/// worktree クリーンアップ + マージ失敗
async fn cleanup_and_fail(
    pool: &PgPool,
    ws_hub: &WsHub,
    task: &Task,
    repo_path: &str,
    worktree_dir: &std::path::Path,
    msg: &str,
) {
    worktree::cleanup_worktree(repo_path, worktree_dir).await.ok();
    fail_merge(pool, ws_hub, task, msg).await;
}

/// リポジトリのローカルパスを取得
async fn get_repo_local_path(pool: &PgPool, task: &Task) -> Result<String, String> {
    let repo_id = task
        .repository_id
        .ok_or("repository_id が未設定")?;

    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT local_path FROM project_repositories WHERE id = $1",
    )
    .bind(repo_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("リポジトリ情報取得失敗: {e}"))?;

    row.and_then(|(path,)| path)
        .ok_or_else(|| "リポジトリの local_path が未設定".to_string())
}

/// WebSocket でマージイベントを通知
fn notify_merge_event(ws_hub: &WsHub, task: &Task, event: &str, message: &str) {
    let payload = json!({
        "type": "merge_event",
        "task_id": task.id,
        "task_title": task.title,
        "event": event,
        "message": message,
    });
    ws_hub.broadcast_global(&payload.to_string());
}

/// PR URL から PR 番号を抽出
fn extract_pr_number(pr_url: &str) -> Option<u64> {
    // https://github.com/owner/repo/pull/123
    pr_url.rsplit('/').next()?.parse().ok()
}

/// PR URL からリポジトリ (owner/name) を抽出
fn extract_repo_nwo(pr_url: &str) -> Option<String> {
    // https://github.com/owner/repo/pull/123
    let parts: Vec<&str> = pr_url.trim_end_matches('/').split('/').collect();
    if parts.len() >= 5 {
        let owner = parts[parts.len() - 4];
        let repo = parts[parts.len() - 3];
        Some(format!("{owner}/{repo}"))
    } else {
        None
    }
}

#[derive(Debug)]
struct PrInfo {
    state: String,
    mergeable: String,
    #[allow(dead_code)] // CI ステータスはログ・将来の拡張用に保持
    ci_status: String,
    head_ref: String,
}

#[derive(Debug, Deserialize)]
struct GhPrView {
    state: String,
    mergeable: Option<String>,
    #[serde(rename = "statusCheckRollup")]
    status_check_rollup: Option<Vec<GhCheckRun>>,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
}

#[derive(Debug, Deserialize)]
struct GhCheckRun {
    conclusion: Option<String>,
    status: Option<String>,
}

/// gh pr view で PR 情報を取得
async fn get_pr_info(repo_nwo: &str, pr_number: u64) -> Result<PrInfo, String> {
    let output = Command::new("gh")
        .args([
            "pr",
            "view",
            &pr_number.to_string(),
            "--json",
            "state,mergeable,statusCheckRollup,headRefName",
            "--repo",
            repo_nwo,
        ])
        .output()
        .await
        .map_err(|e| format!("gh pr view 実行失敗: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "gh pr view 失敗: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let pr: GhPrView = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("PR情報のパース失敗: {e}"))?;

    // CI ステータスの集約
    let ci_status = aggregate_ci_status(&pr.status_check_rollup);

    Ok(PrInfo {
        state: pr.state,
        mergeable: pr.mergeable.unwrap_or_else(|| "UNKNOWN".to_string()),
        ci_status,
        head_ref: pr.head_ref_name,
    })
}

/// CI チェックの結果を集約
fn aggregate_ci_status(checks: &Option<Vec<GhCheckRun>>) -> String {
    let Some(checks) = checks else {
        return "SUCCESS".to_string(); // チェックなし = 成功扱い
    };

    if checks.is_empty() {
        return "SUCCESS".to_string();
    }

    let mut has_pending = false;

    for check in checks {
        if let Some(status) = &check.status {
            if status != "COMPLETED" {
                has_pending = true;
                continue;
            }
        }

        if let Some(conclusion) = &check.conclusion {
            match conclusion.as_str() {
                "FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE" => return "FAILURE".to_string(),
                "ACTION_REQUIRED" => return "FAILURE".to_string(),
                _ => {} // SUCCESS, NEUTRAL, SKIPPED, etc.
            }
        }
    }

    if has_pending {
        "PENDING".to_string()
    } else {
        "SUCCESS".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pr_number() {
        assert_eq!(
            extract_pr_number("https://github.com/sode0417/ai-dev-team/pull/42"),
            Some(42)
        );
        assert_eq!(
            extract_pr_number("https://github.com/owner/repo/pull/1"),
            Some(1)
        );
        assert_eq!(extract_pr_number("invalid-url"), None);
    }

    #[test]
    fn test_extract_repo_nwo() {
        assert_eq!(
            extract_repo_nwo("https://github.com/sode0417/ai-dev-team/pull/42"),
            Some("sode0417/ai-dev-team".to_string())
        );
        assert_eq!(
            extract_repo_nwo("https://github.com/owner/repo/pull/1"),
            Some("owner/repo".to_string())
        );
        assert_eq!(extract_repo_nwo("short"), None);
    }

    #[test]
    fn test_aggregate_ci_status_no_checks() {
        assert_eq!(aggregate_ci_status(&None), "SUCCESS");
        assert_eq!(aggregate_ci_status(&Some(vec![])), "SUCCESS");
    }

    #[test]
    fn test_aggregate_ci_status_all_success() {
        let checks = vec![
            GhCheckRun {
                conclusion: Some("SUCCESS".to_string()),
                status: Some("COMPLETED".to_string()),
            },
            GhCheckRun {
                conclusion: Some("SUCCESS".to_string()),
                status: Some("COMPLETED".to_string()),
            },
        ];
        assert_eq!(aggregate_ci_status(&Some(checks)), "SUCCESS");
    }

    #[test]
    fn test_aggregate_ci_status_has_failure() {
        let checks = vec![
            GhCheckRun {
                conclusion: Some("SUCCESS".to_string()),
                status: Some("COMPLETED".to_string()),
            },
            GhCheckRun {
                conclusion: Some("FAILURE".to_string()),
                status: Some("COMPLETED".to_string()),
            },
        ];
        assert_eq!(aggregate_ci_status(&Some(checks)), "FAILURE");
    }

    #[test]
    fn test_aggregate_ci_status_pending() {
        let checks = vec![
            GhCheckRun {
                conclusion: Some("SUCCESS".to_string()),
                status: Some("COMPLETED".to_string()),
            },
            GhCheckRun {
                conclusion: None,
                status: Some("IN_PROGRESS".to_string()),
            },
        ];
        assert_eq!(aggregate_ci_status(&Some(checks)), "PENDING");
    }
}
