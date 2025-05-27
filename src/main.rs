use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Router, http::StatusCode, routing::get, routing::post};
use image::ImageReader;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;
use webp::Encoder;

mod auth;
mod database;

#[tokio::main]
async fn main() {
    // parse .env file
    dotenv::dotenv().ok();

    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    let db = database::get_db().await;

    let app = Router::new()
        .route("/", get(root))
        .route("/thumbnail/{id}", get(image_handler_default))
        // .route("/thumbnail/{id}", get(image_handler_legacy))
        .route("/thumbnail/{id}/{res}", get(image_handler_res))
        .route("/thumbnail/{id}/info", get(thumbnail_info_handler))
        .route("/upload/{id}", post(upload_handler))
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

// macro for fast response error handling
macro_rules! response_error {
    ($status:expr, $message:expr) => {
        Response::builder()
            .status($status)
            .header(header::CONTENT_TYPE, "text/plain")
            .header(header::CACHE_CONTROL, "no-store")
            .body($message.into())
            .unwrap()
    };
}

async fn handle_image(id: u64, res: Res, db: database::Database) -> Response {
    // check if the image exists in the database
    let upload = match db.get_upload_info(id as i64).await {
        Some(upload) => upload,
        None => return response_error!(StatusCode::NOT_FOUND, "Image not found"),
    };

    let image_path = PathBuf::from(upload.image_path);
    if !image_path.exists() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(format!("Image with ID {} not found", id).into())
            .unwrap();
    }

    let width: u32;
    let height: u32;

    match res {
        Res::High => {
            let image_data = tokio::fs::read(image_path).await.unwrap();
            return Response::builder()
                .header(header::CONTENT_TYPE, "image/webp")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("inline; filename=\"{}.webp\"", id),
                )
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .header(header::CONTENT_LENGTH, image_data.len())
                .header("X-Level-ID", id.to_string())
                .header("X-Thumbnail-Author", upload.username)
                .header("X-Thumbnail-User-ID", upload.account_id.to_string())
                .body(image_data.into())
                .unwrap();
        }
        Res::Medium => {
            width = 1280;
            height = 720;
        }
        Res::Small => {
            width = 640;
            height = 360;
        }
    }

    // read the image and rescale it
    let buffer = match tokio::task::spawn_blocking(move || {
        let image = match ImageReader::open(&image_path)
            .map_err(|_| "Failed to open image")?
            .decode()
            .map_err(|_| "Failed to decode image")
        {
            Ok(image) => image,
            Err(e) => return Err(e.to_string()),
        };

        let img = image
            .resize_exact(width, height, image::imageops::FilterType::Lanczos3)
            .to_rgb8();

        Ok(Encoder::from_rgb(&img, width, height)
            .encode_lossless()
            .to_vec())
    })
    .await
    {
        Ok(Ok(buffer)) => buffer,
        Ok(Err(e)) => return response_error!(StatusCode::IM_A_TEAPOT, e),
        Err(e) => return response_error!(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    Response::builder()
        .header(header::CONTENT_TYPE, "image/webp")
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}.webp\"", id),
        )
        .header(header::CONTENT_LENGTH, buffer.len())
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header("X-Level-ID", id.to_string())
        .header("X-Thumbnail-Author", upload.username)
        .header("X-Thumbnail-User-ID", upload.account_id.to_string())
        .body(buffer.into())
        .unwrap()
}

async fn image_handler_res(
    axum::extract::Path((id, res)): axum::extract::Path<(u64, Res)>,
    axum::extract::State(db): axum::extract::State<database::Database>,
) -> Response {
    handle_image(id, res, db).await
}

async fn image_handler_default(
    axum::extract::Path(id): axum::extract::Path<u64>,
    axum::extract::State(db): axum::extract::State<database::Database>,
) -> Response {
    handle_image(id, Res::High, db).await
}

async fn thumbnail_info_handler(
    axum::extract::Path(id): axum::extract::Path<u64>,
    axum::extract::State(db): axum::extract::State<database::Database>,
) -> Response {
    // check if the image exists in the database
    let upload = match db.get_upload_extended(id as i64).await {
        Some(upload) => upload,
        None => return response_error!(StatusCode::NOT_FOUND, "Image not found"),
    };

    Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "no-store")
        .body(serde_json::to_string(&upload).unwrap().into())
        .unwrap()
}

async fn upload_handler(
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<u64>,
    data: axum::body::Bytes,
) -> impl IntoResponse {
    // this endpoint is protected by simple token authentication
    let token = std::env::var("AUTH_TOKEN").unwrap(); // panic if not set
    let auth_header = headers.get("Authorization");
    if auth_header.is_none() || auth_header.unwrap() != token.as_bytes() {
        return (StatusCode::UNAUTHORIZED, "Unauthorized".to_string());
    }

    let image_path = PathBuf::from(format!("./thumbnails/{}.webp", id));
    if image_path.exists() {
        return (
            StatusCode::CONFLICT,
            format!("Image with ID {} already exists", id),
        );
    }

    // parse the image data from the request body
    let image = match ImageReader::new(Cursor::new(data.clone())).with_guessed_format() {
        Ok(reader) => match reader.decode() {
            Ok(image) => image,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Failed to decode image: {}", e),
                );
            }
        },
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode image: {}", e),
            );
        }
    };

    // check if the image is 1920x1080
    if image.width() != 1920 || image.height() != 1080 {
        return (
            StatusCode::BAD_REQUEST,
            "Image must be 1920x1080".to_string(),
        );
    }

    // re-encode the image to webp
    let encoder = Encoder::from_rgb(image.as_rgb8().unwrap(), 1920, 1080);
    let data = encoder.encode_lossless().to_owned();

    // save the image data to a file
    match tokio::fs::write(&image_path, &data).await {
        Ok(_) => (StatusCode::CREATED, format!("Image with ID {} created", id)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save image: {}", e),
        ),
    }
}

#[derive(Deserialize, Serialize, Debug)]
enum Res {
    #[serde(rename = "high")]
    High, // 1920x1080
    #[serde(rename = "medium")]
    Medium, // 1280x720
    #[serde(rename = "small")]
    Small, // 640x360
}
