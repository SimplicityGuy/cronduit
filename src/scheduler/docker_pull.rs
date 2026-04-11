//! Image pull with retry, error classification, and digest extraction.
//!
//! D-01: Each retry attempt logged with reason + backoff duration.
//! D-02: Terminal errors (unauthorized, manifest unknown) fail immediately.
//! D-03: Successful pull logs only the resolved digest.

use bollard::Docker;
use bollard::query_parameters::CreateImageOptionsBuilder;
use futures_util::StreamExt;

/// Classification of image pull errors.
#[derive(Debug)]
pub enum PullError {
    /// Retryable: network timeout, connection refused, server 5xx.
    Transient(String),
    /// Terminal: manifest unknown, unauthorized, invalid reference.
    Terminal(String),
}

impl std::fmt::Display for PullError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transient(msg) => write!(f, "transient pull error: {msg}"),
            Self::Terminal(msg) => write!(f, "terminal pull error: {msg}"),
        }
    }
}

impl std::error::Error for PullError {}

/// Classify a bollard error as terminal (no retry) or transient (retry).
fn classify_pull_error(err: &bollard::errors::Error) -> PullError {
    let msg = err.to_string();
    let lower = msg.to_lowercase();

    if lower.contains("unauthorized")
        || lower.contains("authentication required")
        || lower.contains("manifest unknown")
        || lower.contains("not found")
        || lower.contains("invalid reference")
    {
        PullError::Terminal(msg)
    } else {
        // Connection refused, timeout, 5xx, etc. -- transient.
        PullError::Transient(msg)
    }
}

/// Pull an image with retry and exponential backoff.
///
/// Returns `Ok(Some(digest))` if the digest was extracted from the pull response,
/// `Ok(None)` if the pull succeeded but no digest was found, or `Err(PullError)`.
///
/// Backoff schedule: 1s, 2s, 4s (doubling).
pub async fn pull_image_with_retry(
    docker: &Docker,
    image: &str,
    max_attempts: u32,
) -> Result<Option<String>, PullError> {
    // Parse image into repository (with implicit :latest).
    let from_image = if image.contains(':') || image.contains('@') {
        image.to_string()
    } else {
        format!("{image}:latest")
    };

    let backoffs: [u64; 3] = [1, 2, 4];

    for attempt in 1..=max_attempts {
        let opts = CreateImageOptionsBuilder::default()
            .from_image(from_image.as_str())
            .build();

        let mut stream = docker.create_image(Some(opts), None, None);
        let mut digest: Option<String> = None;
        let mut last_error: Option<bollard::errors::Error> = None;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(info) => {
                    // Extract digest from status field: "Digest: sha256:..."
                    if let Some(ref status) = info.status
                        && let Some(sha) = status.strip_prefix("Digest: ")
                    {
                        digest = Some(sha.to_string());
                    }
                    // Also check the id field for digest.
                    if let Some(ref id) = info.id
                        && id.starts_with("sha256:")
                    {
                        digest = Some(id.clone());
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                    break;
                }
            }
        }

        if let Some(err) = last_error {
            let classified = classify_pull_error(&err);
            match classified {
                PullError::Terminal(_) => {
                    // D-02: Fail fast on terminal errors.
                    return Err(classified);
                }
                PullError::Transient(ref reason) => {
                    if attempt < max_attempts {
                        let backoff_secs =
                            backoffs.get((attempt - 1) as usize).copied().unwrap_or(4);
                        // D-01: Verbose retry logging.
                        tracing::warn!(
                            target: "cronduit.docker.pull",
                            image,
                            attempt,
                            max_attempts,
                            reason = %reason,
                            backoff_secs,
                            "pull attempt failed, retrying"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                    } else {
                        return Err(classified);
                    }
                }
            }
        } else {
            // D-03: Silent success -- log only the digest.
            tracing::info!(
                target: "cronduit.docker.pull",
                image,
                digest = %digest.as_deref().unwrap_or("unknown"),
                "image pulled"
            );
            return Ok(digest);
        }
    }

    // Should not reach here, but handle gracefully.
    Err(PullError::Transient(format!(
        "exhausted {max_attempts} pull attempts for {image}"
    )))
}

/// Check if an image exists locally and return its digest if found.
pub async fn image_exists_locally(
    docker: &Docker,
    image: &str,
) -> Result<Option<String>, bollard::errors::Error> {
    match docker.inspect_image(image).await {
        Ok(info) => {
            // Extract digest from repo_digests (first sha256 portion).
            let digest = info.repo_digests.and_then(|digests| {
                digests
                    .iter()
                    .find_map(|d| d.split('@').nth(1).map(|sha| sha.to_string()))
            });
            Ok(digest)
        }
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Ensure an image is available locally, pulling if necessary.
///
/// Returns `Ok(Some(digest))` if the digest was found (locally or after pull),
/// `Ok(None)` if available but digest could not be determined.
pub async fn ensure_image(docker: &Docker, image: &str) -> Result<Option<String>, PullError> {
    // Check local first.
    match image_exists_locally(docker, image).await {
        Ok(Some(digest)) => {
            tracing::debug!(
                target: "cronduit.docker.pull",
                image,
                digest = %digest,
                "image already available locally"
            );
            return Ok(Some(digest));
        }
        Ok(None) => {
            // Not local -- pull it.
        }
        Err(e) => {
            // Docker error inspecting -- treat as transient.
            return Err(PullError::Transient(e.to_string()));
        }
    }

    pull_image_with_retry(docker, image, 3).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_pull_error_terminal_unauthorized() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 401,
            message: "unauthorized: authentication required".to_string(),
        };
        let classified = classify_pull_error(&err);
        assert!(
            matches!(classified, PullError::Terminal(_)),
            "unauthorized should be Terminal, got: {classified:?}"
        );
    }

    #[test]
    fn test_classify_pull_error_terminal_manifest() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            message: "manifest unknown: manifest unknown".to_string(),
        };
        let classified = classify_pull_error(&err);
        assert!(
            matches!(classified, PullError::Terminal(_)),
            "manifest unknown should be Terminal, got: {classified:?}"
        );
    }

    #[test]
    fn test_classify_pull_error_transient_connection() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 500,
            message: "connection refused".to_string(),
        };
        let classified = classify_pull_error(&err);
        assert!(
            matches!(classified, PullError::Transient(_)),
            "connection refused should be Transient, got: {classified:?}"
        );
    }

    #[test]
    fn test_classify_pull_error_transient_timeout() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 504,
            message: "timeout waiting for response".to_string(),
        };
        let classified = classify_pull_error(&err);
        assert!(
            matches!(classified, PullError::Transient(_)),
            "timeout should be Transient, got: {classified:?}"
        );
    }
}
