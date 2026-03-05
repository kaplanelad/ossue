use std::time::Duration;

/// Classifies HTTP errors into retryable vs non-retryable.
#[derive(Debug)]
pub enum ApiError {
    /// 401, 403, 404 — stop sync, don't retry
    NonRetryable { status: u16, message: String },
    /// 429, 5xx, network — retry with backoff
    Retryable {
        message: String,
        retry_after: Option<Duration>,
    },
    /// JSON parse error — log and skip
    ParseError { message: String },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonRetryable { status, message } => write!(f, "HTTP {status}: {message}"),
            Self::Retryable { message, .. } => write!(f, "Retryable: {message}"),
            Self::ParseError { message } => write!(f, "Parse error: {message}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// Classify a reqwest error into ApiError.
pub fn classify_reqwest_error(err: &reqwest::Error) -> ApiError {
    if let Some(status) = err.status() {
        let code = status.as_u16();
        match code {
            401 | 403 | 404 => ApiError::NonRetryable {
                status: code,
                message: err.to_string(),
            },
            429 => ApiError::Retryable {
                message: err.to_string(),
                retry_after: None,
            },
            500..=599 => ApiError::Retryable {
                message: err.to_string(),
                retry_after: None,
            },
            _ => ApiError::NonRetryable {
                status: code,
                message: err.to_string(),
            },
        }
    } else {
        // Network error (timeout, DNS, connection refused)
        ApiError::Retryable {
            message: err.to_string(),
            retry_after: None,
        }
    }
}

/// Classify an HTTP response status code.
pub fn classify_status(status: u16) -> Option<ApiError> {
    match status {
        401 | 403 | 404 => Some(ApiError::NonRetryable {
            status,
            message: format!("HTTP {status}"),
        }),
        429 => Some(ApiError::Retryable {
            message: format!("HTTP {status} Rate Limited"),
            retry_after: None,
        }),
        500..=599 => Some(ApiError::Retryable {
            message: format!("HTTP {status} Server Error"),
            retry_after: None,
        }),
        _ => None,
    }
}

/// Execute an HTTP request with retry and exponential backoff.
///
/// `description` is used in log messages (e.g., "GitHub issues page 3 for owner/repo").
/// `max_retries` is the number of retry attempts (3 means up to 4 total attempts).
/// `fetch_fn` is called on each attempt and should return the reqwest Response.
///
/// Returns the successful Response, or ApiError on final failure.
pub async fn fetch_with_retry<F, Fut>(
    description: &str,
    max_retries: u32,
    mut fetch_fn: F,
) -> Result<reqwest::Response, ApiError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    let mut attempt = 0;
    loop {
        attempt += 1;
        match fetch_fn().await {
            Ok(response) => {
                let status = response.status().as_u16();
                if let Some(err) = classify_status(status) {
                    match &err {
                        ApiError::Retryable { .. } if attempt <= max_retries => {
                            let backoff = Duration::from_secs(1 << (attempt - 1));
                            tracing::warn!(
                                attempt = attempt,
                                max_retries = max_retries,
                                backoff_secs = backoff.as_secs(),
                                description = %description,
                                status = status,
                                "Retryable HTTP error, backing off"
                            );
                            tokio::time::sleep(backoff).await;
                            continue;
                        }
                        _ => return Err(err),
                    }
                }
                return Ok(response);
            }
            Err(e) => {
                let classified = classify_reqwest_error(&e);
                match &classified {
                    ApiError::Retryable { .. } if attempt <= max_retries => {
                        let backoff = Duration::from_secs(1 << (attempt - 1));
                        tracing::warn!(
                            attempt = attempt,
                            max_retries = max_retries,
                            backoff_secs = backoff.as_secs(),
                            description = %description,
                            error = %e,
                            "Retryable network error, backing off"
                        );
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                    _ => return Err(classified),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // -----------------------------------------------------------------------
    // classify_status
    // -----------------------------------------------------------------------

    #[rstest]
    #[case(401)]
    #[case(403)]
    #[case(404)]
    fn classify_status_non_retryable(#[case] status: u16) {
        let err = classify_status(status).expect("should return Some");
        match err {
            ApiError::NonRetryable {
                status: s,
                message: _,
            } => assert_eq!(s, status),
            other => panic!("expected NonRetryable, got {other:?}"),
        }
    }

    #[rstest]
    #[case(429)]
    #[case(500)]
    #[case(502)]
    #[case(503)]
    fn classify_status_retryable(#[case] status: u16) {
        let err = classify_status(status).expect("should return Some");
        assert!(
            matches!(err, ApiError::Retryable { .. }),
            "expected Retryable for {status}"
        );
    }

    #[rstest]
    #[case(200)]
    #[case(201)]
    fn classify_status_success(#[case] status: u16) {
        assert!(
            classify_status(status).is_none(),
            "expected None for success status {status}"
        );
    }

    #[test]
    fn classify_status_other_client_error() {
        // 418 I'm a Teapot — not in our classification list
        let result = classify_status(418);
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // ApiError Display
    // -----------------------------------------------------------------------

    #[test]
    fn api_error_display_non_retryable() {
        let err = ApiError::NonRetryable {
            status: 403,
            message: "forbidden".to_string(),
        };
        assert_eq!(err.to_string(), "HTTP 403: forbidden");
    }

    #[test]
    fn api_error_display_retryable() {
        let err = ApiError::Retryable {
            message: "rate limited".to_string(),
            retry_after: None,
        };
        assert_eq!(err.to_string(), "Retryable: rate limited");
    }

    #[test]
    fn api_error_display_parse_error() {
        let err = ApiError::ParseError {
            message: "bad json".to_string(),
        };
        assert_eq!(err.to_string(), "Parse error: bad json");
    }
}
