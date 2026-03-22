use reqwest::Client;
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

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

/// Factrail API クライアント
/// ログインしてJWTトークンを取得し、Fact を送信する
#[derive(Clone)]
pub struct FactrailClient {
    client: Client,
    base_url: String,
    email: String,
    password: String,
    tokens: Arc<RwLock<Option<Tokens>>>,
}

#[derive(Clone)]
struct Tokens {
    access_token: String,
    refresh_token: String,
}

impl FactrailClient {
    pub fn new(base_url: String, email: String, password: String) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            email,
            password,
            tokens: Arc::new(RwLock::new(None)),
        }
    }

    /// 設定が有効かどうか
    pub fn from_config(config: &crate::config::Config) -> Option<Self> {
        let url = config.factrail_url.as_ref()?;
        let email = config.factrail_email.as_ref()?;
        let password = config.factrail_password.as_ref()?;
        Some(Self::new(url.clone(), email.clone(), password.clone()))
    }

    /// ログインしてトークンを取得
    async fn login(&self) -> Result<Tokens, String> {
        let res = self
            .client
            .post(format!("{}/auth/login", self.base_url))
            .json(&serde_json::json!({
                "email": self.email,
                "password": self.password,
            }))
            .send()
            .await
            .map_err(|e| format!("Factrail login request failed: {e}"))?;

        if !res.status().is_success() {
            return Err(format!("Factrail login failed: {}", res.status()));
        }

        let body: Value = res
            .json()
            .await
            .map_err(|e| format!("Factrail login response parse failed: {e}"))?;

        let access_token = body["accessToken"]
            .as_str()
            .ok_or("Missing accessToken in login response")?
            .to_string();
        let refresh_token = body["refreshToken"]
            .as_str()
            .ok_or("Missing refreshToken in login response")?
            .to_string();

        Ok(Tokens {
            access_token,
            refresh_token,
        })
    }

    /// トークンをリフレッシュ
    async fn refresh(&self, refresh_token: &str) -> Result<Tokens, String> {
        let res = self
            .client
            .post(format!("{}/auth/refresh", self.base_url))
            .json(&serde_json::json!({
                "refreshToken": refresh_token,
            }))
            .send()
            .await
            .map_err(|e| format!("Factrail refresh request failed: {e}"))?;

        if !res.status().is_success() {
            return Err(format!("Factrail refresh failed: {}", res.status()));
        }

        let body: Value = res
            .json()
            .await
            .map_err(|e| format!("Factrail refresh response parse failed: {e}"))?;

        let access_token = body["accessToken"]
            .as_str()
            .ok_or("Missing accessToken in refresh response")?
            .to_string();
        let new_refresh = body["refreshToken"]
            .as_str()
            .unwrap_or(refresh_token)
            .to_string();

        Ok(Tokens {
            access_token,
            refresh_token: new_refresh,
        })
    }

    /// 有効なアクセストークンを取得（必要に応じてログイン/リフレッシュ）
    async fn get_access_token(&self) -> Result<String, String> {
        // 既存トークンを試す
        {
            let tokens = self.tokens.read().await;
            if let Some(ref t) = *tokens {
                return Ok(t.access_token.clone());
            }
        }

        // ログイン
        let tokens = self.login().await?;
        let access_token = tokens.access_token.clone();
        *self.tokens.write().await = Some(tokens);
        Ok(access_token)
    }

    /// Fact を送信（認証エラー時はリフレッシュ/再ログインしてリトライ）
    pub async fn send_fact(&self, fact: &Value) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let res = self
            .client
            .post(format!("{}/api/facts", self.base_url))
            .bearer_auth(&token)
            .json(fact)
            .send()
            .await
            .map_err(|e| format!("Factrail send_fact request failed: {e}"))?;

        if res.status().is_success() {
            return Ok(());
        }

        // 401 → リフレッシュしてリトライ
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            let new_tokens = {
                let tokens = self.tokens.read().await;
                if let Some(ref t) = *tokens {
                    match self.refresh(&t.refresh_token).await {
                        Ok(new) => new,
                        Err(_) => self.login().await?,
                    }
                } else {
                    self.login().await?
                }
            };

            let retry_token = new_tokens.access_token.clone();
            *self.tokens.write().await = Some(new_tokens);

            let retry_res = self
                .client
                .post(format!("{}/api/facts", self.base_url))
                .bearer_auth(&retry_token)
                .json(fact)
                .send()
                .await
                .map_err(|e| format!("Factrail send_fact retry failed: {e}"))?;

            if retry_res.status().is_success() {
                return Ok(());
            }

            return Err(format!(
                "Factrail send_fact failed after retry: {}",
                retry_res.status()
            ));
        }

        Err(format!("Factrail send_fact failed: {}", res.status()))
    }
}
