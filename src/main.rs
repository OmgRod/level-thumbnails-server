use std::path::Path;
use axum::response::Response;
use axum::{Router, routing::get, routing::post};
use tower_http::cors;
use tower_http::services::{ServeDir, ServeFile};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing::{info};

mod auth;
mod cache_controller;
mod database;
mod routes;
mod util;

use routes::{login, thumbnail, upload, user};

#[tokio::main]
async fn main() {
    // parse .env file
    dotenv::dotenv().ok();

    // setup logging
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "server.log");
    let (non_blocking_logger, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking_logger)
        .with_ansi(false)
        .init();


    // setup directories
    tokio::fs::create_dir_all("thumbnails").await.unwrap();
    tokio::fs::create_dir_all("uploads").await.unwrap();

    let cors = cors::CorsLayer::new()
        .allow_origin(cors::Any)
        .allow_methods(cors::Any)
        .allow_headers(cors::Any);

    let db = database::get_db().await;

    let app = Router::new()
        .route("/stats", get(get_stats))
        // /thumbnail
        .route("/thumbnail/{id}", get(thumbnail::image_handler_default))
        .route("/thumbnail/{id}/{res}", get(thumbnail::image_handler_with_res))
        .route("/thumbnail/{id}/info", get(thumbnail::thumbnail_info_handler))
        .route("/thumbnail/random", get(thumbnail::random_handler))
        .route("/thumbnail/random/{res}", get(thumbnail::random_res_handler))
        // /auth
        .route("/auth/login", post(login::login))
        .route("/auth/discord", get(login::discord_oauth_handler))
        .route("/auth/session", get(login::get_session))
        .route("/auth/link", get(login::get_link_token))
        .route("/auth/link", post(login::link_account))
        // /user
        .route("/user/me", get(user::get_me))
        .route("/user/{id}", get(user::get_user_by_id))
        // .route("/user/me/uploads", get(routes::user::get_my_uploads))
        // .route("/user/{id}/uploads", get(routes::user::get_user_uploads))
        // /upload
        .route("/upload/{id}", post(upload::upload))
        // /pending
        .route("/pending/{id}/image", get(upload::get_pending_image))
        .route("/pending", get(upload::get_all_pending_uploads))
        .route("/pending/{id}", get(upload::get_pending_info))
        .route("/pending/{id}", post(upload::pending_action))
        .route("/pending/level/{id}", get(upload::get_pending_uploads_for_level))
        .route("/pending/user/{id}", get(upload::get_pending_uploads_for_user))
        // /admin
        // .route("/admin/users", get(routes::admin::get_users))
        // .route("/admin/user/:id", get(routes::admin::get_user_by_id))
        // .route("/admin/user/:id", patch(routes::admin::update_user))
        // .route("/admin/ban/:id", post(routes::admin::ban_user))
        // .route("/admin/thumbnail/:id", delete(routes::admin::delete_thumbnail))
        .with_state(db)
        .layer(cors)
        .fallback_service(ServeDir::new("dist").fallback(ServeFile::new("dist/index.html")));

    let bind_address = dotenv::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();

    info!("Started server!");
    axum::serve(listener, app).await.unwrap();
}

async fn get_dir_stats(path: &Path) -> Result<(u64, usize), std::io::Error> {
    let mut entries = tokio::fs::read_dir(path).await?;
    let mut total_size = 0;
    let mut file_count = 0;

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        file_count += 1;
        total_size += metadata.len();
    }

    Ok((total_size, file_count))
}

async fn get_stats() -> Response {
    let (storage_size, thumbnails_count) = match get_dir_stats(Path::new("thumbnails")).await {
        Ok((size, count)) => (size, count),
        Err(_) => (0, 0),
    };

    let users_per_month = 3292188; // TODO: Fetch this from Cloudflare API

    util::response(
        axum::http::StatusCode::OK,
        serde_json::json!({
            "storage": storage_size,
            "thumbnails": thumbnails_count,
            "users_per_month": users_per_month,
        }),
    )
}
