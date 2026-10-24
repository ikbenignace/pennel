use serde::Deserialize;

use crate::error::MusliError;
use crate::http;

const TIMEOUT_SECS: u64 = 10;
const SPEECH_HINTS: [&str; 4] = ["whisper", "transcribe", "voxtral", "speech"];
/// OpenRouter ids that pick a different model per request.
const ROUTERS: [&str; 2] = ["openrouter/auto", "openrouter/free"];

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RemoteModel {
    pub id: String,
    pub name: Option<String>,
    pub context_length: Option<u32>,
    pub prompt_price_per_million: Option<f64>,
    pub completion_price_per_million: Option<f64>,
    pub audio_input: bool,
    pub is_router: bool,
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<RawModel>,
}

#[derive(Deserialize)]
struct RawModel {
    id: String,
    name: Option<String>,
    context_length: Option<u32>,
    pricing: Option<RawPricing>,
    architecture: Option<RawArchitecture>,
}

#[derive(Deserialize)]
struct RawPricing {
    prompt: Option<String>,
    completion: Option<String>,
}

#[derive(Deserialize)]
struct RawArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
}

pub fn is_router(id: &str) -> bool {
    ROUTERS.contains(&id)
}

#[uniffi::export]
pub fn is_router_model(id: String) -> bool {
    is_router(&id)
}

fn per_million(per_token: Option<&String>) -> Option<f64> {
    per_token?.parse::<f64>().ok().filter(|p| *p >= 0.0).map(|p| (p * 1_000_000.0 * 1000.0).round() / 1000.0)
}

fn convert(raw: RawModel) -> RemoteModel {
    let lower = raw.id.to_lowercase();
    let audio_input = raw.architecture.as_ref().is_some_and(|a| a.input_modalities.iter().any(|m| m == "audio"))
        || SPEECH_HINTS.iter().any(|hint| lower.contains(hint));
    RemoteModel {
        is_router: is_router(&raw.id),
        prompt_price_per_million: per_million(raw.pricing.as_ref().and_then(|p| p.prompt.as_ref())),
        completion_price_per_million: per_million(raw.pricing.as_ref().and_then(|p| p.completion.as_ref())),
        id: raw.id,
        name: raw.name,
        context_length: raw.context_length,
        audio_input,
    }
}

/// Lists a provider's models via the OpenAI-compatible `GET /models`; OpenRouter adds prices and context.
#[uniffi::export]
pub fn provider_models(base_url: String, api_key: Option<String>) -> Result<Vec<RemoteModel>, MusliError> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut request = http::agent(TIMEOUT_SECS).get(&url);
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        request = request.header("Authorization", format!("Bearer {key}"));
    }
    let mut response = request.call().map_err(|e| MusliError::Network { message: e.to_string() })?;
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().map_err(|e| MusliError::Network { message: e.to_string() })?;
    if !(200..300).contains(&status) {
        return Err(http::map_status(status, &body));
    }
    let list: ModelList =
        serde_json::from_str(&body).map_err(|e| MusliError::InvalidResponse { message: e.to_string() })?;
    let mut models: Vec<RemoteModel> = list.data.into_iter().map(convert).collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serve(status: usize, body: &str) -> mockito::ServerGuard {
        let mut server = mockito::Server::new();
        server.mock("GET", "/models").match_header("authorization", "Bearer k").with_status(status).with_body(body).create();
        server
    }

    #[test]
    fn openrouter_models_with_prices() {
        let server = serve(
            200,
            r#"{"data":[{"id":"google/gemini-3.5-flash-lite","name":"Gemini 3.5 Flash Lite","context_length":1048576,
                "pricing":{"prompt":"0.0000001","completion":"0.0000004"},
                "architecture":{"input_modalities":["text","audio"]}}]}"#,
        );
        let models = provider_models(server.url(), Some("k".into())).unwrap();
        assert_eq!(
            models,
            vec![RemoteModel {
                id: "google/gemini-3.5-flash-lite".into(),
                name: Some("Gemini 3.5 Flash Lite".into()),
                context_length: Some(1_048_576),
                prompt_price_per_million: Some(0.1),
                completion_price_per_million: Some(0.4),
                audio_input: true,
                is_router: false,
            }]
        );
    }

    #[test]
    fn plain_openai_models_list() {
        let server = serve(200, r#"{"data":[{"id":"whisper-large-v3-turbo"},{"id":"llama-3.3-70b-versatile"}]}"#);
        let models = provider_models(server.url(), Some("k".into())).unwrap();
        assert_eq!(models[0].id, "llama-3.3-70b-versatile");
        assert!(!models[0].audio_input);
        assert!(models[1].audio_input);
        assert!(models.iter().all(|m| m.prompt_price_per_million.is_none() && m.context_length.is_none()));
    }

    #[test]
    fn flags_routers() {
        let server = serve(200, r#"{"data":[{"id":"openrouter/free"},{"id":"openrouter/auto"},{"id":"openai/gpt-4.1-mini"}]}"#);
        let models = provider_models(server.url(), Some("k".into())).unwrap();
        let routers: Vec<&str> = models.iter().filter(|m| m.is_router).map(|m| m.id.as_str()).collect();
        assert_eq!(routers, ["openrouter/auto", "openrouter/free"]);
    }

    #[test]
    fn http_error_maps() {
        let server = serve(401, r#"{"error":{"message":"bad key"}}"#);
        let err = provider_models(server.url(), Some("k".into())).unwrap_err();
        assert_eq!(err, MusliError::Http { status: 401, message: "bad key".into() });
    }
}
