use std::time::Duration;

use crate::error::PennelError;

pub fn agent(timeout_secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(timeout_secs)))
        .http_status_as_error(false)
        .build()
        .into()
}

pub fn map_status(status: u16, _body: &str) -> PennelError {
    // Provider bodies can echo credentials or transcripts; these errors reach history and diagnostics.
    let message = match status {
        300..=399 => "provider redirected the request",
        400 => "provider rejected the request",
        401 => "provider authentication failed",
        402 => "provider payment required",
        403 => "provider denied access",
        404 => "provider endpoint or model not found",
        405 => "provider request method not allowed",
        408 => "provider request timed out",
        409 => "provider request conflicted",
        413 => "provider request too large",
        415 => "provider request format not supported",
        422 => "provider could not process the request",
        429 => "provider rate limit reached",
        500..=599 => "provider server error",
        _ => "provider request failed",
    };
    PennelError::Http {
        status,
        message: message.into(),
    }
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
    fn json_errors_cannot_echo_credentials_or_dictation_into_stored_errors() {
        let key = "synthetic-api-key-not-a-real-credential";
        let transcript = "Synthetic private dictated sentence";
        let body = serde_json::json!({
            "error": {
                "message": format!("Bearer {key}; request transcript: {transcript}"),
                "code": format!("{key} {transcript}")
            }
        })
        .to_string();
        let error = map_status(401, &body);
        assert_eq!(
            error,
            PennelError::Http {
                status: 401,
                message: "provider authentication failed".into()
            }
        );
        let stored_error = error.to_string();
        assert!(!stored_error.contains(key));
        assert!(!stored_error.contains(transcript));
        assert_eq!(stored_error, "http 401: provider authentication failed");
    }

    #[test]
    fn text_errors_cannot_echo_a_body_prefix_into_stored_errors() {
        let key = "synthetic-api-key-not-a-real-credential";
        let transcript = "Synthetic private dictated sentence";
        let body = format!("{key}: {transcript}; {}", "untrusted details ".repeat(40));
        let error = map_status(502, &body);
        let PennelError::Http {
            status,
            ref message,
        } = error
        else {
            panic!()
        };
        assert_eq!(status, 502);
        assert_eq!(message, "provider server error");
        assert!(!error.to_string().contains(key));
        assert!(!error.to_string().contains(transcript));
        assert!(!error.to_string().contains("untrusted details"));
    }

    #[test]
    fn safe_error_categories_preserve_status_independently_of_response_content() {
        for (status, expected) in [
            (400, "provider rejected the request"),
            (401, "provider authentication failed"),
            (402, "provider payment required"),
            (403, "provider denied access"),
            (404, "provider endpoint or model not found"),
            (408, "provider request timed out"),
            (413, "provider request too large"),
            (422, "provider could not process the request"),
            (429, "provider rate limit reached"),
            (503, "provider server error"),
            (599, "provider server error"),
            (600, "provider request failed"),
        ] {
            for body in ["", "not JSON", r#"{"error":{"message":"private content"}}"#] {
                assert_eq!(
                    map_status(status, body),
                    PennelError::Http {
                        status,
                        message: expected.into()
                    }
                );
            }
        }
    }
}
