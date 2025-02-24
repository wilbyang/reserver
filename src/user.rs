use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Regular,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "user_status", rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub preferences: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum UserError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Authorization error: {0}")]
    AuthzError(String),
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    email: String,
    password: String,
    role: UserRole,
}

#[derive(Debug, Serialize)]
struct Claims {
    sub: i32, // user_id
    email: String,
    role: UserRole,
    exp: i64,
}

impl IntoResponse for UserError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            UserError::DatabaseError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            UserError::AuthError(msg) => (StatusCode::UNAUTHORIZED, msg),
            UserError::AuthzError(msg) => (StatusCode::FORBIDDEN, msg),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

pub async fn login(
    State(pool): State<PgPool>,
    Json(request): Json<LoginRequest>,
) -> Result<impl IntoResponse, UserError> {
    let user = sqlx::query_as!(
        User,
        r#"
        SELECT id, email, role as "role: UserRole", status as "status: UserStatus", 
               preferences, created_at
        FROM users
        WHERE email = $1 AND password_hash = crypt($2, password_hash)
        "#,
        request.email,
        request.password
    )
    .fetch_optional(&pool)
    .await?;

    match user {
        Some(user) if user.status == UserStatus::Active => {
            let token = create_jwt(&user)?;
            Ok((StatusCode::OK, Json(json!({ "token": token }))))
        }
        Some(_) => Err(UserError::AuthError("Account is not active".to_string())),
        None => Err(UserError::AuthError("Invalid credentials".to_string())),
    }
}

pub async fn register(
    State(pool): State<PgPool>,
    Json(request): Json<RegisterRequest>,
) -> Result<impl IntoResponse, UserError> {
    let password_hash = sqlx::query_scalar!(
        "SELECT crypt($1, gen_salt('bf'))",
        request.password
    )
    .fetch_one(&pool)
    .await?;

    let user = sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (email, password_hash, role, status, preferences, created_at)
        VALUES ($1, $2, $3, 'active', '{}', NOW())
        RETURNING id, email, role as "role: UserRole", status as "status: UserStatus",
                  preferences, created_at
        "#,
        request.email,
        password_hash,
        request.role as UserRole
    )
    .fetch_one(&pool)
    .await?;

    Ok((StatusCode::CREATED, Json(user)))
}

fn create_jwt(user: &User) -> Result<String, UserError> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: user.id,
        email: user.email.clone(),
        role: user.role.clone(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(std::env::var("JWT_SECRET").unwrap().as_bytes()),
    )
    .map_err(|e| UserError::AuthError(e.to_string()))
}

// Middleware for checking JWT and permissions
pub async fn require_auth(
    claims: Claims,
    required_role: UserRole,
) -> Result<Claims, UserError> {
    match claims.role {
        UserRole::Admin => Ok(claims),
        UserRole::Regular if matches!(required_role, UserRole::Regular) => Ok(claims),
        _ => Err(UserError::AuthzError("Insufficient permissions".to_string())),
    }
}