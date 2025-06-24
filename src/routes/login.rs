use crate::auth::UserSession;
use crate::{auth, database};
use axum::extract::{Form, State};
use axum::http::{StatusCode, header};
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

pub async fn login(
    State(db): State<database::Database>,
    Form(payload): Form<LoginPayload>,
) -> Response {
    // Validate argon token
    let verdict = match auth::ArgonClient::get()
        .verify(
            payload.account_id,
            payload.user_id,
            &payload.username,
            &payload.argon_token,
        )
        .await
    {
        Ok(verdict) => verdict,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain")
                .header(header::CACHE_CONTROL, "no-store")
                .body(format!("Error verifying token: {}", e).into())
                .unwrap();
        }
    };

    match verdict {
        auth::Verdict::Strong => {
            // Find or create entry in the database
            match db
                .find_or_create_user(payload.account_id, &payload.username)
                .await
            {
                Ok(user) => Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::CACHE_CONTROL, "no-store")
                    .body(
                        json!({
                            "token": UserSession::new(
                                user.id,
                                user.account_id,
                                payload.username,
                            ).to_jwt(),
                            "user": user,
                        })
                        .to_string()
                        .into(),
                    )
                    .unwrap(),
                Err(e) => Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .header(header::CACHE_CONTROL, "no-store")
                    .body(format!("Error creating user: {}", e).into())
                    .unwrap(),
            }
        }
        _ => {
            // Return error response based on verdict
            let message = match verdict {
                auth::Verdict::Invalid(cause) => format!("Invalid token: {}", cause),
                auth::Verdict::Weak(username) => format!("Weak token for user: {}", username),
                _ => "Unknown error".to_string(),
            };
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "text/plain")
                .header(header::CACHE_CONTROL, "no-store")
                .body(message.into())
                .unwrap()
        }
    }
}
