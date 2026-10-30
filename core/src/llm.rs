use serde::Serialize;

use crate::error::PennelError;
use crate::http;
use crate::provider::{ProviderConfig, ProviderKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatMessage {
    pub role: &'static str,
    pub content: String,
}

/// How much a reasoning model may think before answering. Only OpenRouter normalizes this across models;
/// other providers reject unknown fields on non-reasoning models, so they get none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reasoning {
    /// Rewriting text needs no thinking; one model spent 14k thinking tokens on a 400-character cleanup.
    Off,
    Low,
}

pub(crate) fn request_body(
    provider: &ProviderConfig,
    messages: &[ChatMessage],
    reasoning: Reasoning,
) -> serde_json::Value {
    let mut body =
        serde_json::json!({ "model": provider.model, "messages": messages, "temperature": 0 });
    if provider.kind == ProviderKind::OpenRouter {
        match reasoning {
            Reasoning::Off => body["reasoning"] = serde_json::json!({ "enabled": false }),
            Reasoning::Low => body["reasoning"] = serde_json::json!({ "effort": "low" }),
        }
    }
    body
}

pub fn chat_complete_within(
    provider: &ProviderConfig,
    api_key: Option<&str>,
    messages: &[ChatMessage],
    timeout_secs: u64,
    reasoning: Reasoning,
) -> Result<String, PennelError> {
    if provider.kind == ProviderKind::AppleIntelligence {
        return Err(crate::system_llm::missing_model());
    }
    if provider.kind.is_cli_agent() {
        return crate::cli_agent::complete(
            provider.kind,
            &provider.base_url,
            &provider.model,
            messages,
            timeout_secs,
        );
    }
    let body = request_body(provider, messages, reasoning);
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );
    let request = http::agent(timeout_secs)
        .post(&url)
        .header("Content-Type", "application/json");
    let response = http::send_for(
        provider.kind,
        http::with_auth(request, api_key),
        body.to_string().as_bytes(),
    )?;

    let parsed: serde_json::Value =
        serde_json::from_str(&response).map_err(|e| PennelError::InvalidResponse {
            message: e.to_string(),
        })?;
    parsed["choices"][0]["message"]["content"]
        .as_str()
        .map(|t| t.trim().to_owned())
        .ok_or_else(|| PennelError::InvalidResponse {
            message: "missing choices[0].message.content".into(),
        })
}

pub(crate) fn chat_complete_before(
    provider: &ProviderConfig,
    api_key: Option<&str>,
    messages: &[ChatMessage],
    deadline: std::time::Instant,
    reasoning: Reasoning,
) -> Result<String, PennelError> {
    let invalid = |message: &str| PennelError::InvalidResponse {
        message: message.into(),
    };
    let network = || PennelError::Network {
        message: "The cleanup provider could not complete the request.".into(),
    };
    if provider.kind == ProviderKind::AppleIntelligence || provider.kind.is_cli_agent() {
        return Err(invalid(
            "This cleanup provider requires its native adapter.",
        ));
    }
    let mut body = request_body(provider, messages, reasoning);
    if provider.kind == ProviderKind::OpenRouter {
        body["provider"] = serde_json::json!({"allow_fallbacks":false});
    }
    let payload = body.to_string();
    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
    if remaining.is_zero() {
        return Err(network());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(remaining))
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .into();
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );
    let request = agent.post(&url).header("Content-Type", "application/json");
    let mut response = http::with_auth(request, api_key)
        .send(payload.as_bytes())
        .map_err(|_| network())?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(http::map_status(status, ""));
    }
    let response = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_string()
        .map_err(|_| invalid("The cleanup response is unreadable or exceeds its size limit."))?;
    if std::time::Instant::now() >= deadline {
        return Err(network());
    }
    let parsed: serde_json::Value = serde_json::from_str(&response)
        .map_err(|_| invalid("The cleanup provider returned invalid JSON."))?;
    parsed["choices"][0]["message"]["content"]
        .as_str()
        .map(|text| text.trim().to_owned())
        .ok_or_else(|| invalid("The cleanup provider returned no text."))
}

#[cfg(test)]
mod reasoning_tests {
    use super::*;
    use crate::provider::ProviderKind;
    use mockito::Matcher;

    fn ask(kind: ProviderKind, reasoning: Reasoning, body: Matcher) {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/chat/completions")
            .match_body(body)
            .with_body(r#"{"choices":[{"message":{"content":"ok"}}]}"#)
            .create();
        let provider = ProviderConfig {
            kind,
            base_url: server.url(),
            model: "m".into(),
        };
        let messages = [ChatMessage {
            role: "user",
            content: "x".into(),
        }];
        assert_eq!(
            chat_complete_within(&provider, None, &messages, 5, reasoning).unwrap(),
            "ok"
        );
        mock.assert();
    }

    #[test]
    fn openrouter_turns_reasoning_off() {
        ask(
            ProviderKind::OpenRouter,
            Reasoning::Off,
            Matcher::PartialJsonString(r#"{"reasoning":{"enabled":false}}"#.into()),
        );
        ask(
            ProviderKind::OpenRouter,
            Reasoning::Low,
            Matcher::PartialJsonString(r#"{"reasoning":{"effort":"low"}}"#.into()),
        );
    }

    #[test]
    fn other_providers_get_no_reasoning_field() {
        let provider = |kind| ProviderConfig {
            kind,
            base_url: String::new(),
            model: "m".into(),
        };
        assert!(
            request_body(&provider(ProviderKind::Groq), &[], Reasoning::Off)
                .get("reasoning")
                .is_none()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;
    use mockito::Matcher;

    #[test]
    fn apple_intelligence_cannot_use_an_http_endpoint_without_the_host_adapter() {
        let mut server = mockito::Server::new();
        let no_http = server.mock("POST", "/chat/completions").expect(0).create();
        let mut provider = provider(server.url());
        provider.kind = ProviderKind::AppleIntelligence;
        let error =
            chat_complete_within(&provider, None, &messages(), 1, Reasoning::Off).unwrap_err();
        assert!(error.to_string().contains("Apple Intelligence"));
        no_http.assert();
    }

    fn provider(base_url: String) -> ProviderConfig {
        ProviderConfig {
            kind: ProviderKind::Groq,
            base_url,
            model: "m1".into(),
        }
    }

    fn messages() -> Vec<ChatMessage> {
        vec![ChatMessage {
            role: "user",
            content: "hoi".into(),
        }]
    }

    #[test]
    fn chat_complete_parses_first_choice() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/chat/completions")
            .match_header("authorization", "Bearer k")
            .match_body(Matcher::AllOf(vec![
                Matcher::PartialJsonString(r#"{"model":"m1","temperature":0}"#.into()),
                Matcher::Regex(r#""role":"user""#.into()),
            ]))
            .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"  Hoi. "}}]}"#)
            .create();
        assert_eq!(
            chat_complete_within(
                &provider(server.url()),
                Some("k"),
                &messages(),
                5,
                Reasoning::Low
            )
            .unwrap(),
            "Hoi."
        );
        mock.assert();
    }

    #[test]
    fn chat_complete_maps_500() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/chat/completions")
            .with_status(500)
            .with_body("Failed synthetic-api-key-not-real for synthetic private dictated content")
            .create();
        let err = chat_complete_within(
            &provider(server.url()),
            None,
            &messages(),
            5,
            Reasoning::Low,
        )
        .unwrap_err();
        assert_eq!(
            err,
            PennelError::Http {
                status: 500,
                message: "provider server error".into()
            }
        );
    }

    #[test]
    fn cleanup_generation_refuses_redirects_and_does_not_retry() {
        let mut server = mockito::Server::new();
        let target = server
            .mock("POST", "/must-not-run")
            .expect(0)
            .with_body(r#"{"choices":[{"message":{"content":"Synthetic text."}}]}"#)
            .create();
        let first = server
            .mock("POST", "/chat/completions")
            .expect(1)
            .with_status(307)
            .with_header("location", &format!("{}/must-not-run", server.url()))
            .create();
        let result = chat_complete_before(
            &provider(server.url()),
            Some("synthetic-key"),
            &messages(),
            std::time::Instant::now() + std::time::Duration::from_secs(2),
            Reasoning::Off,
        );
        assert!(matches!(result, Err(PennelError::Http { status: 307, .. })));
        first.assert();
        target.assert();
    }

    #[test]
    fn expired_cleanup_deadline_never_dispatches() {
        let mut server = mockito::Server::new();
        let request = server
            .mock("POST", "/chat/completions")
            .expect(0)
            .with_body(r#"{"choices":[{"message":{"content":"Synthetic text."}}]}"#)
            .create();
        assert!(
            chat_complete_before(
                &provider(server.url()),
                None,
                &messages(),
                std::time::Instant::now() - std::time::Duration::from_millis(1),
                Reasoning::Off
            )
            .is_err()
        );
        request.assert();
    }

    #[test]
    fn cleanup_generation_caps_fractional_deadline_and_sanitizes_errors() {
        let mut server = mockito::Server::new();
        let slow = server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_chunked_body(|writer| {
                std::thread::sleep(std::time::Duration::from_millis(600));
                writer.write_all(
                    br#"{"choices":[{"message":{"content":"Synthetic private sentence"}}]}"#,
                )
            })
            .expect(1)
            .create();
        let started = std::time::Instant::now();
        let error = chat_complete_before(
            &provider(server.url()),
            Some("synthetic-key"),
            &messages(),
            started + std::time::Duration::from_millis(150),
            Reasoning::Off,
        )
        .unwrap_err();
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        assert!(!error.to_string().contains("Synthetic private"));
        assert!(!error.to_string().contains("synthetic-key"));
        slow.assert();
    }

    #[test]
    fn cleanup_generation_disables_router_fallback_and_preserves_bounded_output() {
        let mut server = mockito::Server::new();
        let call = server
            .mock("POST", "/chat/completions")
            .expect(1)
            .match_body(Matcher::PartialJson(
                serde_json::json!({"provider":{"allow_fallbacks":false}}),
            ))
            .with_body(r#"{"choices":[{"message":{"content":"Synthetic text."}}]}"#)
            .create();
        let mut configured = provider(server.url());
        configured.kind = ProviderKind::OpenRouter;
        assert_eq!(
            chat_complete_before(
                &configured,
                None,
                &messages(),
                std::time::Instant::now() + std::time::Duration::from_secs(2),
                Reasoning::Off
            )
            .unwrap(),
            "Synthetic text."
        );
        call.assert();
    }
}
