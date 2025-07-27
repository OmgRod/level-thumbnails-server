use crate::{database, util};
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use image::ImageReader;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use webp::Encoder;

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub enum Res {
    #[serde(rename = "high")]
    High, // 1920x1080
    #[serde(rename = "medium")]
    Medium, // 1280x720
    #[serde(rename = "small")]
    Small, // 640x360
}

impl Res {
    fn dimensions(&self) -> (u32, u32) {
        match self {
            Res::High => (1920, 1080),
            Res::Medium => (1280, 720),
            Res::Small => (640, 360),
        }
    }
}

impl std::fmt::Display for Res {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Res::High => write!(f, "high"),
            Res::Medium => write!(f, "medium"),
            Res::Small => write!(f, "small"),
        }
    }
}

fn image_response(image_data: Vec<u8>, id: u64, upload_info: &database::UploadInfo) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "image/webp")
        .header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{}.webp\"", id))
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header(header::CONTENT_LENGTH, image_data.len())
        .header("X-Level-ID", id.to_string())
        .header("X-Thumbnail-Author", &upload_info.username)
        .header("X-Thumbnail-User-ID", upload_info.account_id.to_string())
        .body(image_data.into())
        .unwrap()
}

async fn get_upload_info(
    db: &database::Database,
    id: u64,
) -> Result<database::UploadInfo, Response> {
    match db.get_upload_info(id as i64).await {
        Some(upload) => Ok(upload),
        None => Err(util::str_response(StatusCode::NOT_FOUND, "Image not found")),
    }
}

async fn read_original_image(image_path: &PathBuf) -> Result<Vec<u8>, Response> {
    tokio::fs::read(image_path).await.map_err(|e| {
        util::str_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to read image file: {}", e),
        )
    })
}

async fn resize_image(image_path: PathBuf, target_res: Res) -> Result<Vec<u8>, Response> {
    let (width, height) = target_res.dimensions();

    tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let image = ImageReader::open(&image_path)
            .map_err(|e| format!("Failed to open image: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        let resized_image =
            image.resize_exact(width, height, image::imageops::FilterType::Lanczos3).to_rgb8();

        Ok(Encoder::from_rgb(&resized_image, width, height).encode_lossless().to_vec())
    })
    .await
    .map_err(|e| {
        util::str_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Task join error: {}", e))
    })?
    .map_err(|e| {
        util::str_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Image processing error: {}", e),
        )
    })
}

async fn handle_image(id: u64, res: Res, db: database::Database) -> Response {
    // Check if image file exists
    let image_path = PathBuf::from(format!("thumbnails/{}.webp", id));
    if !image_path.exists() {
        return util::str_response(StatusCode::NOT_FOUND, "Image not found");
    }

    // Verify image exists in database and get metadata
    let upload_info = match get_upload_info(&db, id).await {
        Ok(info) => info,
        Err(response) => return response,
    };

    match res {
        Res::High => {
            // For high resolution, serve the original image
            let image_data = match read_original_image(&image_path).await {
                Ok(data) => data,
                Err(response) => return response,
            };

            image_response(image_data, id, &upload_info)
        }

        Res::Medium | Res::Small => {
            // For lower resolutions, resize the image
            let resized_data = match resize_image(image_path, res).await {
                Ok(data) => data,
                Err(response) => return response,
            };

            image_response(resized_data, id, &upload_info)
        }
    }
}

pub async fn image_handler_with_res(
    Path((id, res)): Path<(u64, Res)>,
    State(db): State<database::Database>,
) -> Response {
    handle_image(id, res, db).await
}

pub async fn image_handler_default(
    Path(id): Path<u64>,
    State(db): State<database::Database>,
) -> Response {
    handle_image(id, Res::High, db).await
}

pub async fn thumbnail_info_handler(
    Path(id): Path<u64>,
    State(db): State<database::Database>,
) -> Response {
    match db.get_upload_extended(id as i64).await {
        Some(upload) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CACHE_CONTROL, "no-store")
            .body(serde_json::to_string(&upload).unwrap().into())
            .unwrap(),
        None => util::str_response(StatusCode::NOT_FOUND, "Image not found"),
    }
}

pub async fn handle_random(res: Res) -> Response {
    // pick random id from directory
    match tokio::fs::read_dir("thumbnails").await {
        Ok(mut entries) => {
            let mut ids: Vec<u64> = Vec::new();
            while let Some(entry) = entries.next_entry().await.unwrap() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Ok(id) = name.trim_end_matches(".webp").parse::<u64>() {
                        ids.push(id);
                    }
                }
            }

            if ids.is_empty() {
                return util::str_response(StatusCode::NOT_FOUND, "No images found");
            }

            let random_id = ids[rand::random::<usize>() % ids.len()];
            let url = format!("/thumbnail/{}/{}", random_id, res.to_string());
            Response::builder()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, url)
                .body("".into())
                .unwrap()
        }
        Err(e) => util::str_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to get thumbnails: {}", e),
        ),
    }
}

pub async fn random_handler() -> Response {
    handle_random(Res::High).await
}

pub async fn random_res_handler(Path(res): Path<Res>) -> Response {
    handle_random(res).await
}
