use serde::Serialize;

use crate::error::MusliError;
use crate::http;
use crate::provider::ProviderConfig;

const TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatMessage {
    pub role: &'static str,
    pub content: String,
}

pub fn chat_complete(
    provider: &ProviderConfig,
    api_key: Option<&str>,
    messages: &[ChatMessage],
) -> Result<String, MusliError> {
    let body = serde_json::json!({ "model": provider.model, "messages": messages, "temperature": 0 });
    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let request = http::agent(TIMEOUT_SECS).post(&url).header("Content-Type", "application/json");
    let response = http::send(http::with_auth(request, api_key), body.to_string().as_bytes())?;

    let parsed: serde_json::Value =
        serde_json::from_str(&response).map_err(|e| MusliError::InvalidResponse { message: e.to_string() })?;
    parsed["choices"][0]["message"]["content"]
        .as_str()
        .map(|t| t.trim().to_owned())
        .ok_or_else(|| MusliError::InvalidResponse { message: "missing choices[0].message.content".into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;
    use mockito::Matcher;

    fn provider(base_url: String) -> ProviderConfig {
        ProviderConfig { kind: ProviderKind::Groq, base_url, model: "m1".into() }
    }

    fn messages() -> Vec<ChatMessage> {
        vec![ChatMessage { role: "user", content: "hoi".into() }]
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
        assert_eq!(chat_complete(&provider(server.url()), Some("k"), &messages()).unwrap(), "Hoi.");
        mock.assert();
    }

    #[test]
    fn chat_complete_maps_500() {
        let mut server = mockito::Server::new();
        server.mock("POST", "/chat/completions").with_status(500).with_body("boom").create();
        let err = chat_complete(&provider(server.url()), None, &messages()).unwrap_err();
        assert_eq!(err, MusliError::Http { status: 500, message: "boom".into() });
    }
}
