use std::borrow::Cow;
use std::net::SocketAddr;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
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

// Resource Types and Structs
#[derive(Debug, Serialize, Deserialize)]
struct Resource {
    id: i32,
    name: String,
    category: ResourceCategory,
    capacity: i32,
    location: String,
    features: Vec<String>,
    metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "resource_category", rename_all = "lowercase")]
enum ResourceCategory {
    Room,
    Device,
    Vehicle,
    Equipment,
}

#[derive(Debug, Deserialize)]
struct CreateResourceRequest {
    name: String,
    category: ResourceCategory,
    capacity: i32,
    location: String,
    features: Vec<String>,
    metadata: serde_json::Value,
}

// Resource Management Endpoints
async fn create_resource(
    State(pool): State<PgPool>,
    Json(request): Json<CreateResourceRequest>,
) -> Result<impl IntoResponse, BookingError> {
    let resource = sqlx::query_as!(
        Resource,
        r#"
        INSERT INTO resources (name, category, capacity, location, features, metadata)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, name, category as "category: ResourceCategory", capacity, location, features, metadata
        "#,
        request.name,
        request.category as ResourceCategory,
        request.capacity,
        request.location,
        &request.features,
        request.metadata
    )
    .fetch_one(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    Ok((StatusCode::CREATED, Json(resource)))
}

async fn get_resources(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, BookingError> {
    let resources = sqlx::query_as!(
        Resource,
        r#"
        SELECT id, name, category as "category: ResourceCategory", capacity, location, features, metadata
        FROM resources
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    Ok(Json(resources))
}

async fn get_resource(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, BookingError> {
    let resource = sqlx::query_as!(
        Resource,
        r#"
        SELECT id, name, category as "category: ResourceCategory", capacity, location, features, metadata
        FROM resources
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    match resource {
        Some(resource) => Ok((StatusCode::OK, Json(resource))),
        None => Err(BookingError::DatabaseError(sqlx::Error::RowNotFound))
    }
}

async fn update_resource(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
    Json(request): Json<CreateResourceRequest>,
) -> Result<impl IntoResponse, BookingError> {
    let resource = sqlx::query_as!(
        Resource,
        r#"
        UPDATE resources
        SET name = $1, category = $2, capacity = $3, location = $4, features = $5, metadata = $6
        WHERE id = $7
        RETURNING id, name, category as "category: ResourceCategory", capacity, location, features, metadata
        "#,
        request.name,
        request.category as ResourceCategory,
        request.capacity,
        request.location,
        &request.features,
        request.metadata,
        id
    )
    .fetch_optional(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    match resource {
        Some(resource) => Ok((StatusCode::OK, Json(resource))),
        None => Err(BookingError::DatabaseError(sqlx::Error::RowNotFound))
    }
}

async fn delete_resource(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, BookingError> {
    let result = sqlx::query!(
        "DELETE FROM resources WHERE id = $1 RETURNING id",
        id
    )
    .fetch_optional(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    match result {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Err(BookingError::DatabaseError(sqlx::Error::RowNotFound))
    }
}

// Resource Availability
async fn get_resource_availability(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
    Query(params): Query<AvailabilityParams>,
) -> Result<impl IntoResponse, BookingError> {
    let bookings = sqlx::query_as!(
        Booking,
        r#"
        SELECT id, resource_id, (lower(timespan), upper(timespan)) as "timespan: (DateTime<Utc>, DateTime<Utc>)", note, user_id
        FROM bookings
        WHERE resource_id = $1
        AND timespan && tstzrange($2, $3)
        ORDER BY lower(timespan)
        "#,
        id,
        params.start_time,
        params.end_time
    )
    .fetch_all(&pool)
    .await
    .map_err(BookingError::DatabaseError)?;

    Ok(Json(bookings))
}

#[derive(Debug, Deserialize)]
struct AvailabilityParams {
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<(), BookingError> {
    let database_url = "postgres://boya:@localhost:28813/booking_system";
    let pool = PgPool::connect(database_url).await?;

    let app = Router::new()
        .route("/resources", post(create_resource))
        .route("/resources", get(get_resources))
        .route("/resources/:id", get(get_resource))
        .route("/resources/:id", put(update_resource))
        .route("/resources/:id", delete(delete_resource))
        .route("/resources/:id/availability", get(get_resource_availability))
        .route("/bookings", post(create_booking))
        .route("/bookings", get(get_bookings))
        .with_state(pool);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
