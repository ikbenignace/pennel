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
    let response = http::send(
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
}
