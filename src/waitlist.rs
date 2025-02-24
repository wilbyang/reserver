use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;

use crate::{BookingError, user::Claims};

#[derive(Debug, Serialize, Deserialize)]
pub struct WaitlistEntry {
    id: i32,
    user_id: i32,
    resource_id: i32,
    preferred_start: DateTime<Utc>,
    preferred_end: DateTime<Utc>,
    status: WaitlistStatus,
    note: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWaitlistRequest {
    resource_id: i32,
    preferred_start: DateTime<Utc>,
    preferred_end: DateTime<Utc>,
    note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum WaitlistStatus {
    Pending,
    Notified,
    Booked,
    Expired,
    Cancelled,
}

// Create a new waitlist entry

pub async fn create_waitlist_entry(
    claims: Claims,
    State(pool): State<PgPool>,
    Json(request): Json<CreateWaitlistRequest>,
) -> Result<impl IntoResponse, BookingError> {
    let entry = sqlx::query_as!(
        WaitlistEntry,
        r#"
        INSERT INTO waitlist_entries 
            (user_id, resource_id, preferred_start, preferred_end, note)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, user_id, resource_id, preferred_start, preferred_end, 
                  status as "status: WaitlistStatus", note, created_at
        "#,
        claims.sub,
        request.resource_id,
        request.preferred_start,
        request.preferred_end,
        request.note,
    )
    .fetch_one(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    Ok((StatusCode::CREATED, Json(entry)))
}

// Get all waitlist entries for a resource
pub async fn get_resource_waitlist(
    State(pool): State<PgPool>,
    Path(resource_id): Path<i32>,
) -> Result<impl IntoResponse, BookingError> {
    let entries = sqlx::query_as!(
        WaitlistEntry,
        r#"
        SELECT id, user_id, resource_id, preferred_start, preferred_end, 
               status as "status: WaitlistStatus", note, created_at
        FROM waitlist_entries
        WHERE resource_id = $1 AND status = 'pending'
        ORDER BY created_at ASC
        "#,
        resource_id
    )
    .fetch_all(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    Ok(Json(entries))
}

// Get user's waitlist entries

pub async fn get_user_waitlist(
    claims: Claims,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, BookingError> {
    let entries = sqlx::query_as!(
        WaitlistEntry,
        r#"
        SELECT id, user_id, resource_id, preferred_start, preferred_end, 
               status as "status: WaitlistStatus", note, created_at
        FROM waitlist_entries
        WHERE user_id = $1 AND status = 'pending'
        ORDER BY created_at ASC
        "#,
        claims.sub
    )
    .fetch_all(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    Ok(Json(entries))
}

// Cancel a waitlist entry

pub async fn cancel_waitlist_entry(
    claims: Claims,
    State(pool): State<PgPool>,
    Path(entry_id): Path<i32>,
) -> Result<impl IntoResponse, BookingError> {
    let result = sqlx::query!(
        r#"
        UPDATE waitlist_entries
        SET status = 'cancelled'
        WHERE id = $1 AND user_id = $2
        RETURNING id
        "#,
        entry_id,
        claims.sub
    )
    .fetch_optional(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    match result {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Ok(StatusCode::NOT_FOUND),
    }
}

// Check for availability and notify waitlist entries
pub async fn check_and_notify_waitlist(
    pool: &PgPool,
    resource_id: i32,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<Vec<WaitlistEntry>, BookingError> {
    let available_entries = sqlx::query_as!(
        WaitlistEntry,
        r#"
        SELECT id, user_id, resource_id, preferred_start, preferred_end, 
               status as "status: WaitlistStatus", note, created_at
        FROM waitlist_entries
        WHERE resource_id = $1 
        AND status = 'pending'
        AND preferred_start >= $2
        AND preferred_end <= $3
        ORDER BY created_at ASC
        "#,
        resource_id,
        start_time,
        end_time
    )
    .fetch_all(pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    // Update status to notified
    for entry in &available_entries {
        sqlx::query!(
            r#"
            UPDATE waitlist_entries
            SET status = 'notified'
            WHERE id = $1
            "#,
            entry.id
        )
        .execute(pool)
        .await
        .map_err(BookingError::DatabaseError)?;

        // Here you would typically send a notification to the user
        // This could be implemented as a separate service
        // notify_user(entry).await?;
    }

    Ok(available_entries)
} 