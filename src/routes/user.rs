use crate::{database, util};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

pub async fn get_user_info(id: i64, db: &database::Database) -> Response {
    match db.get_user_stats(id).await {
        Some(user) => util::response(
            StatusCode::OK,
            serde_json::json!({
                "status": StatusCode::OK.as_u16(),
                "data": user,
            }),
        ),
        None => util::str_response(StatusCode::NOT_FOUND, "User not found"),
    }
}

pub async fn get_me(headers: HeaderMap, State(db): State<database::Database>) -> Response {
    match util::auth_middleware(&headers, &db).await {
        Ok(user) => get_user_info(user.id, &db).await,
        Err(response) => response,
    }
}

pub async fn get_user_by_id(Path(id): Path<i64>, State(db): State<database::Database>) -> Response {
    get_user_info(id, &db).await
}
