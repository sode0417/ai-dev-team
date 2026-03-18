use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub github_token: Option<String>,
    pub auth_enabled: bool,
    pub jwt_secret: String,
    pub jwt_access_expiry_secs: i64,
    pub jwt_refresh_expiry_days: i64,
}

impl Config {
    pub fn from_env() -> Self {
        let auth_enabled = env::var("AUTH_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

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
        }
    }
}
