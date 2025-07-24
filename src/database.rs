use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, Postgres};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Database {
    pub pool: Arc<sqlx::Pool<Postgres>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum Role {
    User,      // regular user
    Verified,  // verified users can upload thumbnails without approval
    Moderator, // moderators can approve or reject uploads
    Admin,     // admins can manage users and uploads
}

#[derive(Debug, FromRow, Serialize)]
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

#[derive(FromRow, Serialize, Deserialize)]
pub struct PendingUpload {
    pub id: i64,
    pub user_id: i64,
    pub level_id: i64,
    pub accepted: bool,
    pub upload_time: NaiveDateTime,
}

#[derive(FromRow, Serialize, Deserialize)]
pub struct UserStats {
    pub id: i64,
    pub account_id: i64,
    pub username: String,
    pub role: Role,
    pub upload_count: i64,
    pub accepted_upload_count: i64,
    pub level_count: i64,
    pub accepted_level_count: i64,
    pub active_thumbnail_count: i64,
}

impl Database {
    pub async fn new() -> Self {
        let connection_string = dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await
            .expect("Failed to connect to the database");

        // Run migrations if needed
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        Database {
            pool: Arc::new(pool),
        }
    }

    pub async fn get_upload_info(&self, id: i64) -> Option<UploadInfo> {
        sqlx::query_as::<_, UploadInfo>(
            "SELECT users.account_id, users.username
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

    pub async fn find_or_create_user(
        &self,
        account_id: i64,
        username: &str,
    ) -> Result<User, sqlx::Error> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE account_id = $1")
            .bind(account_id)
            .bind(username)
            .fetch_optional(&*self.pool)
            .await?;

        if let Some(user) = user {
            Ok(user)
        } else {
            let new_user = sqlx::query_as::<_, User>(
                "INSERT INTO users (account_id, username, role) VALUES ($1, $2, 'user') RETURNING *",
            )
            .bind(account_id)
            .bind(username)
            .fetch_one(&*self.pool)
            .await?;
            Ok(new_user)
        }
    }

    pub async fn get_user_by_id(&self, id: i64) -> Option<User> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&*self.pool)
            .await
            .ok()?
    }

    pub async fn add_upload(
        &self,
        level_id: i64,
        user_id: i64,
        image_path: &str,
        accepted: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
                if accepted {
                    "INSERT INTO uploads (level_id, user_id, image_path, accepted, accepted_time, accepted_by)
                     VALUES ($1, $2, $3, $4, NOW(), $2)"
                } else {
                    "INSERT INTO uploads (level_id, user_id, image_path, accepted)
                     VALUES ($1, $2, $3, $4)"
                }
            )
            .bind(level_id)
            .bind(user_id)
            .bind(image_path)
            .bind(accepted)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_pending_uploads(&self) -> Result<Vec<PendingUpload>, sqlx::Error> {
        sqlx::query_as::<_, PendingUpload>(
            "SELECT id, user_id, level_id, accepted, upload_time FROM uploads
             WHERE accepted = FALSE AND accepted_time IS NULL
             ORDER BY upload_time DESC",
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_pending_uploads_for_level(
        &self,
        level_id: i64,
    ) -> Result<Vec<PendingUpload>, sqlx::Error> {
        sqlx::query_as::<_, PendingUpload>(
            "SELECT id, user_id, level_id, accepted, upload_time FROM uploads
                 WHERE accepted = FALSE AND accepted_time IS NULL AND level_id = $1
                 ORDER BY upload_time DESC",
        )
        .bind(level_id)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_pending_uploads_for_user(
        &self,
        user_id: i64,
    ) -> Result<Vec<PendingUpload>, sqlx::Error> {
        sqlx::query_as::<_, PendingUpload>(
            "SELECT id, user_id, level_id, accepted, upload_time FROM uploads
             WHERE accepted = FALSE AND accepted_time IS NULL AND user_id = $1
             ORDER BY upload_time DESC",
        )
        .bind(user_id)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_pending_upload(&self, id: i64) -> Result<PendingUpload, sqlx::Error> {
        sqlx::query_as::<_, PendingUpload>(
            "SELECT id, user_id, level_id, accepted, upload_time FROM uploads
             WHERE accepted = FALSE AND accepted_time IS NULL AND id = $1",
        )
        .bind(id)
        .fetch_one(&*self.pool)
        .await
    }

    pub async fn accept_upload(
        &self,
        id: i64,
        accepted_by: i64,
        reason: Option<String>,
        accept: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
                "UPDATE uploads SET accepted = $1, accepted_time = NOW(), accepted_by = $2, reason = $3 WHERE id = $4",
            )
            .bind(accept)
            .bind(accepted_by)
            .bind(reason)
            .bind(id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_user_stats(&self, id: i64) -> Option<UserStats> {
        sqlx::query_as::<_, UserStats>(
            "SELECT
                users.id, users.account_id,
                users.username, users.role,
                COUNT(uploads.id) AS upload_count,
                COUNT(DISTINCT uploads.level_id) AS level_count,
                COUNT(uploads.id) FILTER (WHERE uploads.accepted = TRUE) AS accepted_upload_count,
                COUNT(DISTINCT uploads.level_id) FILTER (WHERE uploads.accepted = TRUE) AS accepted_level_count,
                (
                  SELECT COUNT(*)
                  FROM (
                    SELECT u.level_id
                    FROM uploads u
                    WHERE u.accepted = TRUE
                    AND u.user_id = users.id
                    AND u.upload_time = (
                      SELECT MAX(u2.upload_time)
                      FROM uploads u2
                      WHERE u2.level_id = u.level_id
                        AND u2.accepted = TRUE
                    )
                  ) active_levels
                ) AS active_thumbnail_count
              FROM users
              LEFT JOIN uploads ON users.id = uploads.user_id
              WHERE users.id = $1
              GROUP BY users.id, users.account_id, users.username, users.role",
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
