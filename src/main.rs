use std::borrow::Cow;
use std::net::SocketAddr;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use sqlx::PgPool;
use chrono::{DateTime, Utc};


#[derive(Debug, Error)]
enum BookingError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Booking conflict: {0}")]
    BookingConflict(String),
}

#[derive(Debug, Serialize)]
struct Booking {
    id: i32,
    resource_id: i32,
    timespan: Option<(DateTime<Utc>, DateTime<Utc>)>,
    note: Option<String>,
    user_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateBookingRequest {
    resource_id: i32,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    note: Option<String>,
    user_id: String,
}

// Implement IntoResponse for BookingError
impl IntoResponse for BookingError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            BookingError::BookingConflict(msg) => (StatusCode::CONFLICT, msg),
            BookingError::DatabaseError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

async fn create_booking(
    State(pool): State<PgPool>,
    Json(request): Json<CreateBookingRequest>,
) -> Result<impl IntoResponse, BookingError> {
    let booking = sqlx::query_as!(
        Booking,
        r#"
        INSERT INTO bookings (resource_id, timespan, note, user_id)
        VALUES ($1, tstzrange($2, $3), $4, $5)
        RETURNING id, resource_id, (lower(timespan), upper(timespan)) as "timespan: (DateTime<Utc>, DateTime<Utc>)", note, user_id
        "#,
        request.resource_id,
        request.start_time,
        request.end_time,
        request.note,
        request.user_id
    )
    .fetch_one(&pool)
    .await;

    match booking {
        Ok(booking) => Ok((StatusCode::CREATED, Json(booking))),
        Err(sqlx::Error::Database(db_err)) if db_err.code() == Some(Cow::Borrowed("23P01")) => {
            Err(BookingError::BookingConflict("The booking times conflict with an existing booking.".to_string()))
        }
        Err(e) => Err(BookingError::DatabaseError(e)),
    }
}

async fn get_bookings(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, BookingError> {
    let bookings = sqlx::query_as!(
        Booking,
        r#"
        SELECT id, resource_id, (lower(timespan), upper(timespan)) as "timespan: (DateTime<Utc>, DateTime<Utc>)", note, user_id
        FROM bookings
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    Ok(Json(bookings))
}

#[tokio::main]
async fn main() -> Result<(), BookingError> {
    let database_url = "postgres://boya:@localhost:28813/booking_system";
    let pool = PgPool::connect(database_url).await?;

    let app = Router::new()
        .route("/bookings", post(create_booking))
        .route("/bookings", get(get_bookings))
        .with_state(pool);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
