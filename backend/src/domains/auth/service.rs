use argon2::{
    password_hash::{rand_core::{OsRng, RngCore}, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::config::Config;
use crate::error::AppError;

use super::model::{AuthResponse, MeResponse, User};

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Password hashing failed: {e}")))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(format!("Invalid password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_access_token(user_id: Uuid, username: &str, config: &Config) -> Result<String, AppError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::seconds(config.jwt_access_expiry_secs)).timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Token generation failed: {e}")))
}

fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub async fn login(
    pool: &PgPool,
    config: &Config,
    username: &str,
    password: &str,
) -> Result<AuthResponse, AppError> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE username = $1")
        .bind(username)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid username or password".to_string()))?;

    if !verify_password(password, &user.password_hash)? {
        return Err(AppError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }

    let access_token = generate_access_token(user.id, &user.username, config)?;
    let refresh_token = generate_refresh_token();
    let token_hash = hash_token(&refresh_token);
    let expires_at = Utc::now() + Duration::days(config.jwt_refresh_expiry_days);

    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user.id)
        .bind(&token_hash)
        .bind(expires_at)
        .execute(pool)
        .await?;

    Ok(AuthResponse {
        access_token,
        refresh_token,
        expires_in: config.jwt_access_expiry_secs,
    })
}

pub async fn refresh(
    pool: &PgPool,
    config: &Config,
    refresh_token: &str,
) -> Result<AuthResponse, AppError> {
    let token_hash = hash_token(refresh_token);

    let mut tx = pool.begin().await?;

    // DELETE ... RETURNING でアトミックに取得＋削除（Race Condition 防止）
    let stored: super::model::RefreshToken = sqlx::query_as(
        "DELETE FROM refresh_tokens WHERE token_hash = $1 AND expires_at > NOW() RETURNING *",
    )
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid or expired refresh token".to_string()))?;

    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(stored.user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    let access_token = generate_access_token(user.id, &user.username, config)?;
    let new_refresh_token = generate_refresh_token();
    let new_token_hash = hash_token(&new_refresh_token);
    let expires_at = Utc::now() + Duration::days(config.jwt_refresh_expiry_days);

    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user.id)
        .bind(&new_token_hash)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(AuthResponse {
        access_token,
        refresh_token: new_refresh_token,
        expires_in: config.jwt_access_expiry_secs,
    })
}

pub async fn upsert_google_user(
    pool: &PgPool,
    email: &str,
    name: Option<&str>,
) -> Result<User, AppError> {
    // username で検索（既存ユーザー）
    let existing: Option<User> = sqlx::query_as("SELECT * FROM users WHERE username = $1")
        .bind(email)
        .fetch_optional(pool)
        .await?;

    if let Some(user) = existing {
        return Ok(user);
    }

    // 新規作成（Google OAuth ユーザーはパスワードなし）
    let user: User = sqlx::query_as(
        "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING *",
    )
    .bind(email)
    .bind("oauth:google")
    .fetch_one(pool)
    .await?;

    tracing::info!(
        "Created new user via Google OAuth: {} ({})",
        email,
        name.unwrap_or("?")
    );

    Ok(user)
}

pub async fn get_me(pool: &PgPool, user_id: Uuid) -> Result<MeResponse, AppError> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(MeResponse {
        id: user.id,
        username: user.username,
    })
}
