use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserSession {
    pub id: i64,
    pub username: String,
}

impl UserSession {
    pub fn new(id: i64, username: String) -> Self {
        Self { id, username }
    }

    pub fn to_jwt(&self) -> String {
        let jwt_secret = dotenv::var("JWT_SECRET").expect("JWT_SECRET must be set");
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &self,
            &jsonwebtoken::EncodingKey::from_secret(jwt_secret.as_bytes()),
        )
        .expect("Failed to encode JWT")
    }

    pub fn from_jwt(token: &str) -> Result<Self, jsonwebtoken::errors::Error> {
        // strip the "Bearer " prefix if it exists
        let token = if token.starts_with("Bearer ") { &token[7..] } else { token };

        let mut validation = jsonwebtoken::Validation::default();
        validation.validate_exp = false;
        validation.required_spec_claims = HashSet::new();

        let jwt_secret = dotenv::var("JWT_SECRET").expect("JWT_SECRET must be set");
        jsonwebtoken::decode::<UserSession>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes()),
            &validation,
        )
        .map(|data| data.claims)
    }
}

// ArgonClient implementation taken from Globed:
// https://github.com/GlobedGD/globed2/blob/main/server/central/src/argon_client.rs

pub struct ArgonClient {
    base_url: String,
    client: reqwest::Client,
}

pub enum Verdict {
    Strong,
    Weak(String),
    Invalid(String),
}

pub enum ArgonClientError {
    RequestFailed(reqwest::Error),
    InvalidJson(serde_json::Error),
    ArgonError(String),
}

impl From<reqwest::Error> for ArgonClientError {
    fn from(value: reqwest::Error) -> Self {
        Self::RequestFailed(value)
    }
}

impl From<serde_json::Error> for ArgonClientError {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidJson(value)
    }
}

impl Display for ArgonClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestFailed(err) => write!(f, "request failed: {err}"),
            Self::InvalidJson(msg) => write!(f, "invalid server response: {msg}"),
            Self::ArgonError(msg) => write!(f, "error from the server: {msg}"),
        }
    }
}

// #[derive(Deserialize)]
// pub struct ArgonStatus {
//     pub active: bool,
//     pub total_nodes: usize,
//     pub active_nodes: usize,
//     pub ident: String,
// }

#[derive(Deserialize)]
struct ArgonResponse {
    pub valid: bool,
    pub valid_weak: bool,
    #[serde(default)]
    pub cause: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

static ARGON_CLIENT: std::sync::LazyLock<ArgonClient> = std::sync::LazyLock::new(ArgonClient::new);

impl ArgonClient {
    pub fn get() -> &'static Self {
        &ARGON_CLIENT
    }

    pub fn new() -> Self {
        let base_url = dotenv::var("ARGON_BASE_URL")
            .unwrap_or_else(|_| "https://argon.globed.dev/v1".to_string());
        let client = reqwest::ClientBuilder::new()
            .user_agent(format!("level-thumbnails-server/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self { base_url, client }
    }

    pub async fn verify(
        &self,
        account_id: i64,
        user_id: i64,
        username: &str,
        token: &str,
    ) -> Result<Verdict, ArgonClientError> {
        let url = format!("{}/validation/check_strong", self.base_url);
        let response = self
            .client
            .get(&url)
            .query(&[
                ("account_id", account_id.to_string().as_str()),
                ("user_id", user_id.to_string().as_str()),
                ("username", username),
                ("authtoken", token),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let resp = response.text().await?;
            return Err(ArgonClientError::ArgonError(resp));
        }

        let response: ArgonResponse = match serde_json::from_str(&response.text().await?) {
            Ok(x) => x,
            Err(e) => return Err(ArgonClientError::InvalidJson(e)),
        };

        if !response.valid_weak {
            Ok(Verdict::Invalid(response.cause.unwrap_or_else(|| "unknown".to_owned())))
        } else if !response.valid {
            Ok(Verdict::Weak(response.username.unwrap_or_else(|| "<unknown>".to_owned())))
        } else {
            Ok(Verdict::Strong)
        }
    }
}
