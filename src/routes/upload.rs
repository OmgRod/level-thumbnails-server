use crate::auth::UserSession;
use crate::{cache_controller, database};
use axum::Form;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use webp::Encoder;

// handler for uploading images for admins/moderators (and verified for new thumbnails) that directly saves the image
async fn force_save(
    id: u64,
    image_data: &[u8],
    user: &database::User,
    db: &database::Database,
) -> Result<(), String> {
    let image_path = format!("thumbnails/{}.webp", id);
    match tokio::fs::write(&image_path, image_data).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("failed to save image: {}", e)),
    }?;

    match db.add_upload(id as i64, user.id, &image_path, true).await {
        Ok(_) => {
            cache_controller::purge(id as i64);
            Ok(())
        }
        Err(e) => Err(format!("failed to add upload entry: {}", e)),
    }
}

async fn add_to_pending(
    id: u64,
    image_data: &[u8],
    user: &database::User,
    db: &database::Database,
) -> Result<(), (StatusCode, String)> {
    let image_path = format!("uploads/{}_{}.webp", user.id, id);
    match tokio::fs::write(&image_path, image_data).await {
        Ok(_) => Ok(()),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to save image: {}", e),
        )),
    }?;

    match db.add_upload(id as i64, user.id, &image_path, false).await {
        Ok(_) => Ok(()),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to add upload entry: {}", e),
        )),
    }
}

async fn is_image_uploaded(id: u64) -> bool {
    let image_path = format!("thumbnails/{}.webp", id);
    tokio::fs::metadata(&image_path)
        .await
        .map(|_| true)
        .unwrap_or(false)
}

async fn auth_middleware(
    headers: &HeaderMap,
    db: &database::Database,
) -> Result<database::User, (StatusCode, String)> {
    match headers.get("Authorization").and_then(|h| h.to_str().ok()) {
        Some(token) => match UserSession::from_jwt(token) {
            Ok(session) => match db.get_user_by_id(session.id).await {
                Some(user) => Ok(user),
                None => Err((StatusCode::FORBIDDEN, "User not found".to_string())),
            },
            Err(e) => Err((StatusCode::UNAUTHORIZED, format!("Unauthorized: {}", e))),
        },
        None => Err((StatusCode::UNAUTHORIZED, "Unauthorized".to_string())),
    }
}

pub async fn upload(
    State(db): State<database::Database>,
    headers: HeaderMap,
    Path(id): Path<u64>,
    data: Bytes,
) -> impl IntoResponse {
    // Authenticate user
    let user = match auth_middleware(&headers, &db).await {
        Ok(user) => user,
        Err((status, message)) => return (status, message),
    };

    // early return if they already have a pending thumbnail
    if user.role == database::Role::User || user.role == database::Role::Verified {
        let image_path = format!("uploads/{}_{}.webp", user.id, id);
        if tokio::fs::metadata(&image_path).await.is_ok() {
            return (
                StatusCode::CONFLICT,
                format!("you already have a pending thumbnail for id {}", id),
            );
        }
    }

    let image = match image::load_from_memory(&data) {
        Ok(img) => img,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid image data: {}", e),
            );
        }
    };

    // Check image dimensions
    if image.width() != 1920 || image.height() != 1080 {
        return (
            StatusCode::BAD_REQUEST,
            "Image must be 1920x1080".to_string(),
        );
    }

    // Convert image to WebP format
    let data = image.into_rgb8();
    let encoder = Encoder::from_rgb(&data, 1920, 1080);
    let webp_data = encoder.encode_lossless().to_owned();

    match user.role {
        // admins and moderators can upload and replace images
        database::Role::Admin | database::Role::Moderator => {
            return match force_save(id, &webp_data, &user, &db).await {
                Ok(_) => (
                    StatusCode::CREATED,
                    format!("Image for level ID {} uploaded", id),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error saving image: {}", e),
                ),
            };
        }
        // verified users can upload images, but replacing images requires approval
        database::Role::Verified => {
            if !is_image_uploaded(id).await {
                return match force_save(id, &webp_data, &user, &db).await {
                    Ok(_) => (
                        StatusCode::OK,
                        format!("Image for level ID {} replaced", id),
                    ),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Error saving image: {}", e),
                    ),
                };
            }
        }
        // regular users have to be verified for any uploads
        database::Role::User => {}
    };

    match add_to_pending(id, &webp_data, &user, &db).await {
        Ok(_) => (
            StatusCode::ACCEPTED,
            format!("image for level ID {} is now pending", id),
        ),
        Err(e) => e,
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
) -> Result<(StatusCode, String), (StatusCode, String)> {
    let user = match auth_middleware(&headers, &db).await {
        Ok(user) => user,
        Err((status, message)) => return Err((status, message)),
    };

    if (user.role != database::Role::Moderator && user.role != database::Role::Admin)
        || (filter == PendingFilter::ByUser(user.id))
    {
        return Err((
            StatusCode::FORBIDDEN,
            "Only moderators or admins can view pending uploads".to_string(),
        ));
    }

    let res = match filter {
        PendingFilter::All => db.get_pending_uploads().await,
        PendingFilter::ByLevel(level_id) => db.get_pending_uploads_for_level(level_id).await,
        PendingFilter::ByUser(user_id) => db.get_pending_uploads_for_user(user_id).await,
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error fetching pending uploads: {}", e),
        )
    });

    match res {
        Ok(uploads) => Ok((StatusCode::OK, serde_json::to_string(&uploads).unwrap())),
        Err(e) => Err(e),
    }
}

pub async fn get_pending_uploads_for_level(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    get_pending_uploads(headers, &db, PendingFilter::ByLevel(id)).await
}

pub async fn get_all_pending_uploads(
    headers: HeaderMap,
    State(db): State<database::Database>,
) -> impl IntoResponse {
    get_pending_uploads(headers, &db, PendingFilter::All).await
}

pub async fn get_pending_uploads_for_user(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    get_pending_uploads(headers, &db, PendingFilter::ByUser(id)).await
}

pub async fn get_pending_info(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let user = match auth_middleware(&headers, &db).await {
        Ok(user) => user,
        Err((status, message)) => return (status, message),
    };

    if user.role != database::Role::Moderator && user.role != database::Role::Admin {
        return (
            StatusCode::FORBIDDEN,
            "Only moderators or admins can view pending upload info".to_string(),
        );
    }

    match db.get_pending_upload(id).await {
        Ok(upload) => (StatusCode::OK, serde_json::to_string(&upload).unwrap()),
        Err(e) => (
            StatusCode::NOT_FOUND,
            format!("No pending upload found with ID {}: {}", id, e),
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
    Form(action): Form<PendingUploadAction>,
) -> impl IntoResponse {
    let user = match auth_middleware(&headers, &db).await {
        Ok(user) => user,
        Err((status, message)) => return (status, message),
    };

    if user.role != database::Role::Moderator && user.role != database::Role::Admin {
        return (
            StatusCode::FORBIDDEN,
            "Only moderators or admins can perform this action".to_string(),
        );
    }

    let upload = match db.get_pending_upload(id).await {
        Ok(upload) => upload,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                format!("No pending upload found with ID {}", id),
            );
        }
    };

    // Check if the upload is already accepted
    if upload.accepted {
        return (
            StatusCode::CONFLICT,
            "This upload has already been accepted".to_string(),
        );
    }

    let old_image_path = format!("uploads/{}_{}.webp", upload.user_id, upload.level_id);
    if action.accepted {
        // move the image from pending to thumbnails
        let new_image_path = format!("thumbnails/{}.webp", upload.level_id);
        match tokio::fs::rename(&old_image_path, &new_image_path).await {
            Ok(_) => {
                if let Err(e) = db
                    .accept_upload(upload.id, user.id, action.reason, true)
                    .await
                {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Error accepting upload: {}", e),
                    );
                }
                cache_controller::purge(upload.level_id);
                (StatusCode::OK, format!("Upload {} accepted", id))
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error moving image: {}", e),
            ),
        }
    } else {
        // if rejected, delete the image and update the database
        if let Err(e) = tokio::fs::remove_file(&old_image_path).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error deleting image: {}", e),
            );
        }
        if let Err(e) = db
            .accept_upload(upload.id, user.id, action.reason, false)
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error rejecting upload: {}", e),
            );
        }
        (StatusCode::OK, format!("Upload {} rejected", id))
    }
}

pub async fn get_pending_image(
    headers: HeaderMap,
    State(db): State<database::Database>,
    Path(id): Path<i64>,
) -> Response {
    let user = match auth_middleware(&headers, &db).await {
        Ok(user) => user,
        Err((status, message)) => {
            return Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(message.into())
                .unwrap();
        }
    };

    if user.role != database::Role::Moderator && user.role != database::Role::Admin {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "text/plain")
            .body("Only moderators or admins can view pending images".into())
            .unwrap();
    }

    match db.get_pending_upload(id).await {
        Ok(upload) => {
            let image_path = format!("uploads/{}_{}.webp", upload.user_id, upload.level_id);
            let image_data = match tokio::fs::read(&image_path).await {
                Ok(data) => data,
                Err(e) => {
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body(format!("Error reading image: {}", e).into())
                        .unwrap();
                }
            };

            Response::builder()
                .header(header::CONTENT_TYPE, "image/webp")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!(
                        "inline; filename=\"pending_{}_{}.webp\"",
                        upload.user_id, id
                    ),
                )
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .header(header::CONTENT_LENGTH, image_data.len())
                .body(image_data.into())
                .unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(format!("No pending upload found with ID {}: {}", id, e).into())
            .unwrap(),
    }
}
