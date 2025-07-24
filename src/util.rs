use crate::auth::UserSession;
use crate::database;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use serde_json::json;

pub fn response(status: StatusCode, body: serde_json::Value) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "no-store")
        .body(body.to_string().into())
        .unwrap()
}

pub fn str_response(status: StatusCode, message: &str) -> Response {
    response(
        status,
        json!({
            "status": status.as_u16(),
            "message": message,
        }),
    )
}

pub async fn auth_middleware(
    headers: &HeaderMap,
    db: &database::Database,
) -> Result<database::User, Response> {
    match headers.get("Authorization").and_then(|h| h.to_str().ok()) {
        Some(token) => match UserSession::from_jwt(token) {
            Ok(session) => match db.get_user_by_id(session.id).await {
                Some(user) => Ok(user),
                None => Err(str_response(
                    StatusCode::FORBIDDEN,
                    "User not found"
                )),
            },
            Err(e) => Err(str_response(StatusCode::UNAUTHORIZED, &e.to_string())),
        },
        None => Err(str_response(
            StatusCode::UNAUTHORIZED,
            "Missing Authorization header",
        )),
    }
}
