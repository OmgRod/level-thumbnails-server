use axum::{routing::get, routing::post, Router};

mod auth;
mod cache_controller;
mod database;
mod routes;
mod util;

use routes::{login, thumbnail, upload};

#[tokio::main]
async fn main() {
    // parse .env file
    dotenv::dotenv().ok();

    // setup directories
    tokio::fs::create_dir_all("thumbnails").await.unwrap();
    tokio::fs::create_dir_all("uploads").await.unwrap();

    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    let db = database::get_db().await;

    let app = Router::new()
        .route("/", get(root))
        // .route("/stats", get(get_stats))
        // /thumbnail
        .route("/thumbnail/{id}", get(thumbnail::image_handler_default))
        .route("/thumbnail/{id}/{res}", get(thumbnail::image_handler_with_res))
        .route("/thumbnail/{id}/info", get(thumbnail::thumbnail_info_handler))
        .route("/thumbnail/random", get(thumbnail::random_handler))
        .route("/thumbnail/random/{res}", get(thumbnail::random_res_handler))
        // /auth
        .route("/auth/login", post(login::login))
        // /user
        .route("/user/me", get(routes::user::get_me))
        .route("/user/{id}", get(routes::user::get_user_by_id))
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
        .with_state(db)
        .layer(cors);

    let bind_address = dotenv::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    r"
  _                    _   _______ _                     _                 _ _
 | |                  | | |__   __| |                   | |               (_) |
 | |     _____   _____| |    | |  | |__  _   _ _ __ ___ | |__  _ __   __ _ _| |___
 | |    / _ \ \ / / _ \ |    | |  | '_ \| | | | '_ ` _ \| '_ \| '_ \ / _` | | / __|
 | |___|  __/\ V /  __/ |    | |  | | | | |_| | | | | | | |_) | | | | (_| | | \__ \
 |______\___| \_/ \___|_|    |_|  |_| |_|\__,_|_| |_| |_|_.__/|_| |_|\__,_|_|_|___/
  / ____|                          (_)                 | |
 | (___   ___ _ ____   _____ _ __   _ ___   _   _ _ __ | |
  \___ \ / _ \ '__\ \ / / _ \ '__| | / __| | | | | '_ \| |
  ____) |  __/ |   \ V /  __/ |    | \__ \ | |_| | |_) |_|
 |_____/ \___|_|    \_/ \___|_|    |_|___/  \__,_| .__/(_)
                                                 | |
                                                 |_|                               "
}