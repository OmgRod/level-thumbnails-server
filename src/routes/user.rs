use crate::{database, util};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;
use std::env;
use crate::auth::UserSession;

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

#[derive(Deserialize, Debug)]
pub struct DiscordOAuthPayload {
    code: String,
}

pub async fn discord_oauth_handler(
    Query(query): Query<DiscordOAuthPayload>,
    State(db): State<database::Database>,
) -> Response {
    if query.code.is_empty() {
        return util::str_response(StatusCode::BAD_REQUEST, "Missing code parameter");
    }

    let client = reqwest::Client::new();

    // Use the code to fetch user info from Discord
    let res = match client
        .post("https://discord.com/api/oauth2/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("client_id", env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID must be set")),
            ("client_secret", env::var("DISCORD_CLIENT_SECRET").expect("DISCORD_CLIENT_SECRET must be set")),
            ("code", query.code),
            ("grant_type", "authorization_code".to_string()),
            ("redirect_uri", format!("{}/user/link", env::var("HOME_URL").expect("HOME_URL must be set"))),
        ])
        .send()
        .await
    {
        Ok(response) => match response.json::<Value>().await {
            Ok(json) => json,
            Err(_) => {
                return util::str_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to parse Discord response",
                );
            }
        },
        Err(_) => {
            return util::str_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch Discord token",
            );
        }
    };

    if !res.get("access_token").is_some() {
        return util::str_response(StatusCode::UNAUTHORIZED, "Invalid Discord code");
    }

    let access_token = res["access_token"].as_str().unwrap();
    let user_info = match client
        .get("https://discord.com/api/users/@me")
        .bearer_auth(access_token)
        .send()
        .await
    {
        Ok(response) => match response.json::<Value>().await {
            Ok(json) => json,
            Err(_) => {
                return util::str_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to parse Discord user info",
                );
            }
        },
        Err(_) => {
            return util::str_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch Discord user info",
            );
        }
    };

    let discord_id = user_info["id"].as_str().unwrap_or("");
    if discord_id.is_empty() {
        return util::str_response(StatusCode::UNAUTHORIZED, "Invalid Discord user info");
    }

    let username = user_info["username"].as_str().unwrap_or("");
    match db.find_or_create_user_discord(discord_id, username).await {
        Ok(user) => {
            let token = UserSession::new(user.id, user.username.clone()).to_jwt();
            Response::builder()
                .status(StatusCode::FOUND)
                .header("Set-Cookie", format!("token={}; HttpOnly; Path=/; SameSite=Lax; Expires=Fri, 31 Dec 9999 23:59:59 GMT", token))
                .header("Location", "/dashboard")
                .body("Redirecting to dashboard...".into())
                .unwrap()
        }
        Err(e) => util::str_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
