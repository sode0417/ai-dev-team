use reqwest::Client;
use serde_json::Value;
use std::sync::OnceLock;

/// グローバル Factrail クライアント（pipeline.rs から参照用）
static GLOBAL_CLIENT: OnceLock<FactrailClient> = OnceLock::new();

/// グローバルクライアントを設定
pub fn init_global(client: FactrailClient) {
    let _ = GLOBAL_CLIENT.set(client);
}

/// グローバルクライアントを取得
pub fn global() -> Option<&'static FactrailClient> {
    GLOBAL_CLIENT.get()
}

/// Factrail API クライアント（API キー認証）
#[derive(Clone)]
pub struct FactrailClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl FactrailClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    /// Config から生成（FACTRAIL_URL + FACTRAIL_API_KEY が必要）
    pub fn from_config(config: &crate::config::Config) -> Option<Self> {
        let url = config.factrail_url.as_ref()?;
        let api_key = config.factrail_api_key.as_ref()?;
        Some(Self::new(url.clone(), api_key.clone()))
    }

    /// Fact を送信
    pub async fn send_fact(&self, fact: &Value) -> Result<(), String> {
        let res = self
            .client
            .post(format!("{}/api/facts", self.base_url))
            .header("X-API-Key", &self.api_key)
            .json(fact)
            .send()
            .await
            .map_err(|e| format!("Factrail request failed: {e}"))?;

        if res.status().is_success() {
            Ok(())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Factrail API error {status}: {body}"))
        }
    }
}
