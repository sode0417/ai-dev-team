use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

/// プロセス全体で単一の Mutex。git fetch / worktree add / worktree prune など
/// .git/config.lock を取得する操作をシリアライズし、並列実行時のロック競合を防ぐ。
static GIT_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// git worktree を作成して分離された作業ディレクトリを返す
pub async fn create_worktree(
    repo_path: &str,
    task_id: Uuid,
    base_branch: &str,
) -> Result<(PathBuf, String), String> {
    let worktree_dir = Path::new(repo_path)
        .join(".worktrees")
        .join(format!("task-{}", task_id));
    let branch_name = format!("task/{}", task_id);

    // .worktrees ディレクトリを作成（ロック不要）
    let parent = worktree_dir.parent()
        .ok_or_else(|| "worktree path has no parent directory".to_string())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| format!("Failed to create worktrees dir: {e}"))?;

    // git 操作を Mutex でシリアライズ（.git/config.lock 競合防止）
    {
        let _guard = GIT_MUTEX.lock().await;
        tracing::info!(task_id = %task_id, "git mutex acquired for worktree creation");

        // まず最新を fetch
        let fetch = Command::new("git")
            .args(["fetch", "origin", base_branch])
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| format!("git fetch failed: {e}"))?;

        if !fetch.status.success() {
            tracing::warn!(
                "git fetch warning: {}",
                String::from_utf8_lossy(&fetch.stderr)
            );
        }

        // worktree 作成（新ブランチ）
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &branch_name,
                worktree_dir.to_str()
                    .ok_or_else(|| "worktree path contains non-UTF-8 characters".to_string())?,
                &format!("origin/{base_branch}"),
            ])
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| format!("git worktree add failed: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "git worktree add failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        tracing::info!(task_id = %task_id, "git mutex released after worktree creation");
    }

    Ok((worktree_dir, branch_name))
}

/// git worktree を削除
pub async fn cleanup_worktree(repo_path: &str, worktree_path: &Path) -> Result<(), String> {
    // worktree ディレクトリを削除（ファイルシステム操作のみなのでロック不要）
    if worktree_path.exists() {
        tokio::fs::remove_dir_all(worktree_path)
            .await
            .map_err(|e| format!("Failed to remove worktree dir: {e}"))?;
    }

    // git worktree prune は .git/config.lock を取得するため Mutex で保護
    {
        let _guard = GIT_MUTEX.lock().await;
        tracing::info!("git mutex acquired for worktree prune");

        let output = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| format!("git worktree prune failed: {e}"))?;

        if !output.status.success() {
            tracing::warn!(
                "git worktree prune warning: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    Ok(())
}

/// 変更があるかチェック
pub async fn has_changes(worktree_path: &str) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git status failed: {e}"))?;

    Ok(!output.stdout.is_empty())
}

/// 不要ファイルを除外してステージングする
async fn stage_changes(worktree_path: &str) -> Result<(), String> {
    // tracked の変更ファイル
    let tracked = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git diff --name-only failed: {e}"))?;

    // untracked ファイル
    let untracked = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git ls-files failed: {e}"))?;

    let tracked_str = String::from_utf8_lossy(&tracked.stdout);
    let untracked_str = String::from_utf8_lossy(&untracked.stdout);

    let files: Vec<&str> = tracked_str
        .lines()
        .chain(untracked_str.lines())
        .filter(|f| !f.is_empty())
        .filter(|f| !is_excluded(f))
        .collect();

    if files.is_empty() {
        return Ok(());
    }

    let add = Command::new("git")
        .arg("add")
        .args(&files)
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git add failed: {e}"))?;

    if !add.status.success() {
        return Err(format!(
            "git add failed: {}",
            String::from_utf8_lossy(&add.stderr)
        ));
    }

    Ok(())
}

/// 除外パターンに該当するか判定
fn is_excluded(path: &str) -> bool {
    const EXCLUDED_PATTERNS: &[&str] = &[
        ".playwright-mcp/",
        "data/qa-screenshots/",
        "package-lock.json",
    ];
    EXCLUDED_PATTERNS.iter().any(|pat| {
        if pat.ends_with('/') {
            path.starts_with(pat) || path.contains(&format!("/{pat}"))
        } else {
            path == *pat || path.ends_with(&format!("/{pat}"))
        }
    })
}

/// git commit + push + gh pr create
pub async fn commit_and_create_pr(
    worktree_path: &str,
    branch_name: &str,
    title: &str,
    body: &str,
) -> Result<String, String> {
    // git add（不要ファイル除外）
    stage_changes(worktree_path).await?;

    // git commit
    let commit = Command::new("git")
        .args(["commit", "-m", title])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git commit failed: {e}"))?;
    if !commit.status.success() {
        return Err(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        ));
    }

    // git push
    let push = Command::new("git")
        .args(["push", "-u", "origin", branch_name])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git push failed: {e}"))?;
    if !push.status.success() {
        return Err(format!(
            "git push failed: {}",
            String::from_utf8_lossy(&push.stderr)
        ));
    }

    // gh pr create
    let pr = Command::new("gh")
        .args(["pr", "create", "--title", title, "--body", body])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("gh pr create failed: {e}"))?;
    if !pr.status.success() {
        return Err(format!(
            "gh pr create failed: {}",
            String::from_utf8_lossy(&pr.stderr)
        ));
    }

    let pr_url = String::from_utf8_lossy(&pr.stdout).trim().to_string();

    // GitHub Auto-merge を有効化（Branch Protection 未設定の場合は無視される）
    let auto_merge = Command::new("gh")
        .args([
            "pr",
            "merge",
            &pr_url,
            "--auto",
            "--squash",
            "--delete-branch",
        ])
        .current_dir(worktree_path)
        .output()
        .await;

    match auto_merge {
        Ok(output) if output.status.success() => {
            tracing::info!("Auto-merge enabled for PR: {pr_url}");
        }
        Ok(output) => {
            // auto-merge が有効化できなくても PR 作成自体は成功
            tracing::warn!(
                "Auto-merge could not be enabled (Branch Protection may not be configured): {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            tracing::warn!("Failed to run gh pr merge --auto: {e}");
        }
    }

    Ok(pr_url)
}

/// git commit + push（既存PRに追加コミット、PR作成は行わない）
pub async fn commit_and_push(
    worktree_path: &str,
    branch_name: &str,
    message: &str,
) -> Result<(), String> {
    // git add（不要ファイル除外）
    stage_changes(worktree_path).await?;

    // git commit
    let commit = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git commit failed: {e}"))?;
    if !commit.status.success() {
        return Err(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        ));
    }

    // git push
    let push = Command::new("git")
        .args(["push", "origin", branch_name])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git push failed: {e}"))?;
    if !push.status.success() {
        return Err(format!(
            "git push failed: {}",
            String::from_utf8_lossy(&push.stderr)
        ));
    }

    Ok(())
}

/// PR がクローズ済み（MERGED or CLOSED）かを確認
pub async fn check_pr_closed_or_merged(pr_url: &str) -> Result<bool, String> {
    let output = Command::new("gh")
        .args(["pr", "view", pr_url, "--json", "state"])
        .output()
        .await
        .map_err(|e| format!("gh pr view failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "gh pr view failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("MERGED") || stdout.contains("CLOSED"))
}

/// commit 前の diff 統計情報を取得（staged + unstaged）
pub async fn get_diff_stats_unstaged(worktree_path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["diff", "--stat", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git diff --stat failed: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// diff の統計情報を取得
pub async fn get_diff_stats(worktree_path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["diff", "--stat", "HEAD~1"])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git diff --stat failed: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// 変更されたファイル一覧を取得
pub async fn get_changed_files(worktree_path: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD~1"])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("git diff --name-only failed: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_excluded_playwright() {
        assert!(is_excluded(".playwright-mcp/some-file"));
        assert!(is_excluded("path/to/.playwright-mcp/file"));
    }

    #[test]
    fn test_is_excluded_qa_screenshots() {
        assert!(is_excluded("data/qa-screenshots/img.png"));
    }

    #[test]
    fn test_is_excluded_package_lock() {
        assert!(is_excluded("package-lock.json"));
        assert!(is_excluded("frontend/package-lock.json"));
    }

    #[test]
    fn test_is_excluded_normal_files() {
        assert!(!is_excluded("src/main.rs"));
        assert!(!is_excluded("frontend/src/app/page.tsx"));
        assert!(!is_excluded("package.json"));
        assert!(!is_excluded("Cargo.lock"));
    }
}
