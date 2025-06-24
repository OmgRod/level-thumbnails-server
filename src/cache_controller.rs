struct CloudflareClient {
    api_token: String,
    zone_id: String,
    root_url: String,
    client: reqwest::Client,
}

#[derive(Debug)]
pub struct PurgeError {
    pub status: reqwest::StatusCode,
    pub body: String,
}

static CLOUDFLARE_CLIENT: std::sync::LazyLock<CloudflareClient> =
    std::sync::LazyLock::new(CloudflareClient::new);

impl CloudflareClient {
    pub fn get() -> &'static Self {
        &CLOUDFLARE_CLIENT
    }

    fn new() -> Self {
        let api_token = dotenv::var("CLOUDFLARE_API_KEY")
            .expect("CLOUDFLARE_API_KEY must be set in the environment");

        let zone_id = dotenv::var("CLOUDFLARE_ZONE_ID")
            .expect("CLOUDFLARE_ZONE_ID must be set in the environment");

        let root_url = dotenv::var("HOME_URL").expect("HOME_URL must be set in the environment");

        let client = reqwest::ClientBuilder::new()
            .user_agent(format!(
                "level-thumbnails-server/{}",
                env!("CARGO_PKG_VERSION")
            ))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_token,
            zone_id,
            root_url,
            client,
        }
    }

    pub async fn purge_thumbnail(&self, level_id: i64) -> Result<(), PurgeError> {
        let urls = [
            format!("{}/thumbnail/{}", self.root_url, level_id),
            format!("{}/thumbnail/{}/small", self.root_url, level_id),
            format!("{}/thumbnail/{}/medium", self.root_url, level_id),
            format!("{}/thumbnail/{}/high", self.root_url, level_id),
            format!("{}/thumbnail/{}/info", self.root_url, level_id),
        ];

        let endpoint = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/purge_cache",
            self.zone_id
        );

        let payload = serde_json::json!({ "files": urls });
        let response = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_token)
            .json(&payload)
            .send()
            .await;

        let response = match response {
            Ok(resp) => resp,
            Err(e) => {
                return Err(PurgeError {
                    status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                    body: e.to_string(),
                });
            }
        };

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(PurgeError { status, body: text })
        }
    }
}

pub fn purge(level_id: i64) {
    if dotenv::var("CLOUDFLARE_API_KEY").is_err() {
        eprintln!("CLOUDFLARE_API_KEY is not set, not purging level {}", level_id);
        return;
    }

    tokio::spawn(async move {
        let max_retries = 5;

        for attempt in 1..=max_retries {
            match CloudflareClient::get().purge_thumbnail(level_id).await {
                Ok(_) => {
                    println!(
                        "Purge for id {} succeeded after {} attempt(s)",
                        level_id, attempt
                    );
                    return;
                }
                Err(e) => {
                    if e.status.as_u16() == 429 || e.status.is_server_error() {
                        let delay = 30 * attempt;
                        eprintln!(
                            "Purge failed for id {}: {}. Retrying in {} seconds (attempt {}/{})",
                            level_id, e.body, delay, attempt, max_retries
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    } else {
                        eprintln!("Purge failed for id {}: {}", level_id, e.body);
                        break;
                    }
                }
            }
        }
    });
}