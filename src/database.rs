use sqlx::FromRow;
use sqlx::postgres::PgPoolOptions;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// define a struct to represent the database connection
#[derive(Debug, Clone)]
pub struct Database {
    pub pool: Arc<sqlx::Pool<sqlx::Postgres>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Role {
    #[serde(rename = "user")]
    User, // regular user
    #[serde(rename = "verified")]
    Verified, // verified users can upload thumbnails without approval
    #[serde(rename = "moderator")]
    Moderator, // moderators can approve or reject uploads
    #[serde(rename = "admin")]
    Admin, // admins can manage users and uploads
}

// define a struct to represent the upload
#[derive(Debug, FromRow)]
pub struct Upload {
    pub id: i64,
    pub user_id: i64,
    pub level_id: i64,
    pub upload_time: DateTime<Utc>,
    pub accepted_time: Option<DateTime<Utc>>,
    pub accepted_by: Option<i64>,
    pub image_path: String,
    pub accepted: bool,
    pub reason: Option<String>,
}

// define a struct to represent the user
#[derive(Debug, FromRow)]
pub struct User {
    pub id: i64,
    pub account_id: i64,
    pub username: String,
    pub role: Role,
}

#[derive(FromRow)]
pub struct UploadInfo {
    pub account_id: i64,
    pub username: String,
    pub image_path: String,
}

#[derive(FromRow, Serialize, Deserialize)]
pub struct UploadExtended {
    pub level_id: i64,
    pub account_id: i64,
    pub username: String,
    pub upload_time: NaiveDateTime,
    pub first_upload_time: NaiveDateTime,
    pub accepted_time: Option<NaiveDateTime>,
    pub accepted_by: Option<i64>,
    pub accepted_by_username: Option<String>,
}

impl Database {
    pub async fn new() -> Self {
        let connection_string = dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await
            .expect("Failed to connect to the database");
        Database {
            pool: Arc::new(pool),
        }
    }

    pub async fn get_upload_info(&self, id: i64) -> Option<UploadInfo> {
        sqlx::query_as::<_, UploadInfo>(
            "SELECT users.account_id, users.username, uploads.image_path
                 FROM uploads
                 JOIN users ON uploads.user_id = users.id
                 WHERE uploads.level_id = $1 AND accepted = TRUE
                 ORDER BY upload_time DESC LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .ok()?
    }

    pub async fn get_upload_extended(&self, id: i64) -> Option<UploadExtended> {
        sqlx::query_as::<_, UploadExtended>(
            "SELECT 
                    uploads.level_id,
                    users.account_id,
                    users.username,
                    uploads.upload_time,
                    (
                        SELECT MIN(upload_time) FROM uploads u2
                        WHERE u2.level_id = uploads.level_id AND u2.accepted = TRUE
                    ) AS first_upload_time,
                    uploads.accepted_time,
                    accepted_by.account_id AS accepted_by,
                    accepted_by.username AS accepted_by_username
                 FROM uploads
                 JOIN users ON uploads.user_id = users.id
                 LEFT JOIN users AS accepted_by ON uploads.accepted_by = accepted_by.id
                 WHERE uploads.level_id = $1 AND accepted = TRUE
                 ORDER BY upload_time DESC LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .ok()?
    }
}

pub async fn get_db() -> Database {
    Database::new().await
}
