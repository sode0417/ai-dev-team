use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::{decode, DecodingKey, Validation};
use uuid::Uuid;

use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    pub sub: String, // user_id (UUID)
    pub username: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 認証無効時はダミーユーザーを返す（段階的移行用）
        if !state.config.auth_enabled {
            return Ok(AuthUser {
                user_id: Uuid::nil(),
                username: "anonymous".to_string(),
            });
        }

        // 1. API キー認証 (X-API-Key ヘッダー)
        if let Some(api_key) = parts
            .headers
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
        {
            if state.config.api_keys.contains(&api_key.to_string()) {
                return Ok(AuthUser {
                    user_id: Uuid::nil(),
                    username: "service".to_string(),
                });
            }
            return Err(AppError::Unauthorized("Invalid API key".to_string()));
        }

        // 2. Bearer トークン認証 (Authorization ヘッダー)
        if let Some(auth_header) = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
        {
            if let Some(token) = auth_header.strip_prefix("Bearer ") {
                return validate_jwt(token, &state.config.jwt_secret);
            }
        }

        // 3. Cookie 認証 (f2a_token)
        if let Some(cookie_header) = parts.headers.get("Cookie").and_then(|v| v.to_str().ok()) {
            for cookie in cookie_header.split(';') {
                let cookie = cookie.trim();
                if let Some(token) = cookie.strip_prefix("f2a_token=") {
                    return validate_jwt(token, &state.config.jwt_secret);
                }
            }
        }

        Err(AppError::Unauthorized(
            "No valid authentication found".to_string(),
        ))
    }
}

fn validate_jwt(token: &str, secret: &str) -> Result<AuthUser, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(format!("Invalid token: {e}")))?;

    let user_id = Uuid::parse_str(&token_data.claims.sub)
        .map_err(|_| AppError::Unauthorized("Invalid token subject".to_string()))?;

    Ok(AuthUser {
        user_id,
        username: token_data.claims.username,
    })
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(format!("Invalid token: {e}")))?;
    Ok(token_data.claims)
}
