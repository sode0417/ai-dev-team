use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Clone)]
pub struct GitHubClient {
    client: reqwest::Client,
    token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub number: i64,
    pub title: String,
    pub state: String,
    pub body: Option<String>,
    pub labels: Vec<GitHubLabel>,
    pub user: Option<GitHubUser>,
    pub html_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub comments: i64,
    pub pull_request: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubPullRequest {
    pub number: i64,
    pub title: String,
    pub state: String,
    pub draft: Option<bool>,
    pub user: Option<GitHubUser>,
    pub html_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub head: GitHubRef,
    pub base: GitHubRef,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
}

impl GitHubClient {
    pub fn new(token: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("ai-dev-team")
            .build()
            .expect("Failed to create HTTP client");
        Self { client, token }
    }

    fn request(&self, url: &str) -> reqwest::RequestBuilder {
        let mut req = self
            .client
            .get(url)
            .header("Accept", "application/vnd.github.v3+json");
        if let Some(ref token) = self.token {
            req = req.bearer_auth(token);
        }
        req
    }

    pub async fn fetch_issues(
        &self,
        owner: &str,
        repo: &str,
        state: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GitHubIssue>, AppError> {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/issues?state={state}&page={page}&per_page={per_page}&sort=updated&direction=desc"
        );
        let issues: Vec<GitHubIssue> = self.request(&url).send().await?.json().await?;
        // GitHub Issues API には PR も含まれるのでフィルタ
        let issues = issues
            .into_iter()
            .filter(|i| i.pull_request.is_none())
            .collect();
        Ok(issues)
    }

    pub async fn fetch_pulls(
        &self,
        owner: &str,
        repo: &str,
        state: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GitHubPullRequest>, AppError> {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/pulls?state={state}&page={page}&per_page={per_page}&sort=updated&direction=desc"
        );
        let pulls: Vec<GitHubPullRequest> = self.request(&url).send().await?.json().await?;
        Ok(pulls)
    }
}
