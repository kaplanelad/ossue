use serde::{Deserialize, Serialize};

pub const GITHUB_CLIENT_ID: &str = "Ov23liM7NzVENXwaf7Dw";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("Failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),

    #[error("{0}")]
    DeviceFlow(String),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum PollResult {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "success")]
    Success {
        access_token: String,
        token_type: String,
        scope: String,
    },
    #[serde(rename = "slow_down")]
    SlowDown,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "denied")]
    Denied,
    #[serde(rename = "error")]
    Error { message: String },
}

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("ossue")
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client")
}

/// Request a device code from GitHub to begin the OAuth Device Flow.
///
/// The caller should display `user_code` and direct the user to
/// `verification_uri` to authorize the application.
pub async fn request_device_code(client_id: &str, scope: &str) -> Result<DeviceCodeResponse> {
    let client = build_client();
    let resp = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", scope)])
        .send()
        .await?
        .error_for_status()
        .map_err(reqwest::Error::from)?;

    let body = resp.text().await?;
    let device_code: DeviceCodeResponse = serde_json::from_str(&body)?;
    Ok(device_code)
}

/// Poll GitHub for the access token after the user has been sent to the
/// verification URI.
///
/// GitHub returns different JSON shapes for success vs pending/error states.
/// We deserialize into a raw `serde_json::Value` first and then map to the
/// appropriate `PollResult` variant.
pub async fn poll_for_token(client_id: &str, device_code: &str) -> Result<PollResult> {
    let client = build_client();
    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", device_code),
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code",
            ),
        ])
        .send()
        .await?
        .error_for_status()
        .map_err(reqwest::Error::from)?;

    let body = resp.text().await?;
    let value: serde_json::Value = serde_json::from_str(&body)?;

    // GitHub signals errors via an `error` field rather than HTTP status codes.
    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return match error {
            "authorization_pending" => Ok(PollResult::Pending),
            "slow_down" => Ok(PollResult::SlowDown),
            "expired_token" => Ok(PollResult::Expired),
            "access_denied" => Ok(PollResult::Denied),
            other => Ok(PollResult::Error {
                message: value
                    .get("error_description")
                    .and_then(|v| v.as_str())
                    .unwrap_or(other)
                    .to_string(),
            }),
        };
    }

    // On success GitHub returns `access_token`, `token_type`, and `scope`.
    if let Some(access_token) = value.get("access_token").and_then(|v| v.as_str()) {
        let token_type = value
            .get("token_type")
            .and_then(|v| v.as_str())
            .unwrap_or("bearer")
            .to_string();
        let scope = value
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        return Ok(PollResult::Success {
            access_token: access_token.to_string(),
            token_type,
            scope,
        });
    }

    Err(Error::DeviceFlow(format!(
        "Unexpected response from GitHub: {body}"
    )))
}
