use std::collections::HashMap;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domains::projects::model::ProjectRepository;
use crate::domains::scans::model::ScanAnalysisOutput;
use crate::domains::scans::service as scan_service;
use crate::executor::claude_cli;
use crate::github::GitHubClient;
use crate::ws::WsHub;

/// スキャン実行のメインエントリポイント（バックグラウンドで呼ばれる）
pub async fn run_scan(
    pool: &PgPool,
    ws_hub: &WsHub,
    github: &GitHubClient,
    project_id: Uuid,
    scan_id: Uuid,
    project_name: &str,
    project_description: Option<&str>,
    repositories: &[ProjectRepository],
) {
    let result = run_scan_inner(
        pool, ws_hub, github, project_id, scan_id,
        project_name, project_description, repositories,
    ).await;

    if let Err(e) = result {
        tracing::error!("Scan failed: {e}");
        let _ = scan_service::update_scan_failed(pool, scan_id, &e).await;
        broadcast_progress(ws_hub, scan_id, "error", &format!("スキャン失敗: {e}")).await;
    }
}

async fn run_scan_inner(
    pool: &PgPool,
    ws_hub: &WsHub,
    github: &GitHubClient,
    project_id: Uuid,
    scan_id: Uuid,
    project_name: &str,
    project_description: Option<&str>,
    repositories: &[ProjectRepository],
) -> Result<(), String> {
    // 1. データ収集
    broadcast_progress(ws_hub, scan_id, "collecting", "リポジトリデータを収集中...").await;

    let mut repo_sections = Vec::new();
    let mut repo_lookup: HashMap<String, Uuid> = HashMap::new();

    for repo in repositories {
        repo_lookup.insert(repo.name.clone(), repo.id);

        let section = collect_repo_data(github, &repo.owner, &repo.name).await;
        repo_sections.push(section);

        broadcast_progress(
            ws_hub, scan_id, "collecting",
            &format!("{}/{} のデータ収集完了", repo.owner, repo.name),
        ).await;
    }

    // 2. 振り返りデータ収集
    broadcast_progress(ws_hub, scan_id, "retrospective", "過去のタスク実行結果を収集中...").await;

    let retro_section = collect_retrospective_data(pool, project_id).await;

    // 3. プロンプト構築
    broadcast_progress(ws_hub, scan_id, "analyzing", "Claude で分析中...").await;

    let prompt = build_prompt(
        project_name,
        project_description,
        &repo_sections,
        &retro_section,
    );

    // 4. Claude CLI 実行
    let working_dir = repositories
        .first()
        .and_then(|r| r.local_path.as_deref())
        .unwrap_or("/tmp");

    let result = claude_cli::run_claude(&prompt, working_dir, 300)
        .await
        .map_err(|e| format!("Claude CLI error: {e}"))?;

    if result.exit_code != 0 {
        return Err(format!(
            "Claude CLI exited with code {}: {}",
            result.exit_code, result.stderr
        ));
    }

    // 5. JSON パース
    broadcast_progress(ws_hub, scan_id, "parsing", "分析結果をパース中...").await;

    let output = parse_analysis_output(&result.stdout)?;

    // 6. タスク作成
    broadcast_progress(ws_hub, scan_id, "creating_tasks", "タスクを作成中...").await;

    let priority_actions = serde_json::to_value(&output.priority_actions)
        .unwrap_or_default();
    let improvement_suggestions = serde_json::to_value(&output.improvement_suggestions).ok();

    let _tasks = scan_service::create_tasks_from_proposals(
        pool, project_id, scan_id, &output.task_proposals, &repo_lookup,
    )
    .await
    .map_err(|e| format!("Failed to create tasks: {e}"))?;

    // 7. スキャンセッション完了更新
    scan_service::update_scan_completed(
        pool,
        scan_id,
        &output.summary,
        &priority_actions,
        output.retrospective.as_deref(),
        improvement_suggestions.as_ref(),
    )
    .await
    .map_err(|e| format!("Failed to update scan: {e}"))?;

    broadcast_progress(
        ws_hub, scan_id, "completed",
        &format!("スキャン完了: {}件のタスクを提案", output.task_proposals.len()),
    ).await;

    Ok(())
}

/// 単一リポジトリのデータを収集してプロンプト用のテキストにまとめる
async fn collect_repo_data(github: &GitHubClient, owner: &str, repo: &str) -> String {
    let mut section = format!("### {owner}/{repo}\n");

    // Issues
    match github.fetch_issues(owner, repo, "open", 1, 30).await {
        Ok(issues) => {
            section.push_str(&format!("#### Open Issues ({})\n", issues.len()));
            for issue in &issues {
                let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
                let label_str = if labels.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", labels.join(", "))
                };
                section.push_str(&format!("- #{} {}{}\n", issue.number, issue.title, label_str));
            }
        }
        Err(e) => section.push_str(&format!("#### Issues: 取得失敗 ({e})\n")),
    }

    // PRs
    match github.fetch_pulls(owner, repo, "open", 1, 20).await {
        Ok(pulls) => {
            section.push_str(&format!("\n#### Open PRs ({})\n", pulls.len()));
            for pr in &pulls {
                section.push_str(&format!(
                    "- #{} {} ({} → {})\n",
                    pr.number, pr.title, pr.head.ref_name, pr.base.ref_name
                ));
            }
        }
        Err(e) => section.push_str(&format!("#### PRs: 取得失敗 ({e})\n")),
    }

    // Commits
    match github.fetch_commits(owner, repo, 20).await {
        Ok(commits) => {
            section.push_str("\n#### 最近のコミット\n");
            for commit in &commits {
                let msg = commit.commit.message.lines().next().unwrap_or("");
                section.push_str(&format!("- {} {}\n", &commit.sha[..7], msg));
            }
        }
        Err(e) => section.push_str(&format!("#### Commits: 取得失敗 ({e})\n")),
    }

    section
}

/// 振り返りデータを収集
async fn collect_retrospective_data(pool: &PgPool, project_id: Uuid) -> String {
    let mut section = String::new();

    // 直近の完了/失敗タスク
    if let Ok(tasks) = scan_service::get_recent_completed_tasks(pool, project_id, 10).await {
        let completed: Vec<_> = tasks.iter().filter(|t| t.status == crate::domains::tasks::model::TaskStatus::Completed).collect();
        let failed: Vec<_> = tasks.iter().filter(|t| t.status == crate::domains::tasks::model::TaskStatus::Failed).collect();

        if !completed.is_empty() {
            section.push_str("### 完了タスク\n");
            for t in &completed {
                let pr = t.pr_url.as_deref().unwrap_or("PR なし");
                let diff = t.diff_stats.as_deref().unwrap_or("");
                section.push_str(&format!("- \"{}\" → {} ({})\n", t.title, pr, diff));
            }
        }

        if !failed.is_empty() {
            section.push_str("\n### 失敗タスク\n");
            for t in &failed {
                let err = t.error_log.as_deref().unwrap_or("エラーログなし");
                // エラーログは最初の200文字に制限
                let err_summary = if err.len() > 200 { &err[..200] } else { err };
                section.push_str(&format!(
                    "- \"{}\" (リトライ{}回) — {}\n",
                    t.title, t.retry_count, err_summary
                ));
            }
        }
    }

    // 前回のスキャン分析
    if let Ok(Some(last_scan)) = scan_service::get_last_scan(pool, project_id).await {
        section.push_str("\n### 前回のスキャン分析\n");
        if let Some(ref analysis) = last_scan.analysis {
            section.push_str(&format!("{analysis}\n"));
        }
        if let Some(ref actions) = last_scan.priority_actions {
            if let Some(arr) = actions.as_array() {
                section.push_str("前回の優先アクション:\n");
                for action in arr {
                    if let Some(s) = action.as_str() {
                        section.push_str(&format!("- {s}\n"));
                    }
                }
            }
        }
    }

    section
}

fn build_prompt(
    project_name: &str,
    project_description: Option<&str>,
    repo_sections: &[String],
    retro_section: &str,
) -> String {
    let desc = project_description.unwrap_or("説明なし");
    let repos = repo_sections.join("\n");

    let retro_block = if retro_section.is_empty() {
        "（初回スキャン — 過去データなし）".to_string()
    } else {
        retro_section.to_string()
    };

    format!(
        r#"あなたは PM Agent です。プロジェクトの分析と振り返りを行い、タスク提案を JSON で生成してください。

## プロジェクト: {project_name}
{desc}

## リポジトリ分析
{repos}

## 過去のタスク実行結果（振り返り）
{retro_block}

## 出力指示
以下の JSON **のみ** を出力してください（説明文や markdown コードブロックは不要）:
{{
  "summary": "全体分析（2-3文）",
  "retrospective": "振り返り分析（失敗パターン、改善点）。初回は null",
  "priority_actions": ["優先アクション1", "..."],
  "task_proposals": [
    {{
      "repository_name": "repo-name",
      "title": "タスクタイトル",
      "description": "何をなぜやるか",
      "priority": "high|medium|low",
      "proposal_type": "development|improvement|investigation|operation",
      "issue_number": null
    }}
  ],
  "improvement_suggestions": [
    {{
      "target": "CLAUDE.md|planner_prompt|reviewer_prompt|test_prompt",
      "description": "何をどう改善すべきか",
      "reason": "振り返りから得た根拠"
    }}
  ]
}}

ルール:
- 失敗パターンが繰り返されている場合、improvement タスクとして改善を提案
- CLAUDE.md の内容が実態と乖離していれば improvement タスクを提案
- investigation は不明点の調査が必要な場合のみ
- operation は Issue クローズ/作成/ラベル整理など GitHub 操作タスク（コード変更なし）
- タスクは具体的かつ実行可能なものに限定（最大8件）
- repository_name は各リポジトリの名前部分（owner は含めない）"#
    )
}

/// Claude の出力から JSON をパース（markdown コードブロックにも対応）
fn parse_analysis_output(stdout: &str) -> Result<ScanAnalysisOutput, String> {
    // まず直接パースを試みる
    if let Ok(output) = serde_json::from_str::<ScanAnalysisOutput>(stdout.trim()) {
        return Ok(output);
    }

    // ```json ... ``` ブロックから抽出
    let json_str = if let Some(start) = stdout.find("```json") {
        let after_marker = &stdout[start + 7..];
        if let Some(end) = after_marker.find("```") {
            after_marker[..end].trim()
        } else {
            after_marker.trim()
        }
    } else if let Some(start) = stdout.find("```") {
        let after_marker = &stdout[start + 3..];
        if let Some(end) = after_marker.find("```") {
            after_marker[..end].trim()
        } else {
            after_marker.trim()
        }
    } else {
        // { から } までを探す
        let start = stdout.find('{').ok_or("No JSON found in output")?;
        let end = stdout.rfind('}').ok_or("No closing brace in output")?;
        &stdout[start..=end]
    };

    serde_json::from_str::<ScanAnalysisOutput>(json_str)
        .map_err(|e| format!("Failed to parse analysis JSON: {e}\nRaw output:\n{stdout}"))
}

async fn broadcast_progress(ws_hub: &WsHub, scan_id: Uuid, phase: &str, message: &str) {
    let msg = serde_json::json!({
        "scan_id": scan_id.to_string(),
        "phase": phase,
        "message": message,
    });
    ws_hub.broadcast(scan_id, &msg.to_string()).await;
}
