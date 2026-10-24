use std::time::Duration;

use crate::error::PennelError;

pub fn agent(timeout_secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(timeout_secs)))
        .http_status_as_error(false)
        .build()
        .into()
}

pub fn map_status(status: u16, body: &str) -> PennelError {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| body.chars().take(200).collect());
    PennelError::Http { status, message }
}

/// Sends a prepared request and returns the body of a 2xx response, mapping everything else to `PennelError`.
pub fn send(
    request: ureq::RequestBuilder<ureq::typestate::WithBody>,
    body: &[u8],
) -> Result<String, PennelError> {
    let mut response = request.send(body).map_err(|e| PennelError::Network {
        message: e.to_string(),
    })?;
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| PennelError::Network {
            message: e.to_string(),
        })?;
    if (200..300).contains(&status) {
        Ok(text)
    } else {
        Err(map_status(status, &text))
    }
}

pub fn with_auth(
    request: ureq::RequestBuilder<ureq::typestate::WithBody>,
    api_key: Option<&str>,
) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
    match api_key {
        Some(key) => request.header("Authorization", format!("Bearer {key}")),
        None => request,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_status_uses_openai_error_message() {
        assert_eq!(
            map_status(401, r#"{"error":{"message":"Invalid API key"}}"#),
            PennelError::Http {
                status: 401,
                message: "Invalid API key".into()
            }
        );
    }

    #[test]
    fn map_status_falls_back_to_truncated_body() {
        let body = "x".repeat(500);
        let PennelError::Http { status, message } = map_status(502, &body) else {
            panic!()
        };
        assert_eq!(status, 502);
        assert_eq!(message.len(), 200);
    }
}
