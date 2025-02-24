use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde_json::json;

use crate::user::{Claims, UserError};

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));

        match auth_header {
            Some(token) => {
                let claims = decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(std::env::var("JWT_SECRET").unwrap().as_bytes()),
                    &Validation::default(),
                )
                .map_err(|_| {
                    (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({ "error": "Invalid token" })),
                    )
                        .into_response()
                })?;

                Ok(claims.claims)
            }
            None => Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Missing authorization header" })),
            )
                .into_response()),
        }
    }
} 