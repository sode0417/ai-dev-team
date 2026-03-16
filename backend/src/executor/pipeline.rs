use sqlx::PgPool;
use uuid::Uuid;

use crate::domains::executions::service as exec_service;
use crate::domains::tasks::model::{HearingQuestion, TaskStatus};
use crate::domains::tasks::service as task_service;
use crate::ws::WsHub;
use super::claude_cli;
use super::worktree;

/// 既存の一括実行パイプライン (skip_hearing=true 時に使用)
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
    run_coder_to_pr(pool, ws_hub, task_id, session_id, title, description, &plan, "", &wt_path, &branch_name, repo_path, &worktree_dir).await;
}

/// ヒアリングフェーズ: コードベース分析 → 質問生成 → hearing で停止
pub async fn run_hearing_phase(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    title: &str,
    description: &str,
    repo_path: &str,
    base_branch: &str,
) {
    // worktree 作成
    let (worktree_dir, branch_name) = match worktree::create_worktree(repo_path, task_id, base_branch).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to create worktree: {e}");
            let _ = task_service::update_task_execution(
                pool, task_id, TaskStatus::Failed, None, None, None, None, Some(&e),
            ).await;
            broadcast(ws_hub, task_id, "error", &e).await;
            return;
        }
    };

    let wt_path = worktree_dir.to_str().unwrap().to_string();

    // セッション作成
    let session = match exec_service::create_session(pool, task_id, Some(&wt_path), Some(&branch_name)).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create session: {e}");
            let _ = worktree::cleanup_worktree(repo_path, &worktree_dir).await;
            return;
        }
    };

    let session_id = session.id;

    broadcast(ws_hub, task_id, "hearing", "Hearing Agent 起動中...").await;
    log(pool, session_id, "hearing", 1, "info", "Hearing Agent 開始").await;

    let hearing_prompt = format!(
        "あなたは Hearing Agent です。以下のタスクを実装するために、ユーザーに確認すべき不明点を質問として生成してください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        ## 指示\n\
        1. コードベースの構造を分析してください\n\
        2. このタスクを実装するために確認が必要な点を質問として生成してください\n\
        3. 以下の JSON 形式で出力してください（質問がなければ空配列）:\n\
        ```json\n\
        [\n\
          {{\"index\": 1, \"question\": \"質問内容\", \"options\": [\"選択肢A\", \"選択肢B\"]}},\n\
          {{\"index\": 2, \"question\": \"自由回答の質問\"}}\n\
        ]\n\
        ```\n\
        4. JSON のみ出力してください（説明文は不要）\n\
        5. 明らかにコードから判断できることは質問しないでください\n\
        6. 最大5問までにしてください"
    );

    let result = match claude_cli::run_claude(&hearing_prompt, &wt_path, 300).await {
        Ok(r) => r,
        Err(e) => {
            fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &e).await;
            return;
        }
    };

    if result.exit_code != 0 {
        let err = format!("Hearing Agent failed: {}", result.stderr);
        fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &err).await;
        return;
    }

    let questions = parse_questions(&result.stdout);

    if questions.is_empty() {
        // 質問なし → 直接 planning フェーズへ
        log(pool, session_id, "hearing", 1, "info", "質問なし — 計画フェーズへ").await;
        broadcast(ws_hub, task_id, "planning", "質問なし — Planner Agent 起動中...").await;
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Planning, None, None, None, None, None).await;
        run_planning_phase(pool, ws_hub, task_id, title, description, repo_path).await;
    } else {
        // 質問あり → hearing レコード保存 → status = hearing で停止
        let questions_json = serde_json::to_value(&questions).unwrap_or_default();
        let _ = task_service::create_hearing(pool, task_id, Some(session_id), "pre_plan", 1, &questions_json).await;
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Hearing, None, None, None, None, None).await;
        log(pool, session_id, "hearing", 1, "info", &format!("{}件の質問を生成", questions.len())).await;
        broadcast(ws_hub, task_id, "hearing", &format!("{}件の質問があります", questions.len())).await;
    }
}

/// 計画フェーズ: ヒアリング回答 + タスク説明 → Planner → awaiting_approval で停止
pub async fn run_planning_phase(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    title: &str,
    description: &str,
    repo_path: &str,
) {
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Planning, None, None, None, None, None).await;
    broadcast(ws_hub, task_id, "planning", "Planner Agent 起動中...").await;

    // セッション取得（最新のもの）
    let sessions = exec_service::list_sessions(pool, task_id).await.unwrap_or_default();
    let session = match sessions.first() {
        Some(s) => s,
        None => {
            let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Failed, None, None, None, None, Some("No session found")).await;
            broadcast(ws_hub, task_id, "failed", "セッションが見つかりません").await;
            return;
        }
    };

    let session_id = session.id;
    let wt_path = match &session.worktree_path {
        Some(p) => p.clone(),
        None => {
            let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Failed, None, None, None, None, Some("No worktree path")).await;
            broadcast(ws_hub, task_id, "failed", "worktree パスが見つかりません").await;
            return;
        }
    };

    log(pool, session_id, "planner", 1, "info", "Planner Agent 開始").await;

    // ヒアリング回答をコンテキストとして取得
    let hearing_context = task_service::get_hearing_context(pool, task_id).await.unwrap_or_default();

    let planner_prompt = format!(
        "あなたは Planner Agent です。以下のタスクの実装計画を立ててください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        {hearing_section}\
        ## 指示\n\
        1. コードベースを分析して、変更が必要なファイルを特定してください\n\
        2. 実装計画を以下の形式で出力してください:\n\
           - 変更ファイル一覧\n\
           - 各ファイルの変更内容\n\
           - テスト方針\n\
        3. 計画のみ出力し、コード変更は行わないでください\n\
        4. 不明点がある場合は、計画の最後に ## 確認事項 セクションを追加し、以下の JSON 形式で質問を記載してください:\n\
        ```json\n\
        [{{\"index\": 1, \"question\": \"質問内容\"}}]\n\
        ```\n\
        5. 不明点がなければ ## 確認事項 セクションは不要です",
        hearing_section = if hearing_context.is_empty() {
            String::new()
        } else {
            format!("## ヒアリング回答\n{hearing_context}\n")
        }
    );

    let plan_result = match claude_cli::run_claude(&planner_prompt, &wt_path, 300).await {
        Ok(r) => r,
        Err(e) => {
            let worktree_dir = std::path::PathBuf::from(&wt_path);
            fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &e).await;
            return;
        }
    };

    if plan_result.exit_code != 0 {
        let err = format!("Planner failed: {}", plan_result.stderr);
        let worktree_dir = std::path::PathBuf::from(&wt_path);
        fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, &worktree_dir, &err).await;
        return;
    }

    let plan_output = plan_result.stdout.clone();
    log(pool, session_id, "planner", 1, "info", &format!("計画完了: {}bytes", plan_output.len())).await;
    let _ = exec_service::update_session(pool, session_id, "running", Some(&plan_output), None, None, None, None).await;

    // 計画から追加質問を抽出
    let additional_questions = extract_plan_questions(&plan_output);

    if !additional_questions.is_empty() {
        // 追加質問あり → in_plan ヒアリング
        let questions_json = serde_json::to_value(&additional_questions).unwrap_or_default();
        let _ = task_service::create_hearing(pool, task_id, Some(session_id), "in_plan", 1, &questions_json).await;
        // plan は保存するが awaiting_approval にはしない
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Hearing, Some(&plan_output), None, None, None, None).await;
        log(pool, session_id, "planner", 1, "info", &format!("計画中に{}件の追加質問", additional_questions.len())).await;
        broadcast(ws_hub, task_id, "hearing", &format!("計画に関して{}件の確認事項があります", additional_questions.len())).await;
    } else {
        // 質問なし → awaiting_approval で停止
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::AwaitingApproval, Some(&plan_output), None, None, None, None).await;
        log(pool, session_id, "planner", 1, "info", "計画完了 — 承認待ち").await;
        broadcast(ws_hub, task_id, "awaiting_approval", "計画が完成しました。承認をお願いします。").await;
    }
}

/// 実行フェーズ: 計画承認後に Coder → Reviewer → Test → PR
pub async fn run_execution_phase(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    title: &str,
    description: &str,
    repo_path: &str,
) {
    // セッション取得
    let sessions = exec_service::list_sessions(pool, task_id).await.unwrap_or_default();
    let session = match sessions.first() {
        Some(s) => s,
        None => {
            let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Failed, None, None, None, None, Some("No session found")).await;
            return;
        }
    };

    let session_id = session.id;
    let wt_path = match &session.worktree_path {
        Some(p) => p.clone(),
        None => {
            let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Failed, None, None, None, None, Some("No worktree path")).await;
            return;
        }
    };
    let branch_name = session.branch_name.clone().unwrap_or_default();

    // タスクの plan を取得
    let task = match task_service::get_task(pool, task_id).await {
        Ok(t) => t,
        Err(_) => return,
    };
    let plan = task.plan.unwrap_or_default();

    // ヒアリングコンテキスト
    let hearing_context = task_service::get_hearing_context(pool, task_id).await.unwrap_or_default();

    let worktree_dir = std::path::PathBuf::from(&wt_path);
    run_coder_to_pr(pool, ws_hub, task_id, session_id, title, description, &plan, &hearing_context, &wt_path, &branch_name, repo_path, &worktree_dir).await;
}

/// Coder → Reviewer → Test → PR の共通処理
async fn run_coder_to_pr(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    session_id: Uuid,
    title: &str,
    description: &str,
    plan: &str,
    hearing_context: &str,
    wt_path: &str,
    branch_name: &str,
    repo_path: &str,
    worktree_dir: &std::path::Path,
) {
    broadcast(ws_hub, task_id, "executing", "Coder Agent 起動中...").await;
    log(pool, session_id, "coder", 1, "info", "Coder Agent 開始").await;

    let hearing_section = if hearing_context.is_empty() {
        String::new()
    } else {
        format!("## ヒアリング回答\n{hearing_context}\n")
    };

    let coder_prompt = format!(
        "あなたは Coder Agent です。以下の計画に基づいてコードを実装してください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        {hearing_section}\
        ## 実装計画\n\
        {plan}\n\n\
        ## 指示\n\
        - 計画に従って必要なコード変更を実装してください\n\
        - テストコードも追加してください\n\
        - 変更は最小限に留めてください"
    );

    let coder_result = match claude_cli::run_claude_autonomous(&coder_prompt, wt_path, 1500).await {
        Ok(r) => r,
        Err(e) => {
            fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, worktree_dir, &e).await;
            return;
        }
    };

    if coder_result.exit_code != 0 {
        let err = format!("Coder failed: {}", coder_result.stderr);
        fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, worktree_dir, &err).await;
        return;
    }

    log(pool, session_id, "coder", 1, "info", "コード実装完了").await;

    // === Reviewer (max 2 iterations) ===
    broadcast(ws_hub, task_id, "reviewing", "Reviewer Agent 起動中...").await;
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Reviewing, None, None, None, None, None).await;

    for iteration in 1..=2 {
        log(pool, session_id, "reviewer", iteration, "info", &format!("レビュー iteration {iteration}")).await;

        let diff = get_diff_output(wt_path).await;
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

        let review = match claude_cli::run_claude(&reviewer_prompt, wt_path, 300).await {
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
        let _ = claude_cli::run_claude_autonomous(&fix_prompt, wt_path, 600).await;
    }

    // === Test (max 2 iterations) ===
    broadcast(ws_hub, task_id, "executing", "Test Agent 起動中...").await;

    for iteration in 1..=2 {
        log(pool, session_id, "test", iteration, "info", &format!("テスト iteration {iteration}")).await;

        let test_prompt =
            "あなたは Test Agent です。このプロジェクトのテストを実行してください。\n\n\
            ## 指示\n\
            - プロジェクトに適したテストコマンドを特定して実行\n\
            - テスト結果を確認\n\
            - 最終行に必ず VERDICT: PASS または VERDICT: FAIL を出力";

        let test = match claude_cli::run_claude_autonomous(test_prompt, wt_path, 300).await {
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
        let _ = claude_cli::run_claude_autonomous(&fix_prompt, wt_path, 600).await;
    }

    // === Commit & PR ===
    broadcast(ws_hub, task_id, "executing", "PR 作成中...").await;

    if !worktree::has_changes(wt_path).await.unwrap_or(false) {
        log(pool, session_id, "pr", 1, "warn", "変更なし — PR スキップ").await;
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Completed, None, None, None, None, None).await;
        let _ = exec_service::update_session(pool, session_id, "completed", None, None, None, None, None).await;
        broadcast(ws_hub, task_id, "completed", "完了（変更なし）").await;
    } else {
        let pr_body = format!("## タスク\n{description}\n\n## 計画\n{plan}");

        match worktree::commit_and_create_pr(wt_path, branch_name, title, &pr_body).await {
            Ok(pr_url) => {
                let diff_stats = worktree::get_diff_stats(wt_path).await.unwrap_or_default();
                let changed_files = worktree::get_changed_files(wt_path).await.unwrap_or_default();
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
                fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, worktree_dir, &e).await;
                return;
            }
        }
    }

    // Cleanup
    let _ = worktree::cleanup_worktree(repo_path, worktree_dir).await;
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

/// Claude 出力から JSON 形式の質問リストをパース
fn parse_questions(output: &str) -> Vec<HearingQuestion> {
    // JSON ブロックを探す（```json ... ``` or 直接 JSON 配列）
    let json_str = if let Some(start) = output.find("```json") {
        let start = start + 7;
        if let Some(end) = output[start..].find("```") {
            output[start..start + end].trim()
        } else {
            output[start..].trim()
        }
    } else if let Some(start) = output.find('[') {
        if let Some(end) = output.rfind(']') {
            &output[start..=end]
        } else {
            return vec![];
        }
    } else {
        return vec![];
    };

    serde_json::from_str::<Vec<HearingQuestion>>(json_str).unwrap_or_default()
}

/// Planner 出力の「## 確認事項」セクションから追加質問を抽出
fn extract_plan_questions(plan_output: &str) -> Vec<HearingQuestion> {
    // "## 確認事項" セクションを探す
    let section_markers = ["## 確認事項", "## Confirmation Items", "## Questions"];
    let section_start = section_markers.iter().find_map(|marker| {
        plan_output.find(marker).map(|pos| pos + marker.len())
    });

    let section = match section_start {
        Some(start) => {
            // 次の ## セクションまでを取得
            let rest = &plan_output[start..];
            if let Some(end) = rest[1..].find("\n## ") {
                &rest[..end + 1]
            } else {
                rest
            }
        }
        None => return vec![],
    };

    // JSON ブロックを探す
    parse_questions(section)
}
