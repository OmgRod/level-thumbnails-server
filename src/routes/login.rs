use crate::{auth, database, util};
use auth::UserSession;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
pub struct LoginPayload {
    account_id: i64,
    user_id: i64,
    username: String,
    argon_token: String,
}

fn handle_verdict_error(verdict: auth::Verdict) -> Response {
    let details = match verdict {
        auth::Verdict::Invalid(cause) => format!("Invalid token: {}", cause),
        auth::Verdict::Weak(username) => format!("Weak token for user: {}", username),
        _ => "Authentication failed".to_string(),
    };

    util::response(
        StatusCode::UNAUTHORIZED,
        json!({
            "status": StatusCode::UNAUTHORIZED.as_u16(),
            "error": "Authentication failed",
            "details": details,
        }),
    )
}

pub async fn login(
    State(db): State<database::Database>,
    Json(payload): Json<LoginPayload>,
) -> Response {
    // Validate argon token
    let verdict = match auth::ArgonClient::get()
        .verify(payload.account_id, payload.user_id, &payload.username, &payload.argon_token)
        .await
    {
        Ok(verdict) => verdict,
        Err(e) => {
            return util::response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "status": StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    "error": "Argon verification failed",
                    "details": e.to_string(),
                }),
            );
        }
    };

    // Find or create entry in the database
    match verdict {
        auth::Verdict::Strong => {
            match db.find_or_create_user(payload.account_id, &payload.username).await {
                Ok(user) => util::response(
                    StatusCode::OK,
                    json!({
                        "status": StatusCode::OK.as_u16(),
                        "message": "User authenticated successfully",
                        "user": user,
                        "token": UserSession::new(user.id, user.account_id, payload.username).to_jwt(),
                    }),
                ),
                Err(e) => util::response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "status": StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        "error": "Database error",
                        "details": e.to_string(),
                    }),
                ),
            }
        }
        verdict => handle_verdict_error(verdict),
    }
}
