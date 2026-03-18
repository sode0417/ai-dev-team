use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug)]
pub struct ClaudeResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// claude -p でプロンプトを実行
pub async fn run_claude(
    prompt: &str,
    working_dir: &str,
    timeout_secs: u64,
) -> Result<ClaudeResult, String> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("text")
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Claude Code CLI のネスト制限回避
    cmd.env_remove("CLAUDECODE");
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd.env("AI_ASSISTANT_TASK", "1");

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        cmd.output(),
    )
    .await
    .map_err(|_| format!("claude -p timed out after {timeout_secs}s"))?
    .map_err(|e| format!("Failed to execute claude: {e}"))?;

    Ok(ClaudeResult {
        stdout: String::from_utf8_lossy(&result.stdout).to_string(),
        stderr: String::from_utf8_lossy(&result.stderr).to_string(),
        exit_code: result.status.code().unwrap_or(-1),
    })
}

/// claude -p で --dangerously-skip-permissions + --mcp-config 付きで実行（QA Agent 用）
pub async fn run_claude_with_mcp(
    prompt: &str,
    working_dir: &str,
    timeout_secs: u64,
    mcp_config_path: &str,
) -> Result<ClaudeResult, String> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("text")
        .arg("--dangerously-skip-permissions")
        .arg("--mcp-config")
        .arg(mcp_config_path)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.env_remove("CLAUDECODE");
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd.env("AI_ASSISTANT_TASK", "1");

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        cmd.output(),
    )
    .await
    .map_err(|_| format!("claude -p (MCP) timed out after {timeout_secs}s"))?
    .map_err(|e| format!("Failed to execute claude with MCP: {e}"))?;

    Ok(ClaudeResult {
        stdout: String::from_utf8_lossy(&result.stdout).to_string(),
        stderr: String::from_utf8_lossy(&result.stderr).to_string(),
        exit_code: result.status.code().unwrap_or(-1),
    })
}

/// claude -p で --dangerously-skip-permissions 付きで実行（無人実行用）
pub async fn run_claude_autonomous(
    prompt: &str,
    working_dir: &str,
    timeout_secs: u64,
) -> Result<ClaudeResult, String> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("text")
        .arg("--dangerously-skip-permissions")
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.env_remove("CLAUDECODE");
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd.env("AI_ASSISTANT_TASK", "1");

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        cmd.output(),
    )
    .await
    .map_err(|_| format!("claude -p timed out after {timeout_secs}s"))?
    .map_err(|e| format!("Failed to execute claude: {e}"))?;

    Ok(ClaudeResult {
        stdout: String::from_utf8_lossy(&result.stdout).to_string(),
        stderr: String::from_utf8_lossy(&result.stderr).to_string(),
        exit_code: result.status.code().unwrap_or(-1),
    })
}
