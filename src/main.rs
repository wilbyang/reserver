use std::borrow::Cow;

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

#[derive(Debug)]
struct Booking {
    id: i32,
    resource_id: i32,
    timespan: Option<(DateTime<Utc>, DateTime<Utc>)>,
    note: Option<String>,
    user_id: String,
}

async fn create_booking(
    pool: &PgPool,
    resource_id: i32,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    note: Option<String>,
    user_id: String,
) -> Result<Booking, BookingError> {
    let result = sqlx::query_as!(
        Booking,
        r#"
        INSERT INTO bookings (resource_id, timespan, note, user_id)
        VALUES ($1, tstzrange($2, $3), $4, $5)
        RETURNING id, resource_id, (lower(timespan), upper(timespan)) as "timespan: (DateTime<Utc>, DateTime<Utc>)", note, user_id
        "#,
        resource_id,
        start_time,
        end_time,
        note,
        user_id
    )
    .fetch_one(pool)
    .await;

    match result {
        Ok(booking) => Ok(booking),
        Err(sqlx::Error::Database(db_err)) if db_err.code() == Some(Cow::Borrowed("23P01")) => {
            Err(BookingError::BookingConflict("The booking times conflict with an existing booking.".to_string()))
        }
        Err(e) => Err(BookingError::DatabaseError(e)),
    }
}

#[tokio::main]
async fn main() -> Result<(), BookingError> {
    let database_url = "postgres://boya:@localhost:28813/booking_system";
    let pool = PgPool::connect(database_url).await?;

    

    let resource_id = 1;
    let start_time = Utc::now();
    let end_time = start_time + chrono::Duration::hours(2);
    let note = Some("Meeting with team".to_string());
    let user_id = "user123".to_string();

    match create_booking(&pool, resource_id, start_time, end_time, note, user_id).await {
        Ok(booking) => println!("Booking created: {:?}", booking),
        Err(BookingError::BookingConflict(msg)) => println!("Failed to create booking: {}", msg),
        Err(e) => println!("An error occurred: {:?}", e),
    }

    Ok(())
}
