use crate::{cache_controller, database, util};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use webp::Encoder;

const IMAGE_WIDTH: u32 = 1920;
const IMAGE_HEIGHT: u32 = 1080;

// Helper function to authenticate moderator/admin
async fn authenticate_moderator(
    headers: &HeaderMap,
    db: &database::Database,
) -> Result<database::User, Response> {
    let user = util::auth_middleware(headers, db).await?;

    if !matches!(user.role, database::Role::Moderator | database::Role::Admin) {
        return Err(util::str_response(
            StatusCode::FORBIDDEN,
            "Only moderators or admins can perform this action",
        ));
    }

    Ok(user)
}

// Helper function to validate image dimensions and convert to WebP
fn process_image(data: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(data).map_err(|e| format!("Invalid image data: {}", e))?;

    if image.width() != IMAGE_WIDTH || image.height() != IMAGE_HEIGHT {
        return Err(format!("Image must be exactly {}x{}", IMAGE_WIDTH, IMAGE_HEIGHT));
    }

    let rgb_data = image.into_rgb8();
    let encoder = Encoder::from_rgb(&rgb_data, IMAGE_WIDTH, IMAGE_HEIGHT);
    Ok(encoder.encode_lossless().to_owned())
}

// Handler for uploading images for admins/moderators (and verified for new thumbnails)
async fn force_save(
    id: u64,
    image_data: &[u8],
    user: &database::User,
    db: &database::Database,
) -> Result<(), String> {
    let image_path = format!("thumbnails/{}.webp", id);

    tokio::fs::write(&image_path, image_data)
        .await
        .map_err(|e| format!("Failed to save image: {}", e))?;

    db.add_upload(id as i64, user.id, &image_path, true)
        .await
        .map_err(|e| format!("Failed to add upload entry: {}", e))?;

    cache_controller::purge(id as i64);
    Ok(())
}

async fn add_to_pending(
    id: u64,
    image_data: &[u8],
    user: &database::User,
    db: &database::Database,
) -> Response {
    let image_path = format!("uploads/{}_{}.webp", user.id, id);

    match tokio::fs::write(&image_path, image_data).await {
        Ok(_) => {}
        Err(e) => {
            return util::str_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to save pending image: {}", e),
            );
        }
    }

    match db.add_upload(id as i64, user.id, &image_path, false).await {
        Ok(_) => util::str_response(
            StatusCode::ACCEPTED,
            &format!("Image for level ID {} is now pending", id),
        ),
        Err(e) => util::str_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to add pending upload entry: {}", e),
        ),
    }
}

async fn has_pending_upload(user_id: i64, level_id: u64) -> bool {
    let image_path = format!("uploads/{}_{}.webp", user_id, level_id);
    tokio::fs::metadata(&image_path).await.is_ok()
}

async fn is_image_uploaded(id: u64) -> bool {
    let image_path = format!("thumbnails/{}.webp", id);
    tokio::fs::metadata(&image_path).await.is_ok()
}

pub async fn upload(
    State(db): State<database::Database>,
    headers: HeaderMap,
    Path(id): Path<u64>,
    data: Bytes,
) -> Response {
    let user = match util::auth_middleware(&headers, &db).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    // Check for existing pending uploads for regular and verified users
    if matches!(user.role, database::Role::User | database::Role::Verified) {
        if has_pending_upload(user.id, id).await {
            return util::str_response(
                StatusCode::CONFLICT,
                &format!("You already have a pending thumbnail for level ID {}", id),
            );
        }
    }

    // Process and validate the image
    let webp_data = match process_image(&data) {
        Ok(data) => data,
        Err(e) => return util::str_response(StatusCode::BAD_REQUEST, &e),
    };

    match user.role {
        // Admins and moderators can upload and replace images directly
        database::Role::Admin | database::Role::Moderator => {
            match force_save(id, &webp_data, &user, &db).await {
                Ok(_) => util::str_response(
                    StatusCode::CREATED,
                    &format!("Image for level ID {} uploaded", id),
                ),
                Err(e) => util::str_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Error saving image: {}", e),
                ),
            }
        }

        // Verified users can upload new images directly, but replacements need approval
        database::Role::Verified => {
            if !is_image_uploaded(id).await {
                match force_save(id, &webp_data, &user, &db).await {
                    Ok(_) => util::str_response(
                        StatusCode::CREATED,
                        &format!("Image for level ID {} uploaded", id),
                    ),
                    Err(e) => util::str_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Error saving image: {}", e),
                    ),
                }
            } else {
                // Image exists, add to pending for approval
                add_to_pending(id, &webp_data, &user, &db).await
            }
        }

        // Regular users must go through approval process
        database::Role::User => add_to_pending(id, &webp_data, &user, &db).await,
    }
}

#[derive(PartialEq)]
enum PendingFilter {
    All,
    ByLevel(i64),
    ByUser(i64),
}

async fn get_pending_uploads(
    headers: HeaderMap,
    db: &database::Database,
    filter: PendingFilter,
) -> Response {
    let user = match authenticate_moderator(&headers, db).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    // Special case: users can view their own pending uploads
    if let PendingFilter::ByUser(user_id) = filter {
        if user.id != user_id {
            return util::str_response(
                StatusCode::FORBIDDEN,
                "You can only view your own pending uploads",
            );
        }
    }

    let uploads_result = match filter {
        PendingFilter::All => db.get_pending_uploads().await,
        PendingFilter::ByLevel(level_id) => db.get_pending_uploads_for_level(level_id).await,
        PendingFilter::ByUser(user_id) => db.get_pending_uploads_for_user(user_id).await,
    };

    match uploads_result {
        Ok(mut uploads) => {
            for upload in &mut uploads {
                upload.replacement = is_image_uploaded(upload.level_id as u64).await;
            }

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(serde_json::to_string(&uploads).unwrap().into())
                .unwrap()
        }
        Err(e) => util::str_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Error fetching pending uploads: {}", e),
        ),
    }
}

pub async fn get_pending_uploads_for_level(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
) -> Response {
    get_pending_uploads(headers, &db, PendingFilter::ByLevel(id)).await
}

pub async fn get_all_pending_uploads(
    headers: HeaderMap,
    State(db): State<database::Database>,
) -> Response {
    get_pending_uploads(headers, &db, PendingFilter::All).await
}

pub async fn get_pending_uploads_for_user(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
) -> Response {
    get_pending_uploads(headers, &db, PendingFilter::ByUser(id)).await
}

pub async fn get_pending_info(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
) -> Response {
    let _user = match authenticate_moderator(&headers, &db).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    match db.get_pending_upload(id).await {
        Ok(upload) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&upload).unwrap().into())
            .unwrap(),
        Err(e) => util::str_response(
            StatusCode::NOT_FOUND,
            &format!("No pending upload found with ID {}: {}", id, e),
        ),
    }
}

#[derive(Deserialize, Serialize)]
pub struct PendingUploadAction {
    pub accepted: bool,
    pub reason: Option<String>,
}

pub async fn pending_action(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
    Json(action): Json<PendingUploadAction>,
) -> Response {
    let user = match authenticate_moderator(&headers, &db).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let upload = match db.get_pending_upload(id).await {
        Ok(upload) => upload,
        Err(e) => {
            return util::str_response(
                StatusCode::NOT_FOUND,
                &format!("No pending upload found with ID {}: {}", id, e),
            );
        }
    };

    if upload.accepted {
        return util::str_response(StatusCode::CONFLICT, "This upload has already been accepted");
    }

    let old_image_path = format!("uploads/{}_{}.webp", upload.user_id, upload.level_id);

    if action.accepted {
        // Accept: move image from uploads to thumbnails
        let new_image_path = format!("thumbnails/{}.webp", upload.level_id);

        if let Err(e) = tokio::fs::rename(&old_image_path, &new_image_path).await {
            return util::str_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Error moving image: {}", e),
            );
        }

        if let Err(e) = db.accept_upload(upload.id, user.id, action.reason, true).await {
            return util::str_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Error accepting upload: {}", e),
            );
        }

        cache_controller::purge(upload.level_id);
        util::str_response(StatusCode::OK, &format!("Upload {} accepted", id))
    } else {
        // Reject: delete the pending image
        if let Err(e) = tokio::fs::remove_file(&old_image_path).await {
            return util::str_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Error deleting image: {}", e),
            );
        }

        if let Err(e) = db.accept_upload(upload.id, user.id, action.reason, false).await {
            return util::str_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Error rejecting upload: {}", e),
            );
        }

        util::str_response(StatusCode::OK, &format!("Upload {} rejected", id))
    }
}

pub async fn get_pending_image(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
) -> Response {
    let _user = match authenticate_moderator(&headers, &db).await {
        Ok(user) => user,
        Err(response) => return response,
    };

    let upload = match db.get_pending_upload(id).await {
        Ok(upload) => upload,
        Err(e) => {
            return util::str_response(
                StatusCode::NOT_FOUND,
                &format!("No pending upload found with ID {}: {}", id, e),
            );
        }
    };

    let image_path = format!("uploads/{}_{}.webp", upload.user_id, upload.level_id);
    let image_data = match tokio::fs::read(&image_path).await {
        Ok(data) => data,
        Err(e) => {
            return util::str_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Error reading image file: {}", e),
            );
        }
    };

    Response::builder()
        .header(header::CONTENT_TYPE, "image/webp")
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"pending_{}_{}.webp\"", upload.user_id, id),
        )
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header(header::CONTENT_LENGTH, image_data.len())
        .body(image_data.into())
        .unwrap()
}
