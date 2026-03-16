use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser;

// シングルユーザー前提: 認証不要
// Phase 2+ で JWT 認証を追加予定
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        _parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(AuthUser)
    }
}
