use sqlx::PgPool;
use uuid::Uuid;

use crate::config::timeout;
use crate::domains::executions::model::ExecutionSession;
use crate::domains::executions::service as exec_service;
use crate::domains::tasks::model::{HearingQuestion, TaskStatus};
use crate::domains::tasks::service as task_service;
use crate::factrail;
use crate::ws::WsHub;
use super::claude_cli;
use super::claude_cli::ClaudeResult;
use super::worktree;

/// 出力テキストを指定文字数で切り詰める
fn truncate(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        s
    } else {
        // UTF-8 境界を安全に扱う
        let mut end = max_chars;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

/// セッションの各フェーズ結果から PR 本文を構築
fn build_pr_body(description: &str, diff_stats: &str, session: Option<&ExecutionSession>) -> String {
    let mut body = format!("## 概要\n{description}\n");

    if !diff_stats.trim().is_empty() {
        body.push_str(&format!("\n## 変更内容\n```\n{}\n```\n", diff_stats.trim()));
    }

    if let Some(s) = session {
        // テスト結果
        if let Some(passed) = s.test_passed {
            let verdict = if passed { "PASS" } else { "FAIL" };
            body.push_str(&format!("\n## テスト結果\n- テスト: **{verdict}**\n"));
            if let Some(ref output) = s.test_output {
                let summary = truncate(output.trim(), 500);
                body.push_str(&format!("\n<details><summary>テスト出力</summary>\n\n```\n{summary}\n```\n</details>\n"));
            }
        }

        // QA 結果
        if let Some(passed) = s.qa_passed {
            let verdict = if passed { "PASS" } else { "FAIL" };
            body.push_str(&format!("\n## QA 確認\n- QA: **{verdict}**\n"));
        }

        // レビュー結果
        if let Some(ref verdict) = s.review_verdict {
            body.push_str(&format!("\n## レビュー\n- Review: **{verdict}**\n"));
            if let Some(ref output) = s.review_output {
                let summary = truncate(output.trim(), 500);
                body.push_str(&format!("\n<details><summary>レビュー出力</summary>\n\n```\n{summary}\n```\n</details>\n"));
            }
        }
    }

    body
}

/// DoD セクションを構築（プロンプト注入用）
fn build_dod_section(definition_of_done: &Option<String>) -> String {
    match definition_of_done {
        Some(dod) if !dod.trim().is_empty() => {
            format!("## 完了条件 (Definition of Done)\n{dod}\n\n")
        }
        _ => String::new(),
    }
}

/// タイムアウト時に1回だけリトライする run_claude ラッパー
async fn run_claude_with_retry(
    prompt: &str,
    working_dir: &str,
    timeout_secs: u64,
) -> Result<ClaudeResult, String> {
    match claude_cli::run_claude(prompt, working_dir, timeout_secs).await {
        Ok(r) => Ok(r),
        Err(e) if e.contains("timed out") => {
            tracing::warn!("Claude timed out, retrying once: {e}");
            claude_cli::run_claude(prompt, working_dir, timeout_secs).await
        }
        Err(e) => Err(e),
    }
}

/// タイムアウト時に1回だけリトライする run_claude_autonomous ラッパー
async fn run_claude_autonomous_with_retry(
    prompt: &str,
    working_dir: &str,
    timeout_secs: u64,
) -> Result<ClaudeResult, String> {
    match claude_cli::run_claude_autonomous(prompt, working_dir, timeout_secs).await {
        Ok(r) => Ok(r),
        Err(e) if e.contains("timed out") => {
            tracing::warn!("Claude autonomous timed out, retrying once: {e}");
            claude_cli::run_claude_autonomous(prompt, working_dir, timeout_secs).await
        }
        Err(e) => Err(e),
    }
}

/// タイムアウト時に1回だけリトライする run_claude_with_mcp ラッパー
async fn run_claude_with_mcp_with_retry(
    prompt: &str,
    working_dir: &str,
    timeout_secs: u64,
    mcp_config_path: &str,
) -> Result<ClaudeResult, String> {
    match claude_cli::run_claude_with_mcp(prompt, working_dir, timeout_secs, mcp_config_path).await {
        Ok(r) => Ok(r),
        Err(e) if e.contains("timed out") => {
            tracing::warn!("Claude MCP timed out, retrying once: {e}");
            claude_cli::run_claude_with_mcp(prompt, working_dir, timeout_secs, mcp_config_path).await
        }
        Err(e) => Err(e),
    }
}

/// 既存の一括実行パイプライン (skip_hearing=true 時に使用)
pub async fn run_pipeline(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    title: &str,
    description: &str,
    repo_path: &str,
    base_branch: &str,
    proposal_type: &str,
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

    let wt_path = match worktree_dir.to_str() {
        Some(s) => s.to_string(),
        None => {
            tracing::error!("Worktree path contains non-UTF-8 characters");
            let _ = worktree::cleanup_worktree(repo_path, &worktree_dir).await;
            return;
        }
    };

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

    // DoD 取得
    let dod = task_service::get_task(pool, task_id).await.ok()
        .and_then(|t| t.definition_of_done);
    let dod_section = build_dod_section(&dod);

    let planner_prompt = format!(
        "あなたは Planner Agent です。以下のタスクの実装計画を立ててください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        {dod_section}\
        ## 指示\n\
        1. コードベースを分析して、変更が必要なファイルを特定してください\n\
        2. 実装計画を以下の形式で出力してください:\n\
           - 変更ファイル一覧\n\
           - 各ファイルの変更内容\n\
           - テスト方針\n\
        3. 計画のみ出力し、コード変更は行わないでください\n\
        4. 完了条件がある場合は、すべての条件を満たす計画にしてください"
    );

    let plan_result = match run_claude_with_retry(&planner_prompt, &wt_path, timeout::PLANNER_SECS).await {
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

    // === Phase 2: 実行 (proposal_type で分岐) ===
    match proposal_type {
        "investigation" => {
            run_investigation(pool, ws_hub, task_id, session_id, title, description, &plan, &wt_path, repo_path, &worktree_dir).await;
        }
        "operation" => {
            run_operation(pool, ws_hub, task_id, session_id, title, description, &plan, &wt_path, repo_path, &worktree_dir).await;
        }
        "improvement" => {
            run_coder_to_pr(pool, ws_hub, task_id, session_id, title, description, &plan, "", &wt_path, &branch_name, repo_path, &worktree_dir, true, true).await;
        }
        _ => {
            run_coder_to_pr(pool, ws_hub, task_id, session_id, title, description, &plan, "", &wt_path, &branch_name, repo_path, &worktree_dir, false, false).await;
        }
    }
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
    proposal_type: &str,
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

    let wt_path = match worktree_dir.to_str() {
        Some(s) => s.to_string(),
        None => {
            tracing::error!("Worktree path contains non-UTF-8 characters");
            let _ = worktree::cleanup_worktree(repo_path, &worktree_dir).await;
            return;
        }
    };

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

    let result = match run_claude_with_retry(&hearing_prompt, &wt_path, timeout::HEARING_SECS).await {
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
        run_planning_phase(pool, ws_hub, task_id, title, description, repo_path, proposal_type).await;
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
    proposal_type: &str,
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

    let hearing_section = if hearing_context.is_empty() {
        String::new()
    } else {
        format!("## ヒアリング回答\n{hearing_context}\n")
    };

    // DoD 取得
    let dod = task_service::get_task(pool, task_id).await.ok()
        .and_then(|t| t.definition_of_done);
    let dod_section = build_dod_section(&dod);

    let planner_prompt = match proposal_type {
        "investigation" => format!(
            "あなたは Planner Agent です。以下の調査タスクの調査計画を立ててください。\n\n\
            ## タスク\n\
            タイトル: {title}\n\
            説明: {description}\n\n\
            {hearing_section}\
            {dod_section}\
            ## 指示\n\
            1. コードベースを分析して、調査対象のファイルや領域を特定してください\n\
            2. 調査計画を以下の形式で出力してください:\n\
               - 調査対象ファイル・領域一覧\n\
               - 各調査項目の確認内容\n\
               - 期待される調査結果の形式\n\
            3. 計画のみ出力し、実際の調査は行わないでください\n\
            4. 不明点がある場合は、計画の最後に ## 確認事項 セクションを追加し、以下の JSON 形式で質問を記載してください:\n\
            ```json\n\
            [{{\"index\": 1, \"question\": \"質問内容\"}}]\n\
            ```\n\
            5. 不明点がなければ ## 確認事項 セクションは不要です"
        ),
        "operation" => format!(
            "あなたは Planner Agent です。以下の GitHub 操作タスクの手順を計画してください。\n\n\
            ## タスク\n\
            タイトル: {title}\n\
            説明: {description}\n\n\
            {hearing_section}\
            {dod_section}\
            ## 指示\n\
            1. タスク内容を分析して、必要な GitHub 操作（Issue クローズ/作成/ラベル整理等）を特定してください\n\
            2. 操作計画を以下の形式で出力してください:\n\
               - 操作対象（Issue/PR/ラベル等）一覧\n\
               - 各操作の具体的な手順（gh コマンド）\n\
               - 操作の実行順序\n\
            3. 計画のみ出力し、実際の操作は行わないでください\n\
            4. 不明点がある場合は、計画の最後に ## 確認事項 セクションを追加し、以下の JSON 形式で質問を記載してください:\n\
            ```json\n\
            [{{\"index\": 1, \"question\": \"質問内容\"}}]\n\
            ```\n\
            5. 不明点がなければ ## 確認事項 セクションは不要です"
        ),
        _ => format!(
            "あなたは Planner Agent です。以下のタスクの実装計画を立ててください。\n\n\
            ## タスク\n\
            タイトル: {title}\n\
            説明: {description}\n\n\
            {hearing_section}\
            {dod_section}\
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
            5. 不明点がなければ ## 確認事項 セクションは不要です"
        ),
    };

    let plan_result = match run_claude_with_retry(&planner_prompt, &wt_path, timeout::PLANNER_SECS).await {
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

/// 実行フェーズ: 計画承認後に Coder → Reviewer → Test → PR (proposal_type で分岐)
pub async fn run_execution_phase(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    title: &str,
    description: &str,
    repo_path: &str,
    proposal_type: &str,
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

    match proposal_type {
        "investigation" => {
            run_investigation(pool, ws_hub, task_id, session_id, title, description, &plan, &wt_path, repo_path, &worktree_dir).await;
        }
        "operation" => {
            run_operation(pool, ws_hub, task_id, session_id, title, description, &plan, &wt_path, repo_path, &worktree_dir).await;
        }
        "improvement" => {
            run_coder_to_pr(pool, ws_hub, task_id, session_id, title, description, &plan, &hearing_context, &wt_path, &branch_name, repo_path, &worktree_dir, true, true).await;
        }
        _ => {
            run_coder_to_pr(pool, ws_hub, task_id, session_id, title, description, &plan, &hearing_context, &wt_path, &branch_name, repo_path, &worktree_dir, false, false).await;
        }
    }
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
    skip_review: bool,
    skip_test: bool,
) {
    broadcast(ws_hub, task_id, "executing", "Coder Agent 起動中...").await;
    log(pool, session_id, "coder", 1, "info", "Coder Agent 開始").await;

    let hearing_section = if hearing_context.is_empty() {
        String::new()
    } else {
        format!("## ヒアリング回答\n{hearing_context}\n")
    };

    // DoD 取得
    let dod = task_service::get_task(pool, task_id).await.ok()
        .and_then(|t| t.definition_of_done);
    let dod_section = build_dod_section(&dod);

    let coder_prompt = format!(
        "あなたは Coder Agent です。以下の計画に基づいてコードを実装してください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        {hearing_section}\
        {dod_section}\
        ## 実装計画\n\
        {plan}\n\n\
        ## 指示\n\
        - 計画に従って必要なコード変更を実装してください\n\
        - テストコードも追加してください\n\
        - 変更は最小限に留めてください\n\
        - 完了条件がある場合は、すべての条件を満たすように実装してください"
    );

    let coder_result = match run_claude_autonomous_with_retry(&coder_prompt, wt_path, timeout::CODER_SECS).await {
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
    if !skip_review {
        broadcast(ws_hub, task_id, "reviewing", "Reviewer Agent 起動中...").await;
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Reviewing, None, None, None, None, None).await;

        for iteration in 1..=2 {
            log(pool, session_id, "reviewer", iteration, "info", &format!("レビュー iteration {iteration}")).await;

            let diff = get_diff_output(wt_path).await;
            let reviewer_dod = build_dod_section(&dod);
            let reviewer_prompt = format!(
                "あなたは Reviewer Agent です。以下の diff をレビューしてください。\n\n\
                ## タスク\n\
                {description}\n\n\
                {reviewer_dod}\
                ## Diff\n\
                ```\n{diff}\n```\n\n\
                ## 指示\n\
                - コード品質、バグ、セキュリティの観点でレビュー\n\
                - 完了条件がある場合は、すべての条件が満たされているか確認\n\
                - 最終行に必ず VERDICT: APPROVE または VERDICT: REQUEST_CHANGES を出力"
            );

            let review = match run_claude_with_retry(&reviewer_prompt, wt_path, timeout::REVIEWER_SECS).await {
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
            let _ = run_claude_autonomous_with_retry(&fix_prompt, wt_path, timeout::FIX_SECS).await;
        }
    } else {
        log(pool, session_id, "reviewer", 1, "info", "Reviewer スキップ (improvement タイプ)").await;
    }

    // === Test (max 2 iterations) ===
    if !skip_test {
        broadcast(ws_hub, task_id, "executing", "Test Agent 起動中...").await;

        for iteration in 1..=2 {
            log(pool, session_id, "test", iteration, "info", &format!("テスト iteration {iteration}")).await;

            let test_prompt =
                "あなたは Test Agent です。このプロジェクトのテストを実行してください。\n\n\
                ## 指示\n\
                - プロジェクトに適したテストコマンドを特定して実行\n\
                - テスト結果を確認\n\
                - 最終行に必ず VERDICT: PASS または VERDICT: FAIL を出力";

            let test = match run_claude_autonomous_with_retry(test_prompt, wt_path, timeout::TEST_SECS).await {
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
            let _ = run_claude_autonomous_with_retry(&fix_prompt, wt_path, timeout::FIX_SECS).await;
        }
    } else {
        log(pool, session_id, "test", 1, "info", "Test スキップ (improvement タイプ)").await;
    }

    // === QA (Playwright MCP, フロントエンド変更時のみ) ===
    if !skip_test {
        // git diff --name-only で変更ファイルを取得（commit 前なので staging + unstaged）
        let changed_files = get_changed_file_list(wt_path).await;
        if has_frontend_changes(&changed_files) {
            broadcast(ws_hub, task_id, "executing", "QA Agent 起動中 (Playwright)...").await;
            log(pool, session_id, "qa", 1, "info", "QA Agent 開始 — フロントエンド変更検出").await;

            let screenshot_dir = format!("data/qa-screenshots/{task_id}");
            // スクリーンショットディレクトリ作成
            let _ = tokio::fs::create_dir_all(&screenshot_dir).await;

            let mcp_config_path = std::env::current_dir()
                .map(|p| p.join("config/playwright-mcp.json").to_string_lossy().to_string())
                .unwrap_or_else(|_| "config/playwright-mcp.json".to_string());

            let qa_prompt = format!(
                "あなたは QA Agent です。Playwright MCP を使ってフロントエンドの動作確認を行ってください。\n\n\
                ## タスク\n\
                タイトル: {title}\n\
                説明: {description}\n\n\
                ## 変更ファイル\n\
                {changed_files}\n\n\
                ## 指示\n\
                1. まず開発サーバーを起動してください: `PORT=3199 npm run dev` (frontend/ ディレクトリで)\n\
                2. Playwright MCP の browser_navigate で http://localhost:3199 にアクセス\n\
                3. 変更に関連するページを確認し、以下をテスト:\n\
                   - ページが正しく表示されるか\n\
                   - UI 要素が期待通りに動作するか\n\
                   - エラーがコンソールに出ていないか\n\
                4. 各確認ポイントで browser_take_screenshot でスクリーンショットを取得\n\
                5. テスト完了後、開発サーバーを停止してください\n\
                6. スクリーンショットファイルを `{screenshot_dir}/` にコピーしてください\n\
                7. 最終行に必ず VERDICT: PASS または VERDICT: FAIL を出力\n\n\
                ## 注意\n\
                - dev server ポートは 3199 を使用（衝突回避）\n\
                - スクリーンショットのファイル名は `01_toppage.png`, `02_detail.png` のように連番で"
            );

            for iteration in 1..=2 {
                log(pool, session_id, "qa", iteration, "info", &format!("QA iteration {iteration}")).await;

                let qa_result = match run_claude_with_mcp_with_retry(&qa_prompt, wt_path, timeout::QA_SECS, &mcp_config_path).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("QA Agent failed: {e}");
                        log(pool, session_id, "qa", iteration, "warn", &format!("QA Agent エラー: {e}")).await;
                        break;
                    }
                };

                let verdict = parse_qa_verdict(&qa_result.stdout);
                let passed = verdict == "PASS";
                log(pool, session_id, "qa", iteration, "info", &format!("QA verdict: {verdict}")).await;

                // スクリーンショット一覧を取得
                let screenshots = collect_screenshots(&screenshot_dir).await;
                let screenshots_json = serde_json::to_value(&screenshots).unwrap_or_default();

                let _ = exec_service::update_session_with_qa(
                    pool, session_id, "running",
                    Some(&qa_result.stdout), Some(passed), Some(&screenshots_json),
                ).await;

                if passed || iteration == 2 {
                    break;
                }

                // FAIL → Coder で修正
                broadcast(ws_hub, task_id, "executing", &format!("QA 修正中 (iteration {iteration})...")).await;
                let fix_prompt = format!(
                    "QA テストが失敗しました。以下の QA 結果に基づいて修正してください:\n\n{}\n\n修正のみ行い、余計な変更はしないでください。",
                    qa_result.stdout
                );
                let _ = run_claude_autonomous_with_retry(&fix_prompt, wt_path, timeout::FIX_SECS).await;
            }
        } else {
            log(pool, session_id, "qa", 1, "info", "QA スキップ — フロントエンド変更なし").await;
        }
    } else {
        log(pool, session_id, "qa", 1, "info", "QA スキップ (improvement タイプ)").await;
    }

    // === Commit & PR ===
    broadcast(ws_hub, task_id, "executing", "PR 作成中...").await;

    if !worktree::has_changes(wt_path).await.unwrap_or(false) {
        log(pool, session_id, "pr", 1, "warn", "変更なし — PR スキップ").await;
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Completed, None, None, None, None, None).await;
        let _ = exec_service::update_session(pool, session_id, "completed", None, None, None, None, None).await;
        broadcast(ws_hub, task_id, "completed", "完了（変更なし）").await;
    } else {
        // commit 前に diff stats を取得
        let diff_stats = worktree::get_diff_stats_unstaged(wt_path).await.unwrap_or_default();

        // session から各フェーズの結果を取得して PR 本文を構築
        let session_data = exec_service::get_session(pool, session_id).await.ok();
        let pr_body = build_pr_body(description, &diff_stats, session_data.as_ref());

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

    // PR作成タスクではワークツリーを保持（修正依頼に備える）
    // ワークツリーは PR マージ後にバックグラウンドタスクで自動削除される
    ws_hub.remove_channel(&task_id).await;
    send_task_fact(pool, task_id, "completed").await;
    check_sprint_auto_transition(pool, ws_hub, task_id).await;
}

/// 修正依頼パイプライン: 既存ワークツリーで Coder→Reviewer→Test→QA→commit+push
pub async fn run_revision_phase(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    title: &str,
    description: &str,
    instructions: &str,
) {
    // 最新セッションからワークツリー・ブランチ情報を取得
    let sessions = exec_service::list_sessions(pool, task_id).await.unwrap_or_default();
    let prev_session = match sessions.first() {
        Some(s) => s,
        None => {
            let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Failed, None, None, None, None, Some("No previous session found")).await;
            broadcast(ws_hub, task_id, "failed", "前回のセッションが見つかりません").await;
            return;
        }
    };

    let wt_path = match &prev_session.worktree_path {
        Some(p) => p.clone(),
        None => {
            let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Failed, None, None, None, None, Some("No worktree path in previous session")).await;
            broadcast(ws_hub, task_id, "failed", "ワークツリーが見つかりません（既に削除済み）").await;
            return;
        }
    };
    let branch_name = prev_session.branch_name.clone().unwrap_or_default();

    // ワークツリーの存在確認
    if !std::path::Path::new(&wt_path).exists() {
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Failed, None, None, None, None, Some("Worktree directory does not exist")).await;
        broadcast(ws_hub, task_id, "failed", "ワークツリーディレクトリが存在しません").await;
        return;
    }

    // 新しいセッションを作成（既存ワークツリーを再利用）
    let session = match exec_service::create_session_with_instructions(
        pool, task_id, Some(&wt_path), Some(&branch_name), Some(instructions),
    ).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create revision session: {e}");
            let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Failed, None, None, None, None, Some(&format!("Session creation failed: {e}"))).await;
            return;
        }
    };

    let session_id = session.id;
    // タスクの plan とヒアリングコンテキストを取得
    let task = match task_service::get_task(pool, task_id).await {
        Ok(t) => t,
        Err(_) => return,
    };
    let plan = task.plan.unwrap_or_default();
    let dod = task.definition_of_done;
    let dod_section = build_dod_section(&dod);
    let hearing_context = task_service::get_hearing_context(pool, task_id).await.unwrap_or_default();

    // === Coder (修正指示付き) ===
    broadcast(ws_hub, task_id, "executing", "修正: Coder Agent 起動中...").await;
    log(pool, session_id, "coder", 1, "info", &format!("修正 Coder Agent 開始: {instructions}")).await;

    let hearing_section = if hearing_context.is_empty() {
        String::new()
    } else {
        format!("## ヒアリング回答\n{hearing_context}\n")
    };

    let coder_prompt = format!(
        "あなたは Coder Agent です。既存の PR に対して修正を行ってください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        {hearing_section}\
        {dod_section}\
        ## 元の実装計画\n\
        {plan}\n\n\
        ## 修正依頼\n\
        {instructions}\n\n\
        ## 指示\n\
        - 上記の修正依頼に基づいて必要なコード変更を実装してください\n\
        - これは既存 PR への追加修正です\n\
        - 修正依頼の内容に集中し、余計な変更は避けてください\n\
        - 完了条件がある場合は、すべての条件を満たすように修正してください"
    );

    let coder_result = match claude_cli::run_claude_autonomous(&coder_prompt, &wt_path, 1500).await {
        Ok(r) => r,
        Err(e) => {
            fail_revision(pool, ws_hub, task_id, session_id, &e).await;
            return;
        }
    };

    if coder_result.exit_code != 0 {
        let err = format!("Coder failed: {}", coder_result.stderr);
        fail_revision(pool, ws_hub, task_id, session_id, &err).await;
        return;
    }

    log(pool, session_id, "coder", 1, "info", "修正コード実装完了").await;

    // === Reviewer ===
    broadcast(ws_hub, task_id, "reviewing", "修正: Reviewer Agent 起動中...").await;
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Reviewing, None, None, None, None, None).await;

    for iteration in 1..=2 {
        log(pool, session_id, "reviewer", iteration, "info", &format!("修正レビュー iteration {iteration}")).await;

        let diff = get_diff_output(&wt_path).await;
        let reviewer_dod = build_dod_section(&dod);
        let reviewer_prompt = format!(
            "あなたは Reviewer Agent です。以下の diff をレビューしてください。\n\n\
            ## タスク\n\
            {description}\n\n\
            ## 修正依頼\n\
            {instructions}\n\n\
            {reviewer_dod}\
            ## Diff\n\
            ```\n{diff}\n```\n\n\
            ## 指示\n\
            - コード品質、バグ、セキュリティの観点でレビュー\n\
            - 修正依頼の内容が適切に反映されているか確認\n\
            - 完了条件がある場合は、すべての条件が満たされているか確認\n\
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

        broadcast(ws_hub, task_id, "executing", &format!("修正中 (iteration {iteration})...")).await;
        let fix_prompt = format!(
            "レビューで修正が指摘されました。以下のレビューコメントに基づいて修正してください:\n\n{}\n\n修正のみ行い、余計な変更はしないでください。",
            review.stdout
        );
        let _ = claude_cli::run_claude_autonomous(&fix_prompt, &wt_path, 600).await;
    }

    // === Test ===
    broadcast(ws_hub, task_id, "executing", "修正: Test Agent 起動中...").await;

    for iteration in 1..=2 {
        log(pool, session_id, "test", iteration, "info", &format!("修正テスト iteration {iteration}")).await;

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

        broadcast(ws_hub, task_id, "executing", &format!("修正テスト修正中 (iteration {iteration})...")).await;
        let fix_prompt = format!(
            "テストが失敗しました。以下のテスト出力に基づいて修正してください:\n\n{}\n\n修正のみ行い、余計な変更はしないでください。",
            test.stdout
        );
        let _ = claude_cli::run_claude_autonomous(&fix_prompt, &wt_path, 600).await;
    }

    // === QA (フロントエンド変更時のみ) ===
    let changed_files = get_changed_file_list(&wt_path).await;
    if has_frontend_changes(&changed_files) {
        broadcast(ws_hub, task_id, "executing", "修正: QA Agent 起動中 (Playwright)...").await;
        log(pool, session_id, "qa", 1, "info", "修正 QA Agent 開始").await;

        let screenshot_dir = format!("data/qa-screenshots/{task_id}");
        let _ = tokio::fs::create_dir_all(&screenshot_dir).await;

        let mcp_config_path = std::env::current_dir()
            .map(|p| p.join("config/playwright-mcp.json").to_string_lossy().to_string())
            .unwrap_or_else(|_| "config/playwright-mcp.json".to_string());

        let qa_prompt = format!(
            "あなたは QA Agent です。Playwright MCP を使ってフロントエンドの動作確認を行ってください。\n\n\
            ## タスク\n\
            タイトル: {title}\n\
            説明: {description}\n\n\
            ## 変更ファイル\n\
            {changed_files}\n\n\
            ## 指示\n\
            1. まず開発サーバーを起動してください: `PORT=3199 npm run dev` (frontend/ ディレクトリで)\n\
            2. Playwright MCP の browser_navigate で http://localhost:3199 にアクセス\n\
            3. 変更に関連するページを確認し、以下をテスト:\n\
               - ページが正しく表示されるか\n\
               - UI 要素が期待通りに動作するか\n\
               - エラーがコンソールに出ていないか\n\
            4. 各確認ポイントで browser_take_screenshot でスクリーンショットを取得\n\
            5. テスト完了後、開発サーバーを停止してください\n\
            6. スクリーンショットファイルを `{screenshot_dir}/` にコピーしてください\n\
            7. 最終行に必ず VERDICT: PASS または VERDICT: FAIL を出力\n\n\
            ## 注意\n\
            - dev server ポートは 3199 を使用（衝突回避）\n\
            - スクリーンショットのファイル名は `01_toppage.png`, `02_detail.png` のように連番で"
        );

        for iteration in 1..=2 {
            log(pool, session_id, "qa", iteration, "info", &format!("修正 QA iteration {iteration}")).await;

            let qa_result = match claude_cli::run_claude_with_mcp(&qa_prompt, &wt_path, 600, &mcp_config_path).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("QA Agent failed: {e}");
                    log(pool, session_id, "qa", iteration, "warn", &format!("QA Agent エラー: {e}")).await;
                    break;
                }
            };

            let verdict = parse_qa_verdict(&qa_result.stdout);
            let passed = verdict == "PASS";
            log(pool, session_id, "qa", iteration, "info", &format!("QA verdict: {verdict}")).await;

            let screenshots = collect_screenshots(&screenshot_dir).await;
            let screenshots_json = serde_json::to_value(&screenshots).unwrap_or_default();

            let _ = exec_service::update_session_with_qa(
                pool, session_id, "running",
                Some(&qa_result.stdout), Some(passed), Some(&screenshots_json),
            ).await;

            if passed || iteration == 2 {
                break;
            }

            broadcast(ws_hub, task_id, "executing", &format!("修正 QA 修正中 (iteration {iteration})...")).await;
            let fix_prompt = format!(
                "QA テストが失敗しました。以下の QA 結果に基づいて修正してください:\n\n{}\n\n修正のみ行い、余計な変更はしないでください。",
                qa_result.stdout
            );
            let _ = claude_cli::run_claude_autonomous(&fix_prompt, &wt_path, 600).await;
        }
    } else {
        log(pool, session_id, "qa", 1, "info", "QA スキップ — フロントエンド変更なし").await;
    }

    // === Commit & Push (既存 PR に追加コミット) ===
    broadcast(ws_hub, task_id, "executing", "修正コミット & プッシュ中...").await;

    if !worktree::has_changes(&wt_path).await.unwrap_or(false) {
        log(pool, session_id, "pr", 1, "warn", "変更なし — コミットスキップ").await;
        let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Completed, None, None, None, None, None).await;
        let _ = exec_service::update_session(pool, session_id, "completed", None, None, None, None, None).await;
        broadcast(ws_hub, task_id, "completed", "修正完了（変更なし）").await;
    } else {
        let truncated: String = instructions.chars().take(60).collect();
        let commit_msg = format!("fix: {truncated}");

        match worktree::commit_and_push(&wt_path, &branch_name, &commit_msg).await {
            Ok(()) => {
                let diff_stats = worktree::get_diff_stats(&wt_path).await.unwrap_or_default();
                let changed = worktree::get_changed_files(&wt_path).await.unwrap_or_default();
                let files_json = serde_json::to_value(&changed).unwrap_or_default();

                log(pool, session_id, "pr", 1, "info", "修正コミット & プッシュ完了").await;
                let _ = task_service::update_task_execution(
                    pool, task_id, TaskStatus::Completed, None, None,
                    Some(&files_json), Some(&diff_stats), None,
                ).await;
                let _ = exec_service::update_session(pool, session_id, "completed", None, None, None, None, None).await;
                broadcast(ws_hub, task_id, "completed", "修正完了: PR が更新されました").await;
            }
            Err(e) => {
                fail_revision(pool, ws_hub, task_id, session_id, &e).await;
                return;
            }
        }
    }

    ws_hub.remove_channel(&task_id).await;
    send_task_fact(pool, task_id, "completed").await;
    check_sprint_auto_transition(pool, ws_hub, task_id).await;
}

/// 修正パイプライン失敗（ワークツリーは保持する）
async fn fail_revision(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    session_id: Uuid,
    error: &str,
) {
    tracing::error!("Revision pipeline failed for task {task_id}: {error}");
    log(pool, session_id, "error", 1, "error", error).await;
    let _ = task_service::update_task_execution(
        pool, task_id, TaskStatus::Failed, None, None, None, None, Some(error),
    ).await;
    let _ = exec_service::update_session(pool, session_id, "failed", None, None, None, None, None).await;
    broadcast(ws_hub, task_id, "failed", error).await;
    // ワークツリーは保持（再度修正依頼できるように）
    ws_hub.remove_channel(&task_id).await;
    send_task_fact(pool, task_id, "failed").await;
    check_sprint_auto_transition(pool, ws_hub, task_id).await;
}

/// 調査タスク: Claude で調査 → 結果を plan に保存 → 完了 (PR なし)
async fn run_investigation(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    session_id: Uuid,
    title: &str,
    description: &str,
    plan: &str,
    wt_path: &str,
    repo_path: &str,
    worktree_dir: &std::path::Path,
) {
    broadcast(ws_hub, task_id, "executing", "Investigation Agent 起動中...").await;
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Executing, None, None, None, None, None).await;
    log(pool, session_id, "investigation", 1, "info", "調査開始").await;

    let investigation_prompt = format!(
        "あなたは Investigation Agent です。以下の調査計画に基づいてコードベースを調査し、結果をレポート形式で出力してください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        ## 調査計画\n\
        {plan}\n\n\
        ## 指示\n\
        - コードベースを分析・調査してください\n\
        - 調査結果をマークダウン形式のレポートとして出力してください\n\
        - コード変更は行わないでください\n\
        - 発見事項、推奨事項、次のアクションを明確に記載してください"
    );

    let result = match run_claude_with_retry(&investigation_prompt, wt_path, timeout::INVESTIGATION_SECS).await {
        Ok(r) => r,
        Err(e) => {
            fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, worktree_dir, &e).await;
            return;
        }
    };

    if result.exit_code != 0 {
        let err = format!("Investigation failed: {}", result.stderr);
        fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, worktree_dir, &err).await;
        return;
    }

    let report = result.stdout.clone();
    log(pool, session_id, "investigation", 1, "info", &format!("調査完了: {}bytes", report.len())).await;
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Completed, Some(&report), None, None, None, None).await;
    let _ = exec_service::update_session(pool, session_id, "completed", Some(&report), None, None, None, None).await;
    broadcast(ws_hub, task_id, "completed", "調査完了").await;

    // Cleanup
    let _ = worktree::cleanup_worktree(repo_path, worktree_dir).await;
    ws_hub.remove_channel(&task_id).await;
    send_task_fact(pool, task_id, "completed").await;
    check_sprint_auto_transition(pool, ws_hub, task_id).await;
}

/// 操作タスク: Claude autonomous で gh コマンド等を実行 → 結果を plan に保存 → 完了 (PR なし)
async fn run_operation(
    pool: &PgPool,
    ws_hub: &WsHub,
    task_id: Uuid,
    session_id: Uuid,
    title: &str,
    description: &str,
    plan: &str,
    wt_path: &str,
    repo_path: &str,
    worktree_dir: &std::path::Path,
) {
    broadcast(ws_hub, task_id, "executing", "Operation Agent 起動中...").await;
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Executing, None, None, None, None, None).await;
    log(pool, session_id, "operation", 1, "info", "操作開始").await;

    let operation_prompt = format!(
        "あなたは Operation Agent です。以下の操作計画に基づいて GitHub の操作を実行してください。\n\n\
        ## タスク\n\
        タイトル: {title}\n\
        説明: {description}\n\n\
        ## 操作計画\n\
        {plan}\n\n\
        ## 指示\n\
        - 計画に従って gh コマンド等で GitHub の操作を実行してください\n\
        - Issue のクローズ、作成、ラベル整理など、計画された操作を実行してください\n\
        - 実行した操作の結果をレポートとして出力してください\n\
        - コードの変更は行わないでください"
    );

    let result = match run_claude_autonomous_with_retry(&operation_prompt, wt_path, timeout::OPERATION_SECS).await {
        Ok(r) => r,
        Err(e) => {
            fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, worktree_dir, &e).await;
            return;
        }
    };

    if result.exit_code != 0 {
        let err = format!("Operation failed: {}", result.stderr);
        fail_pipeline(pool, ws_hub, task_id, session_id, repo_path, worktree_dir, &err).await;
        return;
    }

    let report = result.stdout.clone();
    log(pool, session_id, "operation", 1, "info", &format!("操作完了: {}bytes", report.len())).await;
    let _ = task_service::update_task_execution(pool, task_id, TaskStatus::Completed, Some(&report), None, None, None, None).await;
    let _ = exec_service::update_session(pool, session_id, "completed", Some(&report), None, None, None, None).await;
    broadcast(ws_hub, task_id, "completed", "操作完了").await;

    // Cleanup
    let _ = worktree::cleanup_worktree(repo_path, worktree_dir).await;
    ws_hub.remove_channel(&task_id).await;
    send_task_fact(pool, task_id, "completed").await;
    check_sprint_auto_transition(pool, ws_hub, task_id).await;
}

/// タスク完了/失敗時にスプリントの自動遷移をチェック
/// スプリント内の全タスクが終了状態なら retrospective に遷移
async fn check_sprint_auto_transition(pool: &PgPool, ws_hub: &WsHub, task_id: Uuid) {
    // タスクの sprint_id を取得
    let task = match task_service::get_task(pool, task_id).await {
        Ok(t) => t,
        Err(_) => return,
    };

    let sprint_id = match task.sprint_id {
        Some(id) => id,
        None => return,
    };

    // スプリントが executing 状態か確認（executing 以外のフェーズでは遷移しない）
    let sprint = match crate::domains::sprints::service::get_sprint(pool, sprint_id).await {
        Ok(s) => s,
        Err(_) => return,
    };

    if sprint.status != "executing" {
        return;
    }

    // 全タスクが終了状態か確認
    let all_terminal = crate::domains::sprints::service::all_tasks_terminal(pool, sprint_id)
        .await
        .unwrap_or(false);

    if !all_terminal {
        return;
    }

    // run_sprint_execution が既に走っている場合はそちらに任せる
    // ここでは run_sprint_execution を経由しない個別実行のケースをカバー
    tracing::info!("Sprint {sprint_id}: all tasks terminal, auto-triggering retrospective");

    let pool = pool.clone();
    let ws_hub = ws_hub.clone();
    tokio::spawn(async move {
        let sprint_ws_msg = serde_json::json!({
            "sprint_id": sprint_id.to_string(),
            "phase": "generating_retro",
            "message": "全タスク完了 — 振り返りを自動生成中...",
        });
        ws_hub.broadcast(sprint_id, &sprint_ws_msg.to_string()).await;

        crate::scanner::analyzer::run_sprint_retrospective_only(&pool, &ws_hub, sprint_id).await;
    });
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

    // PR作成済みタスク（revision_count > 0 または pr_url あり）はワークツリーを保持
    let should_keep = match task_service::get_task(pool, task_id).await {
        Ok(t) => t.pr_url.is_some(),
        Err(_) => false,
    };
    if !should_keep {
        let _ = worktree::cleanup_worktree(repo_path, worktree_dir).await;
    }

    ws_hub.remove_channel(&task_id).await;
    send_task_fact(pool, task_id, "failed").await;
    check_sprint_auto_transition(pool, ws_hub, task_id).await;
}

/// タスク完了/失敗時に Factrail へ Fact を送信
async fn send_task_fact(pool: &PgPool, task_id: Uuid, status: &str) {
    let client = match factrail::global() {
        Some(c) => c,
        None => return,
    };

    let task = match task_service::get_task(pool, task_id).await {
        Ok(t) => t,
        Err(_) => return,
    };

    let fact_type = if status == "completed" {
        "task_completed"
    } else {
        "task_failed"
    };

    let source_url = task.pr_url.as_deref()
        .or(task.issue_url.as_deref())
        .unwrap_or("");

    let fact = serde_json::json!({
        "source": "ai-dev-team",
        "type": fact_type,
        "title": format!("[{}] {}", status.to_uppercase(), task.title),
        "summary": task.description,
        "metadata": {
            "task_id": task.id.to_string(),
            "project_id": task.project_id.to_string(),
            "proposal_type": task.proposal_type,
            "pr_url": task.pr_url,
            "issue_url": task.issue_url,
            "issue_number": task.issue_number,
            "diff_stats": task.diff_stats,
            "revision_count": task.revision_count,
            "error_log": task.error_log,
        },
        "raw": {
            "task_id": task.id.to_string(),
            "status": status,
        },
        "sourceUrl": source_url,
        "externalId": format!("ai-dev-team:task:{}", task.id),
    });

    if let Err(e) = client.send_fact(&fact).await {
        tracing::warn!("Failed to send fact to Factrail: {e}");
    } else {
        tracing::info!("Fact sent to Factrail: task {task_id} {status}");
    }
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
    "REQUEST_CHANGES".to_string() // デフォルトは安全側（パース失敗時）
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
    "FAIL".to_string() // デフォルトは安全側（パース失敗時）
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

fn parse_qa_verdict(output: &str) -> String {
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
    "FAIL".to_string() // デフォルトは安全側（パース失敗時）
}

/// フロントエンド関連の変更があるか判定
fn has_frontend_changes(file_list: &str) -> bool {
    file_list.lines().any(|line| {
        let line = line.trim();
        line.starts_with("frontend/")
            || line.ends_with(".tsx")
            || line.ends_with(".jsx")
            || line.ends_with(".css")
            || line.ends_with(".html")
    })
}

/// git diff --name-only + git ls-files --others で変更ファイル一覧を取得
async fn get_changed_file_list(worktree_path: &str) -> String {
    let tracked = tokio::process::Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .await;

    let untracked = tokio::process::Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(worktree_path)
        .output()
        .await;

    let mut files = String::new();
    if let Ok(o) = tracked {
        files.push_str(&String::from_utf8_lossy(&o.stdout));
    }
    if let Ok(o) = untracked {
        files.push_str(&String::from_utf8_lossy(&o.stdout));
    }
    files
}

/// スクリーンショットディレクトリからファイル名一覧を収集
async fn collect_screenshots(dir: &str) -> Vec<String> {
    let mut screenshots = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg") {
                screenshots.push(name);
            }
        }
    }
    screenshots.sort();
    screenshots
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // --- truncate ---

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_multibyte() {
        // "あいう" = 9 bytes (3 bytes each)
        let s = "あいう";
        let result = truncate(s, 4);
        assert_eq!(result, "あ"); // 3 bytes boundary
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate("", 10), "");
    }

    // --- parse_verdict ---

    #[test]
    fn test_parse_verdict_approve() {
        let output = "Some review text\nVERDICT: APPROVE\n";
        assert_eq!(parse_verdict(output), "APPROVE");
    }

    #[test]
    fn test_parse_verdict_request_changes() {
        let output = "Some review text\nVERDICT: REQUEST_CHANGES\n";
        assert_eq!(parse_verdict(output), "REQUEST_CHANGES");
    }

    #[test]
    fn test_parse_verdict_case_insensitive() {
        let output = "verdict: approve";
        assert_eq!(parse_verdict(output), "APPROVE");
    }

    #[test]
    fn test_parse_verdict_default_safe_side() {
        let output = "no verdict here";
        assert_eq!(parse_verdict(output), "REQUEST_CHANGES");
    }

    #[test]
    fn test_parse_verdict_empty() {
        assert_eq!(parse_verdict(""), "REQUEST_CHANGES");
    }

    // --- parse_test_verdict ---

    #[test]
    fn test_parse_test_verdict_pass() {
        let output = "All tests passed\nVERDICT: PASS\n";
        assert_eq!(parse_test_verdict(output), "PASS");
    }

    #[test]
    fn test_parse_test_verdict_fail() {
        let output = "Tests failed\nVERDICT: FAIL\n";
        assert_eq!(parse_test_verdict(output), "FAIL");
    }

    #[test]
    fn test_parse_test_verdict_default_safe_side() {
        let output = "no verdict";
        assert_eq!(parse_test_verdict(output), "FAIL");
    }

    // --- parse_qa_verdict ---

    #[test]
    fn test_parse_qa_verdict_pass() {
        let output = "QA check OK\nVERDICT: PASS";
        assert_eq!(parse_qa_verdict(output), "PASS");
    }

    #[test]
    fn test_parse_qa_verdict_fail() {
        let output = "QA issues found\nVERDICT: FAIL";
        assert_eq!(parse_qa_verdict(output), "FAIL");
    }

    #[test]
    fn test_parse_qa_verdict_default_safe_side() {
        assert_eq!(parse_qa_verdict(""), "FAIL");
    }

    // --- has_frontend_changes ---

    #[test]
    fn test_has_frontend_changes_true() {
        assert!(has_frontend_changes("frontend/src/app/page.tsx\nbackend/src/main.rs"));
        assert!(has_frontend_changes("some/file.tsx"));
        assert!(has_frontend_changes("styles.css"));
        assert!(has_frontend_changes("index.html"));
    }

    #[test]
    fn test_has_frontend_changes_false() {
        assert!(!has_frontend_changes("backend/src/main.rs\nbackend/Cargo.toml"));
        assert!(!has_frontend_changes(""));
    }

    // --- build_pr_body ---

    #[test]
    fn test_build_pr_body_minimal() {
        let body = build_pr_body("fix bug", "", None);
        assert!(body.contains("## 概要"));
        assert!(body.contains("fix bug"));
        assert!(!body.contains("## 変更内容"));
    }

    #[test]
    fn test_build_pr_body_with_diff_stats() {
        let body = build_pr_body("fix bug", " 2 files changed\n", None);
        assert!(body.contains("## 変更内容"));
        assert!(body.contains("2 files changed"));
    }

    #[test]
    fn test_build_pr_body_with_session() {
        let session = ExecutionSession {
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            attempt: 1,
            status: "completed".to_string(),
            worktree_path: None,
            branch_name: None,
            plan_output: None,
            review_output: Some("Looks good".to_string()),
            review_verdict: Some("APPROVE".to_string()),
            test_output: Some("All 5 tests passed".to_string()),
            test_passed: Some(true),
            qa_output: None,
            qa_passed: Some(true),
            qa_screenshots: None,
            revision_instructions: None,
            started_at: Utc::now(),
            completed_at: None,
        };

        let body = build_pr_body("new feature", "1 file changed", Some(&session));
        assert!(body.contains("テスト: **PASS**"));
        assert!(body.contains("QA: **PASS**"));
        assert!(body.contains("Review: **APPROVE**"));
    }

    // --- build_dod_section ---

    #[test]
    fn test_build_dod_section_none() {
        assert_eq!(build_dod_section(&None), "");
    }

    #[test]
    fn test_build_dod_section_empty() {
        assert_eq!(build_dod_section(&Some("  ".to_string())), "");
    }

    #[test]
    fn test_build_dod_section_with_content() {
        let result = build_dod_section(&Some("- API returns 200".to_string()));
        assert!(result.contains("完了条件"));
        assert!(result.contains("API returns 200"));
    }
}
