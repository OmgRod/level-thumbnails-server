use crate::database;
use axum::http::{StatusCode, header};
use axum::response::Response;
use image::ImageReader;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use webp::Encoder;

#[derive(Deserialize, Serialize, Debug)]
pub enum Res {
    #[serde(rename = "high")]
    High, // 1920x1080
    #[serde(rename = "medium")]
    Medium, // 1280x720
    #[serde(rename = "small")]
    Small, // 640x360
}

// macro for fast response error handling
#[macro_export]
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
    let image_path = PathBuf::from(format!("thumbnails/{}.webp", id));
    if !image_path.exists() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(format!("Image with ID {} not found", id).into())
            .unwrap();
    }

    // check if the image exists in the database
    let upload = match db.get_upload_info(id as i64).await {
        Some(upload) => upload,
        None => return response_error!(StatusCode::NOT_FOUND, "Image not found"),
    };

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

pub async fn image_handler_with_res(
    axum::extract::Path((id, res)): axum::extract::Path<(u64, Res)>,
    axum::extract::State(db): axum::extract::State<database::Database>,
) -> Response {
    handle_image(id, res, db).await
}

pub async fn image_handler_default(
    axum::extract::Path(id): axum::extract::Path<u64>,
    axum::extract::State(db): axum::extract::State<database::Database>,
) -> Response {
    handle_image(id, Res::High, db).await
}

pub async fn thumbnail_info_handler(
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