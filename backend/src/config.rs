use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub github_token: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8100".to_string())
                .parse()
                .expect("PORT must be a valid u16"),
            github_token: env::var("GITHUB_TOKEN").ok(),
        }
    }
}
