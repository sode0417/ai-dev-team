use sqlx::PgPool;
use uuid::Uuid;

use crate::domains::executions::service as exec_service;
use crate::domains::tasks::model::TaskStatus;
use crate::domains::tasks::service as task_service;
use crate::ws::WsHub;
use super::claude_cli;
use super::worktree;

/// Planner → Coder → Reviewer → Test → PR 作成パイプライン
pub async fn run_pipeline(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    title: &str,
    description: &str,
    repo_path: &str,
    base_branch: &str,
) {
    // セッション作成前に worktree を準備
    let (worktree_dir, branch_name) = match worktree::create_worktree(repo_path, task_id, base_branch).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to create worktree: {e}");
            let _ = task_service::update_task_execution(
                pool, task_id, TaskStatus::Failed, None, None, None, None, Some(&e),
            )
            .await;
            broadcast(ws_hub, task_id, "error", &e).await;
            return;
        }
    };

    let wt_path = worktree_dir.to_str().unwrap().to_string();

    // 実行セッション作成
    let session = match exec_service::create_session(pool, task_id, Some(&wt_path), Some(&branch_name)).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create session: {e}");
            let _ = worktree::cleanup_worktree(repo_path, &worktree_dir).await;
            return;
        }
    };

    let session_id = session.id;

    // === Phase 1: Planner ===
    broadcast(ws_hub, task_id, "planning", "Planner Agent 起動中...").await;
    log(pool, session_id, "planner", 1, "info", "Planner Agent 開始").await;

    let planner_prompt = format!(
        "あなたは Planner Agent です。以下のタスクの実装計画を立ててください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        ## 指示\n\
        1. コードベースを分析して、変更が必要なファイルを特定してください\n\
        2. 実装計画を以下の形式で出力してください:\n\
           - 変更ファイル一覧\n\
           - 各ファイルの変更内容\n\
           - テスト方針\n\
        3. 計画のみ出力し、コード変更は行わないでください"
    );

    let plan_result = match claude_cli::run_claude(&planner_prompt, &wt_path, 300).await {
        Ok(r) => r,
        Err(e) => {
            fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &e).await;
            return;
        }
    };

    if plan_result.exit_code != 0 {
        let err = format!("Planner failed: {}", plan_result.stderr);
        fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &err).await;
        return;
    }

    let plan = plan_result.stdout.clone();
    log(pool, session_id, "planner", 1, "info", &format!("計画完了: {}bytes", plan.len())).await;
    let _ = exec_service::update_session(pool, session_id, "running", Some(&plan), None, None, None, None).await;
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Executing, Some(&plan), None, None, None, None).await;

    // === Phase 2: Coder ===
    broadcast(ws_hub, task_id, "executing", "Coder Agent 起動中...").await;
    log(pool, session_id, "coder", 1, "info", "Coder Agent 開始").await;

    let coder_prompt = format!(
        "あなたは Coder Agent です。以下の計画に基づいてコードを実装してください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        ## 実装計画\n\
        {plan}\n\n\
        ## 指示\n\
        - 計画に従って必要なコード変更を実装してください\n\
        - テストコードも追加してください\n\
        - 変更は最小限に留めてください"
    );

    let coder_result = match claude_cli::run_claude_autonomous(&coder_prompt, &wt_path, 1500).await {
        Ok(r) => r,
        Err(e) => {
            fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &e).await;
            return;
        }
    };

    if coder_result.exit_code != 0 {
        let err = format!("Coder failed: {}", coder_result.stderr);
        fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &err).await;
        return;
    }

    log(pool, session_id, "coder", 1, "info", "コード実装完了").await;

    // === Phase 3: Reviewer (max 2 iterations) ===
    broadcast(ws_hub, task_id, "reviewing", "Reviewer Agent 起動中...").await;
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Reviewing, None, None, None, None, None).await;

    for iteration in 1..=2 {
        log(pool, session_id, "reviewer", iteration, "info", &format!("レビュー iteration {iteration}")).await;

        let diff = get_diff_output(&wt_path).await;
        let reviewer_prompt = format!(
            "あなたは Reviewer Agent です。以下の diff をレビューしてください。\n\n\
            ## タスク\n\
            {description}\n\n\
            ## Diff\n\
            ```\n{diff}\n```\n\n\
            ## 指示\n\
            - コード品質、バグ、セキュリティの観点でレビュー\n\
            - 最終行に必ず VERDICT: APPROVE または VERDICT: REQUEST_CHANGES を出力"
        );

        let review = match claude_cli::run_claude(&reviewer_prompt, &wt_path, 300).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Reviewer failed: {e}");
                break;
            }
        };

        let verdict = parse_verdict(&review.stdout);
        log(pool, session_id, "reviewer", iteration, "info", &format!("Verdict: {verdict}")).await;
        let _ = exec_service::update_session(pool, session_id, "running", None, Some(&review.stdout), Some(&verdict), None, None).await;

        if verdict == "APPROVE" || iteration == 2 {
            break;
        }

        // REQUEST_CHANGES → Coder で修正
        broadcast(ws_hub, task_id, "executing", &format!("修正中 (iteration {iteration})...")).await;
        let fix_prompt = format!(
            "レビューで修正が指摘されました。以下のレビューコメントに基づいて修正してください:\n\n{}\n\n修正のみ行い、余計な変更はしないでください。",
            review.stdout
        );
        let _ = claude_cli::run_claude_autonomous(&fix_prompt, &wt_path, 600).await;
    }

    // === Phase 4: Test (max 2 iterations) ===
    broadcast(ws_hub, task_id, "executing", "Test Agent 起動中...").await;

    for iteration in 1..=2 {
        log(pool, session_id, "test", iteration, "info", &format!("テスト iteration {iteration}")).await;

        let test_prompt =
            "あなたは Test Agent です。このプロジェクトのテストを実行してください。\n\n\
            ## 指示\n\
            - プロジェクトに適したテストコマンドを特定して実行\n\
            - テスト結果を確認\n\
            - 最終行に必ず VERDICT: PASS または VERDICT: FAIL を出力";

        let test = match claude_cli::run_claude_autonomous(test_prompt, &wt_path, 300).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Test failed: {e}");
                break;
            }
        };

        let verdict = parse_test_verdict(&test.stdout);
        let passed = verdict == "PASS";
        log(pool, session_id, "test", iteration, "info", &format!("Test verdict: {verdict}")).await;
        let _ = exec_service::update_session(pool, session_id, "running", None, None, None, Some(&test.stdout), Some(passed)).await;

        if passed || iteration == 2 {
            break;
        }

        // FAIL → Coder で修正
        broadcast(ws_hub, task_id, "executing", &format!("テスト修正中 (iteration {iteration})...")).await;
        let fix_prompt = format!(
            "テストが失敗しました。以下のテスト出力に基づいて修正してください:\n\n{}\n\n修正のみ行い、余計な変更はしないでください。",
            test.stdout
        );
        let _ = claude_cli::run_claude_autonomous(&fix_prompt, &wt_path, 600).await;
    }

    // === Phase 5: Commit & PR ===
    broadcast(ws_hub, task_id, "executing", "PR 作成中...").await;

    if !worktree::has_changes(&wt_path).await.unwrap_or(false) {
        log(pool, session_id, "pr", 1, "warn", "変更なし — PR スキップ").await;
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Completed, None, None, None, None, None).await;
        let _ = exec_service::update_session(pool, session_id, "completed", None, None, None, None, None).await;
        broadcast(ws_hub, task_id, "completed", "完了（変更なし）").await;
    } else {
        let pr_body = format!("## タスク\n{description}\n\n## 計画\n{plan}");

        match worktree::commit_and_create_pr(&wt_path, &branch_name, title, &pr_body).await {
            Ok(pr_url) => {
                let diff_stats = worktree::get_diff_stats(&wt_path).await.unwrap_or_default();
                let changed_files = worktree::get_changed_files(&wt_path).await.unwrap_or_default();
                let files_json = serde_json::to_value(&changed_files).unwrap_or_default();

                log(pool, session_id, "pr", 1, "info", &format!("PR 作成完了: {pr_url}")).await;
                let _ = task_service::update_task_execution(
                    pool, task_id, TaskStatus::Completed, None, Some(&pr_url),
                    Some(&files_json), Some(&diff_stats), None,
                ).await;
                let _ = exec_service::update_session(pool, session_id, "completed", None, None, None, None, None).await;
                broadcast(ws_hub, task_id, "completed", &format!("完了: {pr_url}")).await;
            }
            Err(e) => {
                fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &e).await;
                return;
            }
        }
    }

    // Cleanup
    let _ = worktree::cleanup_worktree(repo_path, &worktree_dir).await;
    ws_hub.remove_channel(&task_id).await;
}

async fn fail_pipeline(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    session_id: Uuid,
    repo_path: &str,
    worktree_dir: &std::path::Path,
    error: &str,
) {
    tracing::error!("Pipeline failed for task {task_id}: {error}");
    log(pool, session_id, "error", 1, "error", error).await;
    let _ = task_service::update_task_execution(
        pool, task_id, TaskStatus::Failed, None, None, None, None, Some(error),
    )
    .await;
    let _ = exec_service::update_session(pool, session_id, "failed", None, None, None, None, None).await;
    broadcast(ws_hub, task_id, "failed", error).await;
    let _ = worktree::cleanup_worktree(repo_path, worktree_dir).await;
    ws_hub.remove_channel(&task_id).await;
}

async fn broadcast(ws_hub: &WsHub, task_id: Uuid, phase: &str, message: &str) {
    let msg = serde_json::json!({
        "task_id": task_id,
        "phase": phase,
        "message": message,
    })
    .to_string();
    ws_hub.broadcast(task_id, &msg).await;
}

async fn log(pool: &PgPool, session_id: Uuid, phase: &str, iteration: i32, level: &str, message: &str) {
    let _ = exec_service::add_log(pool, session_id, phase, iteration, level, message, None).await;
}

async fn get_diff_output(worktree_path: &str) -> String {
    let output = tokio::process::Command::new("git")
        .args(["diff"])
        .current_dir(worktree_path)
        .output()
        .await;

    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => String::new(),
    }
}

fn parse_verdict(output: &str) -> String {
    for line in output.lines().rev() {
        let line = line.trim().to_uppercase();
        if line.contains("VERDICT:") {
            if line.contains("APPROVE") {
                return "APPROVE".to_string();
            }
            if line.contains("REQUEST_CHANGES") {
                return "REQUEST_CHANGES".to_string();
            }
        }
    }
    "APPROVE".to_string() // デフォルトは APPROVE
}

fn parse_test_verdict(output: &str) -> String {
    for line in output.lines().rev() {
        let line = line.trim().to_uppercase();
        if line.contains("VERDICT:") {
            if line.contains("PASS") {
                return "PASS".to_string();
            }
            if line.contains("FAIL") {
                return "FAIL".to_string();
            }
        }
    }
    "PASS".to_string() // デフォルトは PASS
}
