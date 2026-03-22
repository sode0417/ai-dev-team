use std::env;

/// タスク実行タイムアウト（秒）
pub mod timeout {
    // 分析・計画系（読み取り専用、比較的短い）
    pub const HEARING_SECS: u64 = 300;         // 5分
    pub const PLANNER_SECS: u64 = 300;         // 5分
    pub const SCAN_SECS: u64 = 300;            // 5分
    pub const SPRINT_PLANNING_SECS: u64 = 180; // 3分
    pub const RETROSPECTIVE_SECS: u64 = 180;   // 3分（120→180に引き上げ）

    // 実行系（コード変更あり、長め）
    pub const CODER_SECS: u64 = 1200;          // 20分（1タスク=1PRで粒度が小さくなる前提）
    pub const REVIEWER_SECS: u64 = 300;        // 5分
    pub const TEST_SECS: u64 = 300;            // 5分
    pub const QA_SECS: u64 = 600;              // 10分

    // 修正系（レビュー/テスト指摘の修正）
    pub const FIX_SECS: u64 = 600;             // 10分

    // 特殊タスク
    pub const INVESTIGATION_SECS: u64 = 600;   // 10分
    pub const OPERATION_SECS: u64 = 600;       // 10分
}

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub github_token: Option<String>,
    pub auth_enabled: bool,
    pub jwt_secret: String,
    pub jwt_access_expiry_secs: i64,
    pub jwt_refresh_expiry_days: i64,
    pub allowed_origins: Vec<String>,
    pub factrail_url: Option<String>,
    pub factrail_email: Option<String>,
    pub factrail_password: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let auth_enabled = env::var("AUTH_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let allowed_origins = env::var("ALLOWED_ORIGINS")
            .map(|s| {
                s.split(',')
                    .map(|o| o.trim().to_string())
                    .filter(|o| !o.is_empty())
                    .collect()
            })
            .unwrap_or_else(|_| vec!["http://localhost:3100".to_string()]);

        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8100".to_string())
                .parse()
                .expect("PORT must be a valid u16"),
            github_token: env::var("GITHUB_TOKEN").ok(),
            auth_enabled,
            jwt_secret: if auth_enabled {
                env::var("JWT_SECRET").expect("JWT_SECRET must be set when AUTH_ENABLED=true")
            } else {
                env::var("JWT_SECRET").unwrap_or_default()
            },
            jwt_access_expiry_secs: env::var("JWT_ACCESS_EXPIRY")
                .unwrap_or_else(|_| "900".to_string())
                .parse()
                .expect("JWT_ACCESS_EXPIRY must be a valid i64"),
            jwt_refresh_expiry_days: env::var("JWT_REFRESH_EXPIRY_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .expect("JWT_REFRESH_EXPIRY_DAYS must be a valid i64"),
            allowed_origins,
            factrail_url: env::var("FACTRAIL_URL").ok(),
            factrail_email: env::var("FACTRAIL_EMAIL").ok(),
            factrail_password: env::var("FACTRAIL_PASSWORD").ok(),
        }
    }
}
