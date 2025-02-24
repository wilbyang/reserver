use axum::{
    async_trait,
    extract::FromRequestParts,
    
    http::request::Parts,
};
use jsonwebtoken::{decode, DecodingKey, Validation};

use crate::user::{Claims, UserError};

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = UserError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(|| UserError::AuthError("Missing authorization header".to_string()))?;

        let token_data = decode::<Claims>(
            auth_header,
            &DecodingKey::from_secret(std::env::var("JWT_SECRET").unwrap().as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| UserError::AuthError("Invalid token".to_string()))?;

        Ok(token_data.claims)
    }
} 