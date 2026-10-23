use crate::error::MusliError;
use crate::http;
use crate::provider::ProviderConfig;

const BASE_TIMEOUT_SECS: u64 = 30;
/// Budget for a slow uplink (~2 Mbit/s) so long hands-free recordings can still upload.
const UPLOAD_BYTES_PER_SEC: u64 = 250_000;

pub fn timeout_secs(upload_bytes: usize) -> u64 {
    BASE_TIMEOUT_SECS + upload_bytes as u64 / UPLOAD_BYTES_PER_SEC
}

pub fn transcribe(
    provider: &ProviderConfig,
    api_key: Option<&str>,
    wav: &[u8],
    prompt: Option<&str>,
    language: Option<&str>,
) -> Result<String, MusliError> {
    let boundary = format!("musli-{}", uuid::Uuid::now_v7().simple());
    let mut fields = vec![("model", provider.model.as_str()), ("response_format", "json")];
    fields.extend(prompt.map(|p| ("prompt", p)));
    fields.extend(language.map(|l| ("language", l)));
    let body = multipart(&boundary, &fields, wav);

    let url = format!("{}/audio/transcriptions", provider.base_url.trim_end_matches('/'));
    let request = http::agent(timeout_secs(body.len()))
        .post(&url)
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"));
    let response = http::send(http::with_auth(request, api_key), &body)?;

    let parsed: serde_json::Value =
        serde_json::from_str(&response).map_err(|e| MusliError::InvalidResponse { message: e.to_string() })?;
    parsed["text"]
        .as_str()
        .map(|t| t.trim().to_owned())
        .ok_or_else(|| MusliError::InvalidResponse { message: "missing text field".into() })
}

fn multipart(boundary: &str, fields: &[(&str, &str)], wav: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(wav.len() + 512);
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;
    use mockito::Matcher;

    fn provider(base_url: String) -> ProviderConfig {
        ProviderConfig {
            kind: ProviderKind::Groq,
            base_url,
            model: "whisper-large-v3-turbo".into(),
        }
    }

    #[test]
    fn timeout_grows_with_upload_size() {
        assert_eq!(timeout_secs(0), 30);
        assert!(timeout_secs(19_200_000) >= 100, "{}", timeout_secs(19_200_000));
    }

    #[test]
    fn sends_multipart_with_model_and_auth() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/audio/transcriptions")
            .match_header("authorization", "Bearer k1")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("name=\"model\"\r\n\r\nwhisper-large-v3-turbo".into()),
                Matcher::Regex("name=\"file\"; filename=\"audio.wav\"".into()),
                Matcher::Regex("name=\"response_format\"\r\n\r\njson".into()),
            ]))
            .with_body(r#"{"text":"  hallo wereld "}"#)
            .create();
        let text = transcribe(&provider(server.url()), Some("k1"), b"RIFF", None, None).unwrap();
        assert_eq!(text, "hallo wereld");
        mock.assert();
    }

    #[test]
    fn omits_auth_header_without_key() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/audio/transcriptions")
            .match_header("authorization", Matcher::Missing)
            .with_body(r#"{"text":"ok"}"#)
            .create();
        transcribe(&provider(server.url()), None, b"RIFF", None, None).unwrap();
        mock.assert();
    }

    #[test]
    fn includes_prompt_and_language_when_given() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/audio/transcriptions")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("name=\"prompt\"\r\n\r\nMusli, Q8".into()),
                Matcher::Regex("name=\"language\"\r\n\r\nnl".into()),
            ]))
            .with_body(r#"{"text":"ok"}"#)
            .create();
        transcribe(&provider(server.url()), None, b"RIFF", Some("Musli, Q8"), Some("nl")).unwrap();
        mock.assert();
    }

    #[test]
    fn maps_401_with_openai_error_body() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/audio/transcriptions")
            .with_status(401)
            .with_body(r#"{"error":{"message":"Invalid API key"}}"#)
            .create();
        let err = transcribe(&provider(server.url()), Some("bad"), b"RIFF", None, None).unwrap_err();
        assert_eq!(err, MusliError::Http { status: 401, message: "Invalid API key".into() });
    }

    #[test]
    fn maps_connection_refused_to_network() {
        let err = transcribe(&provider("http://127.0.0.1:9".into()), None, b"RIFF", None, None).unwrap_err();
        assert!(matches!(err, MusliError::Network { .. }), "{err:?}");
    }

    #[test]
    fn invalid_json_is_invalid_response() {
        let mut server = mockito::Server::new();
        server.mock("POST", "/audio/transcriptions").with_body("<html>").create();
        let err = transcribe(&provider(server.url()), None, b"RIFF", None, None).unwrap_err();
        assert!(matches!(err, MusliError::InvalidResponse { .. }), "{err:?}");
    }
}
