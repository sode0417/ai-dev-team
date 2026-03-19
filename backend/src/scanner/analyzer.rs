use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use sqlx::PgPool;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::domains::projects::model::ProjectRepository;
use crate::domains::scans::model::ScanAnalysisOutput;
use crate::domains::scans::service as scan_service;
use crate::domains::sprints::service as sprint_service;
use crate::domains::tasks::model::TaskStatus;
use crate::executor::claude_cli;
use crate::github::GitHubClient;
use crate::ws::WsHub;

/// マルチバイト文字境界を考慮して文字列を切り詰める
fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    match s.char_indices().take_while(|(i, _)| *i <= max_bytes).last() {
        Some((i, c)) => &s[..i + c.len_utf8()],
        None => s,
    }
}

/// スキャン実行のメインエントリポイント（バックグラウンドで呼ばれる）
/// sprint_id を scan_id としても使用（scan_sessions テーブルとの互換性維持）
pub async fn run_scan(
    pool: &PgPool,
    ws_hub: &WsHub,
    github: &GitHubClient,
    project_id: Uuid,
    sprint_id: Uuid,
    project_name: &str,
    project_description: Option<&str>,
    repositories: &[ProjectRepository],
) {
    let result = run_scan_inner(
        pool, ws_hub, github, project_id, sprint_id,
        project_name, project_description, repositories,
    ).await;

    if let Err(e) = result {
        tracing::error!("Scan failed: {e}");
        let _ = sprint_service::fail_sprint(pool, sprint_id, &e).await;
        broadcast_progress(ws_hub, sprint_id, "error", &format!("スキャン失敗: {e}")).await;
    }
}

async fn run_scan_inner(
    pool: &PgPool,
    ws_hub: &WsHub,
    github: &GitHubClient,
    project_id: Uuid,
    sprint_id: Uuid,
    project_name: &str,
    project_description: Option<&str>,
    repositories: &[ProjectRepository],
) -> Result<(), String> {
    // 1. データ収集
    broadcast_progress(ws_hub, sprint_id, "collecting", "リポジトリデータを収集中...").await;

    let mut repo_sections = Vec::new();
    let mut repo_lookup: HashMap<String, Uuid> = HashMap::new();

    for repo in repositories {
        repo_lookup.insert(repo.name.clone(), repo.id);

        let section = collect_repo_data(github, &repo.owner, &repo.name).await;
        repo_sections.push(section);

        broadcast_progress(
            ws_hub, sprint_id, "collecting",
            &format!("{}/{} のデータ収集完了", repo.owner, repo.name),
        ).await;
    }

    // 2. 振り返りデータ収集（前回スプリント含む）
    broadcast_progress(ws_hub, sprint_id, "retrospective", "過去のスプリント・タスク実行結果を収集中...").await;

    let retro_section = collect_retrospective_data(pool, project_id).await;

    // 3. プロンプト構築
    broadcast_progress(ws_hub, sprint_id, "analyzing", "Claude で分析中...").await;

    let prompt = build_scan_prompt(
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
    broadcast_progress(ws_hub, sprint_id, "parsing", "分析結果をパース中...").await;

    let output = parse_analysis_output(&result.stdout)?;

    // 6. タスク作成 (sprint_id を紐付け)
    broadcast_progress(ws_hub, sprint_id, "creating_tasks", "タスクを作成中...").await;

    let priority_actions = serde_json::to_value(&output.priority_actions)
        .unwrap_or_default();

    // scan_sessions にも保存（後方互換）
    let scan = scan_service::create_scan(pool, project_id).await
        .map_err(|e| format!("Failed to create scan session: {e}"))?;

    let _tasks = scan_service::create_tasks_from_proposals(
        pool, project_id, scan.id, &output.task_proposals, &repo_lookup, Some(sprint_id),
    )
    .await
    .map_err(|e| format!("Failed to create tasks: {e}"))?;

    let improvement_suggestions = serde_json::to_value(&output.improvement_suggestions).ok();

    // scan_sessions 更新
    let _ = scan_service::update_scan_completed(
        pool, scan.id, &output.summary, &priority_actions,
        output.retrospective.as_deref(), improvement_suggestions.as_ref(),
    ).await;

    // スプリントのスキャン結果を更新
    sprint_service::update_scan_completed(pool, sprint_id, &output.summary, &priority_actions)
        .await
        .map_err(|e| format!("Failed to update sprint: {e}"))?;

    broadcast_progress(
        ws_hub, sprint_id, "completed",
        &format!("スキャン完了: {}件のタスクを提案", output.task_proposals.len()),
    ).await;

    Ok(())
}

/// スプリント計画: PM Agent が実行順序を決定
pub async fn run_sprint_planning(pool: &PgPool, ws_hub: &WsHub, sprint_id: Uuid) {
    let result = run_sprint_planning_inner(pool, ws_hub, sprint_id).await;

    if let Err(e) = result {
        tracing::error!("Sprint planning failed: {e}");
        let _ = sprint_service::fail_sprint(pool, sprint_id, &e).await;
        broadcast_progress(ws_hub, sprint_id, "error", &format!("計画失敗: {e}")).await;
    }
}

async fn run_sprint_planning_inner(
    pool: &PgPool,
    ws_hub: &WsHub,
    sprint_id: Uuid,
) -> Result<(), String> {
    broadcast_progress(ws_hub, sprint_id, "planning", "実行計画を作成中...").await;

    let sprint = sprint_service::get_sprint_with_tasks(pool, sprint_id)
        .await
        .map_err(|e| format!("Failed to get sprint: {e}"))?;

    // awaiting_approval のタスクのみ対象
    let ready_tasks: Vec<_> = sprint.tasks.iter()
        .filter(|t| t.status == TaskStatus::AwaitingApproval)
        .collect();

    if ready_tasks.is_empty() {
        return Err("No tasks ready for planning".to_string());
    }

    // タスク情報をプロンプトに
    let tasks_info: Vec<String> = ready_tasks.iter().enumerate().map(|(i, t)| {
        let plan_summary = t.plan.as_deref().unwrap_or("計画なし");
        let plan_short = truncate_str(plan_summary, 300);
        format!(
            "{}. [{}] {} (priority: {:?})\n   説明: {}\n   計画概要: {}",
            i + 1, t.id, t.title, t.priority, t.description, plan_short
        )
    }).collect();

    let prompt = format!(
        r#"あなたは PM Agent です。以下のタスクの実行順序と並列実行グループを決定してください。

## タスク一覧
{}

## 出力指示
以下の情報を含む実行計画を Markdown で出力してください:

1. **実行順序**: タスクの最適な実行順序とその理由
2. **依存関係**: タスク間の依存関係（あれば）
3. **並列実行グループ**: 同時実行可能なタスクのグループ分け
4. **リスク**: 注意すべきリスクや並行実行できないもの
5. **見積もり**: 全体の実行時間の概算

タスク ID と実行順序・並列グループの対応を以下の JSON も最後に含めてください:
```json
[{{"task_id": "uuid", "order": 1, "execution_group": 0}}, ...]
```

### execution_group のルール:
- 同じ `execution_group` 値のタスクは並列実行される
- 小さいグループ番号から順に実行（group 0 → group 1 → ...）
- **依存関係がないタスクは同じグループ**に配置（並列実行で時間短縮）
- **依存先があるタスクは後のグループ**に配置
- **同一ファイルを変更するタスクは別グループ**に配置（衝突回避）"#,
        tasks_info.join("\n\n")
    );

    let working_dir = "/tmp";
    let result = claude_cli::run_claude(&prompt, working_dir, 180)
        .await
        .map_err(|e| format!("Claude CLI error: {e}"))?;

    if result.exit_code != 0 {
        return Err(format!("Planning failed: {}", result.stderr));
    }

    let plan = result.stdout.clone();

    // タスクの execution_order と execution_group を更新
    if let Some(orders) = extract_task_orders(&result.stdout) {
        for (task_id, order, group) in orders {
            let _ = sqlx::query(
                "UPDATE tasks SET execution_order = $2, execution_group = $3, updated_at = NOW() WHERE id = $1"
            )
            .bind(task_id)
            .bind(order)
            .bind(group)
            .execute(pool)
            .await;
        }
    }

    // スプリントに計画を保存（status は planning のまま、ユーザー承認待ち）
    sqlx::query(
        "UPDATE sprints SET execution_plan = $2 WHERE id = $1"
    )
    .bind(sprint_id)
    .bind(&plan)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to save plan: {e}"))?;

    broadcast_progress(ws_hub, sprint_id, "plan_ready", "実行計画が完成しました。承認をお願いします。").await;

    Ok(())
}

/// スプリント実行: タスクを順次実行
pub async fn run_sprint_execution(pool: &PgPool, ws_hub: &WsHub, sprint_id: Uuid) {
    let result = run_sprint_execution_inner(pool, ws_hub, sprint_id).await;

    if let Err(e) = result {
        tracing::error!("Sprint execution failed: {e}");
        let _ = sprint_service::fail_sprint(pool, sprint_id, &e).await;
        broadcast_progress(ws_hub, sprint_id, "error", &format!("実行失敗: {e}")).await;
    }
}

async fn run_sprint_execution_inner(
    pool: &PgPool,
    ws_hub: &WsHub,
    sprint_id: Uuid,
) -> Result<(), String> {
    let sprint = sprint_service::get_sprint(pool, sprint_id)
        .await
        .map_err(|e| format!("Failed to get sprint: {e}"))?;

    let tasks = sprint_service::get_sprint_tasks(pool, sprint_id)
        .await
        .map_err(|e| format!("Failed to get tasks: {e}"))?;

    let executable: Vec<_> = tasks.into_iter()
        .filter(|t| t.status == TaskStatus::AwaitingApproval)
        .collect();

    let total = executable.len();

    // execution_group でグループ化
    let mut groups: BTreeMap<i32, Vec<_>> = BTreeMap::new();
    for task in executable {
        groups.entry(task.execution_group).or_default().push(task);
    }

    let max_parallel = sprint.max_parallel_tasks.max(1) as usize;
    let semaphore = Arc::new(Semaphore::new(max_parallel));
    let mut completed_count = 0usize;

    for (group_id, group_tasks) in &groups {
        let group_size = group_tasks.len();

        if group_size > 1 {
            broadcast_progress(
                ws_hub, sprint_id, "executing",
                &format!("Group {group_id}: {group_size}タスク並列実行中 ({completed_count}/{total} 完了済み)"),
            ).await;
        } else {
            broadcast_progress(
                ws_hub, sprint_id, "executing",
                &format!("タスク実行中 ({}/{total}): {}", completed_count + 1, group_tasks[0].title),
            ).await;
        }

        let mut handles = vec![];

        for task in group_tasks {
            // リポジトリ情報取得
            let repo_id = match task.repository_id {
                Some(id) => id,
                None => {
                    tracing::warn!("Task {} has no repository, skipping", task.id);
                    continue;
                }
            };

            let repo: Option<ProjectRepository> = sqlx::query_as(
                "SELECT id, project_id, owner, name, default_branch, local_path, created_at \
                 FROM project_repositories WHERE id = $1",
            )
            .bind(repo_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("DB error: {e}"))?;

            let local_path = match repo.and_then(|r| r.local_path) {
                Some(p) => p,
                None => {
                    tracing::warn!("Task {} repository has no local_path, skipping", task.id);
                    continue;
                }
            };

            // タスクの計画承認
            let _ = crate::domains::tasks::service::approve_plan(pool, task.id).await;

            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let pool = pool.clone();
            let ws_hub = ws_hub.clone();
            let task_id = task.id;
            let task_title = task.title.clone();
            let task_description = task.description.clone();
            let proposal_type = task.proposal_type.clone();

            let handle = tokio::spawn(async move {
                // 実行フェーズ（各タスクは独立 worktree で動作）
                crate::executor::pipeline::run_execution_phase(
                    &pool, &ws_hub, task_id, &task_title, &task_description,
                    &local_path, &proposal_type,
                ).await;
                drop(permit);
                task_id
            });
            handles.push((task.id, task.title.clone(), handle));
        }

        // グループ内全タスク完了を待機
        for (task_id, task_title, handle) in handles {
            let _ = handle.await;
            completed_count += 1;

            // 実行結果を確認
            let updated_task = crate::domains::tasks::service::get_task(pool, task_id)
                .await
                .map_err(|e| format!("Failed to get task: {e}"))?;

            let status_icon = match updated_task.status {
                TaskStatus::Completed => "✅",
                TaskStatus::Failed => "❌",
                _ => "⏳",
            };

            broadcast_progress(
                ws_hub, sprint_id, "task_done",
                &format!("{status_icon} ({completed_count}/{total}) {task_title} — {:?}", updated_task.status),
            ).await;
        }
    }

    // 全タスク完了 → 振り返りフェーズ
    broadcast_progress(ws_hub, sprint_id, "generating_retro", "振り返りを生成中...").await;

    run_retrospective(pool, ws_hub, sprint_id).await?;

    broadcast_progress(ws_hub, sprint_id, "retrospective", "スプリント完了。フィードバックをお願いします。").await;

    Ok(())
}

/// 個別タスク実行完了後の自動振り返り（スプリント実行フローを経由しないケース）
pub async fn run_sprint_retrospective_only(pool: &PgPool, ws_hub: &WsHub, sprint_id: Uuid) {
    let result = run_retrospective(pool, ws_hub, sprint_id).await;

    match result {
        Ok(()) => {
            broadcast_progress(ws_hub, sprint_id, "retrospective", "スプリント完了。フィードバックをお願いします。").await;
        }
        Err(e) => {
            tracing::error!("Auto-retrospective failed for sprint {sprint_id}: {e}");
            // retrospective 生成に失敗しても、ステータスだけ retrospective に遷移
            let _ = sprint_service::save_retrospective(
                pool, sprint_id,
                &format!("振り返り自動生成に失敗しました: {e}"),
                None,
            ).await;
            broadcast_progress(ws_hub, sprint_id, "retrospective", "振り返り生成に失敗しましたが、フィードバックを入力できます。").await;
        }
    }
}

/// 振り返り生成
async fn run_retrospective(
    pool: &PgPool,
    _ws_hub: &WsHub,
    sprint_id: Uuid,
) -> Result<(), String> {
    let sprint = sprint_service::get_sprint_with_tasks(pool, sprint_id)
        .await
        .map_err(|e| format!("Failed to get sprint: {e}"))?;

    let task_results: Vec<String> = sprint.tasks.iter()
        .filter(|t| t.status != TaskStatus::Cancelled && t.status != TaskStatus::Proposed)
        .map(|t| {
            let status = match t.status {
                TaskStatus::Completed => "成功",
                TaskStatus::Failed => "失敗",
                _ => "未完了",
            };
            let pr = t.pr_url.as_deref().unwrap_or("PR なし");
            let err = t.error_log.as_deref().map(|e| {
                let short = truncate_str(e, 200);
                format!("\n   エラー: {short}")
            }).unwrap_or_default();
            format!("- [{}] {} → {}{}", status, t.title, pr, err)
        })
        .collect();

    let prompt = format!(
        r#"あなたは PM Agent です。スプリントの振り返りを行ってください。

## スプリント分析
{}

## タスク実行結果
{}

## 出力指示
以下を含む振り返りを Markdown で出力してください:
1. **成果**: 何が達成できたか
2. **課題**: 何がうまくいかなかったか
3. **改善点**: 次のスプリントに活かすべきこと
4. **提案**: プロンプトやプロセスの改善提案"#,
        sprint.sprint.scan_analysis.as_deref().unwrap_or("分析なし"),
        task_results.join("\n"),
    );

    let result = claude_cli::run_claude(&prompt, "/tmp", 120)
        .await
        .map_err(|e| format!("Retrospective generation failed: {e}"))?;

    let retrospective = if result.exit_code == 0 {
        result.stdout
    } else {
        format!("振り返り生成に失敗: {}", result.stderr)
    };

    sprint_service::save_retrospective(pool, sprint_id, &retrospective, None)
        .await
        .map_err(|e| format!("Failed to save retrospective: {e}"))?;

    Ok(())
}

/// 改善フェーズ実行
pub async fn run_improving_phase(
    pool: &PgPool,
    ws_hub: &WsHub,
    github: &GitHubClient,
    sprint_id: Uuid,
) {
    let result = run_improving_phase_inner(pool, ws_hub, github, sprint_id).await;

    match result {
        Ok(()) => {
            broadcast_progress(ws_hub, sprint_id, "improving_done", "改善フェーズが完了しました。結果を確認してください。").await;
        }
        Err(e) => {
            tracing::error!("Improving phase failed for sprint {sprint_id}: {e}");
            // エラーでも結果を保存して improving_done に
            let error_result = serde_json::json!([{
                "target": "error",
                "description": format!("改善フェーズでエラーが発生: {e}"),
                "status": "failed",
                "pr_url": null,
                "issue_url": null,
                "error": format!("{e}"),
            }]);
            let _ = sprint_service::save_improvement_results(pool, sprint_id, &error_result).await;
            broadcast_progress(ws_hub, sprint_id, "improving_done", &format!("改善フェーズでエラーが発生しましたが、スプリントを完了できます: {e}")).await;
        }
    }
}

async fn run_improving_phase_inner(
    pool: &PgPool,
    ws_hub: &WsHub,
    github: &GitHubClient,
    sprint_id: Uuid,
) -> Result<(), String> {
    let sprint = sprint_service::get_sprint(pool, sprint_id)
        .await
        .map_err(|e| format!("Failed to get sprint: {e}"))?;

    let suggestions = sprint
        .improvement_suggestions
        .as_ref()
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if suggestions.is_empty() {
        return Ok(());
    }

    // プロジェクトのリポジトリ情報を取得
    let repos: Vec<crate::domains::projects::model::ProjectRepository> = sqlx::query_as(
        "SELECT id, project_id, owner, name, default_branch, local_path, created_at \
         FROM project_repositories WHERE project_id = $1",
    )
    .bind(sprint.project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to get repositories: {e}"))?;

    // Issue 作成先のリポジトリ（最初のリポジトリを使用）
    let (issue_owner, issue_repo) = repos
        .first()
        .map(|r| (r.owner.as_str(), r.name.as_str()))
        .unwrap_or(("sode0417", "ai-dev-team"));

    let mut results: Vec<serde_json::Value> = Vec::new();

    for (i, suggestion) in suggestions.iter().enumerate() {
        let target = suggestion["target"].as_str().unwrap_or("unknown");
        let description = suggestion["description"].as_str().unwrap_or("");
        let reason = suggestion["reason"].as_str().unwrap_or("");

        broadcast_progress(
            ws_hub, sprint_id, "improving",
            &format!("改善 {}/{}: {} — {}", i + 1, suggestions.len(), target, description),
        ).await;

        let result = match target {
            "CLAUDE.md" => {
                process_claudemd_improvement(
                    ws_hub, sprint_id, &repos, description, reason,
                ).await
            }
            "planner_prompt" | "reviewer_prompt" | "test_prompt" => {
                process_prompt_improvement(
                    github, issue_owner, issue_repo, target, description, reason, sprint_id,
                ).await
            }
            _ => {
                // 未知のターゲットは GitHub Issue にする
                process_prompt_improvement(
                    github, issue_owner, issue_repo, target, description, reason, sprint_id,
                ).await
            }
        };

        results.push(result);
    }

    // 結果を DB に保存
    let results_json = serde_json::to_value(&results)
        .map_err(|e| format!("Failed to serialize results: {e}"))?;
    sprint_service::save_improvement_results(pool, sprint_id, &results_json)
        .await
        .map_err(|e| format!("Failed to save improvement results: {e}"))?;

    Ok(())
}

/// CLAUDE.md 改善: Claude CLI で worktree 上で編集 → PR 作成
async fn process_claudemd_improvement(
    ws_hub: &WsHub,
    sprint_id: Uuid,
    repos: &[crate::domains::projects::model::ProjectRepository],
    description: &str,
    reason: &str,
) -> serde_json::Value {
    let mut pr_urls: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for repo in repos {
        let local_path = match &repo.local_path {
            Some(p) => p.clone(),
            None => continue,
        };

        let base_branch = repo.default_branch.clone();
        let improvement_id = Uuid::new_v4();

        // worktree 作成
        let (worktree_dir, branch_name) = match crate::executor::worktree::create_worktree(
            &local_path, improvement_id, &base_branch,
        ).await {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{}/{}: worktree作成失敗: {e}", repo.owner, repo.name));
                continue;
            }
        };

        let worktree_path = worktree_dir.to_str().unwrap_or("").to_string();

        // Claude CLI で CLAUDE.md を編集
        let prompt = format!(
            "以下の改善点を CLAUDE.md に反映してください。変更は最小限にしてください。\n\n\
             改善点: {description}\n根拠: {reason}\n\n\
             CLAUDE.md ファイルが存在しない場合は何もしないでください。"
        );

        let cli_result = claude_cli::run_claude_autonomous(&prompt, &worktree_path, 120).await;

        match cli_result {
            Ok(result) if result.exit_code == 0 => {
                // 変更があるか確認
                match crate::executor::worktree::has_changes(&worktree_path).await {
                    Ok(true) => {
                        let desc_short: String = description.chars().take(50).collect();
                        let pr_title = format!("[改善] CLAUDE.md: {}", desc_short);
                        let pr_body = format!(
                            "## 改善内容\n{description}\n\n## 根拠\n{reason}\n\n---\nスプリント {sprint_id} の振り返りから自動生成"
                        );

                        match crate::executor::worktree::commit_and_create_pr(
                            &worktree_path, &branch_name, &pr_title, &pr_body,
                        ).await {
                            Ok(url) => {
                                pr_urls.push(url.clone());
                                broadcast_progress(ws_hub, sprint_id, "improving",
                                    &format!("PR作成完了: {url}")).await;
                            }
                            Err(e) => errors.push(format!("{}/{}: PR作成失敗: {e}", repo.owner, repo.name)),
                        }
                    }
                    Ok(false) => {
                        // 変更なし — スキップ
                    }
                    Err(e) => errors.push(format!("{}/{}: 変更確認失敗: {e}", repo.owner, repo.name)),
                }
            }
            Ok(result) => {
                errors.push(format!("{}/{}: Claude CLI failed (exit {}): {}",
                    repo.owner, repo.name, result.exit_code, result.stderr));
            }
            Err(e) => {
                errors.push(format!("{}/{}: Claude CLI error: {e}", repo.owner, repo.name));
            }
        }

        // worktree クリーンアップ
        let _ = crate::executor::worktree::cleanup_worktree(&local_path, &worktree_dir).await;
    }

    if !pr_urls.is_empty() {
        serde_json::json!({
            "target": "CLAUDE.md",
            "description": description,
            "status": "applied",
            "pr_url": pr_urls.join(", "),
            "issue_url": null,
            "error": if errors.is_empty() { None } else { Some(errors.join("; ")) },
        })
    } else if errors.is_empty() {
        serde_json::json!({
            "target": "CLAUDE.md",
            "description": description,
            "status": "skipped",
            "pr_url": null,
            "issue_url": null,
            "error": "変更対象なし",
        })
    } else {
        serde_json::json!({
            "target": "CLAUDE.md",
            "description": description,
            "status": "failed",
            "pr_url": null,
            "issue_url": null,
            "error": errors.join("; "),
        })
    }
}

/// プロンプト改善: GitHub Issue を作成
async fn process_prompt_improvement(
    github: &GitHubClient,
    owner: &str,
    repo_name: &str,
    target: &str,
    description: &str,
    reason: &str,
    sprint_id: Uuid,
) -> serde_json::Value {
    let desc_short: String = description.chars().take(60).collect();
    let title = format!("[改善提案] {}: {}", target, desc_short);
    let body = format!(
        "## 改善対象\n`{target}`\n\n## 改善内容\n{description}\n\n## 根拠\n{reason}\n\n---\nスプリント `{sprint_id}` の振り返りから自動生成"
    );
    let labels = ["type:improvement", "auto-generated"];

    match github.create_issue(owner, repo_name, &title, &body, &labels).await {
        Ok(issue) => {
            serde_json::json!({
                "target": target,
                "description": description,
                "status": "applied",
                "pr_url": null,
                "issue_url": issue.html_url,
                "error": null,
            })
        }
        Err(e) => {
            serde_json::json!({
                "target": target,
                "description": description,
                "status": "failed",
                "pr_url": null,
                "issue_url": null,
                "error": format!("GitHub Issue 作成失敗: {e}"),
            })
        }
    }
}

/// 実行順序・並列グループ JSON を抽出
fn extract_task_orders(output: &str) -> Option<Vec<(Uuid, i32, i32)>> {
    // ```json [...] ``` ブロックから抽出
    let json_str = if let Some(start) = output.rfind("[{") {
        let end = output[start..].find("]").map(|e| start + e + 1)?;
        &output[start..end]
    } else {
        return None;
    };

    #[derive(serde::Deserialize)]
    struct TaskOrder {
        task_id: String,
        order: i32,
        #[serde(default)]
        execution_group: i32,
    }

    let orders: Vec<TaskOrder> = serde_json::from_str(json_str).ok()?;
    let result: Vec<_> = orders.iter()
        .filter_map(|o| {
            Uuid::parse_str(&o.task_id).ok().map(|id| (id, o.order, o.execution_group))
        })
        .collect();

    if result.is_empty() { None } else { Some(result) }
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

/// 振り返りデータを収集（前回スプリント含む）
async fn collect_retrospective_data(pool: &PgPool, project_id: Uuid) -> String {
    let mut section = String::new();

    // 前回のスプリント振り返り
    if let Ok(Some(last_sprint)) = sprint_service::get_last_completed_sprint(pool, project_id).await {
        section.push_str("### 前回のスプリント振り返り\n");
        if let Some(ref retro) = last_sprint.sprint.retrospective {
            section.push_str(&format!("{retro}\n"));
        }
        if let Some(ref fb) = last_sprint.sprint.user_feedback {
            section.push_str(&format!("\nユーザーフィードバック: {fb}\n"));
        }

        // 前回スプリントのタスク結果
        let completed: Vec<_> = last_sprint.tasks.iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .collect();
        let failed: Vec<_> = last_sprint.tasks.iter()
            .filter(|t| t.status == TaskStatus::Failed)
            .collect();

        if !completed.is_empty() {
            section.push_str("\n完了タスク:\n");
            for t in &completed {
                let pr = t.pr_url.as_deref().unwrap_or("PR なし");
                section.push_str(&format!("- \"{}\" → {}\n", t.title, pr));
            }
        }
        if !failed.is_empty() {
            section.push_str("\n失敗タスク:\n");
            for t in &failed {
                let err = t.error_log.as_deref().unwrap_or("エラーログなし");
                let err_short = truncate_str(err, 200);
                section.push_str(&format!("- \"{}\" — {}\n", t.title, err_short));
            }
        }
    }

    // 前回のスキャン分析（フォールバック）
    if section.is_empty() {
        if let Ok(Some(last_scan)) = scan_service::get_last_scan(pool, project_id).await {
            section.push_str("### 前回のスキャン分析\n");
            if let Some(ref analysis) = last_scan.analysis {
                section.push_str(&format!("{analysis}\n"));
            }
        }
    }

    section
}

fn build_scan_prompt(
    project_name: &str,
    project_description: Option<&str>,
    repo_sections: &[String],
    retro_section: &str,
) -> String {
    let desc = project_description.unwrap_or("説明なし");
    let repos = repo_sections.join("\n");

    let retro_block = if retro_section.is_empty() {
        "（初回スプリント — 過去データなし）".to_string()
    } else {
        retro_section.to_string()
    };

    format!(
        r#"あなたは PM Agent です。プロジェクトの分析と振り返りを行い、次のスプリントのタスク提案を JSON で生成してください。

## プロジェクト: {project_name}
{desc}

## リポジトリ分析
{repos}

## 過去のスプリント振り返り
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
- ユーザーは Issue に簡単なバグ・改善を書いているので、それをタスク化する
- 失敗パターンが繰り返されている場合、improvement タスクとして改善を提案
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

async fn broadcast_progress(ws_hub: &WsHub, id: Uuid, phase: &str, message: &str) {
    let msg = serde_json::json!({
        "sprint_id": id.to_string(),
        "phase": phase,
        "message": message,
    });
    ws_hub.broadcast(id, &msg.to_string()).await;
}
