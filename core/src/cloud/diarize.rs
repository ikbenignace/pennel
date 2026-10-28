//! Speaker recognition through Pennel Cloud for devices without the local models. The server runs
//! the same pipeline in memory and returns turns and voiceprints; matching them against the user's
//! voices stays on the device.

use pennel_speakers::{Diarization, MERGE_THRESHOLD, SAMPLE_RATE, merge_chunks};
use serde::Deserialize;

use super::account::Session;
use super::http::{agent, error_from, send_raw};
use crate::error::PennelError;
use crate::models::EMBEDDING_ID;

/// Ten minutes of 16-bit PCM is 19.2 MB, within the server's 25 MB body limit.
const CHUNK_SAMPLES: usize = 600 * SAMPLE_RATE as usize;
/// A tail shorter than this joins the previous chunk; alone it is too short to tell voices apart.
const MIN_TAIL_SAMPLES: usize = 30 * SAMPLE_RATE as usize;
const TIMEOUT_SECS: u64 = 300;

#[derive(Deserialize)]
struct Response {
    model: String,
    #[serde(flatten)]
    diarization: Diarization,
}

/// `(offset in samples, length)` of each upload.
pub(crate) fn chunks(total: usize) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0;
    while start < total {
        let mut length = CHUNK_SAMPLES.min(total - start);
        if total - start - length < MIN_TAIL_SAMPLES {
            length = total - start;
        }
        spans.push((start, length));
        start += length;
    }
    spans
}

fn post(session: &Session, wav: &[u8]) -> Result<Diarization, PennelError> {
    let http = agent(TIMEOUT_SECS);
    let url = format!("{}/v1/audio/diarize", session.base_url());
    let send = |token: &str| {
        send_raw(
            &http,
            "POST",
            &url,
            Some(token),
            &[],
            "audio/wav",
            Some(wav),
        )
    };
    let mut response = send(&session.access_token()?)?;
    if response.0 == 401 {
        response = send(&session.refreshed_token()?)?;
    }
    let (status, _, body) = response;
    parse_response(status, &body)
}

pub(crate) fn parse_response(status: u16, body: &[u8]) -> Result<Diarization, PennelError> {
    if body.len() > 8 * 1024 * 1024 {
        return Err(PennelError::InvalidResponse {
            message: "Pennel Cloud returned a speaker result that is too large.".into(),
        });
    }
    if !(200..300).contains(&status) {
        let json = serde_json::from_slice(body).unwrap_or(serde_json::Value::Null);
        return Err(error_from(status, &json));
    }
    let parsed: Response =
        serde_json::from_slice(body).map_err(|_| PennelError::InvalidResponse {
            message: "Pennel Cloud returned an unreadable speaker result.".into(),
        })?;
    if !consistent(&parsed.diarization) {
        return Err(PennelError::InvalidResponse {
            message: "Pennel Cloud returned an unreadable speaker result.".into(),
        });
    }
    if parsed.model != EMBEDDING_ID {
        return Err(PennelError::Cloud {
            code: "speaker_model".into(),
            message: "Pennel Cloud uses a different voice model from this version of Pennel. Update Pennel.".into(),
        });
    }
    Ok(parsed.diarization)
}

/// Every turn names a listed speaker, and no speaker is listed twice.
fn consistent(diarization: &Diarization) -> bool {
    let ids: Vec<u32> = diarization.speakers.iter().map(|s| s.speaker).collect();
    let unique = ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id));
    unique
        && diarization.speakers.iter().all(|speaker| {
            speaker.embedding.len() <= 4096
                && speaker.embedding.iter().all(|value| value.is_finite())
        })
        && diarization
            .turns
            .iter()
            .all(|t| t.end_ms > t.start_ms && ids.contains(&t.speaker))
}

/// Diarizes 16 kHz mono audio in chunks and joins the voices across them.
pub fn diarize(session: &Session, samples: &[f32]) -> Result<Diarization, PennelError> {
    let mut results = Vec::new();
    for (start, length) in chunks(samples.len()) {
        let wav = crate::wav::encode_wav(&samples[start..start + length], SAMPLE_RATE);
        let offset_ms = start as u64 * 1000 / u64::from(SAMPLE_RATE);
        results.push((offset_ms, post(session, &wav)?));
    }
    Ok(merge_chunks(results, MERGE_THRESHOLD))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::cloud::REFRESH_ACCOUNT;
    use crate::cloud::SecretStore;
    use crate::cloud::tests::MemorySecrets;

    fn signed_in(server: &mut mockito::ServerGuard) -> Session {
        let secrets = Arc::new(MemorySecrets::default());
        secrets.set(REFRESH_ACCOUNT.into(), Some("ref".into()));
        server
            .mock("POST", "/v1/auth/refresh")
            .with_body(r#"{"access_token":"acc","refresh_token":"ref2","expires_in":900}"#)
            .create();
        Session::new(server.url(), secrets)
    }

    #[test]
    fn chunks_cover_everything_without_a_tiny_tail() {
        let minute = 60 * SAMPLE_RATE as usize;
        assert_eq!(chunks(0), vec![]);
        assert_eq!(chunks(5 * minute), vec![(0, 5 * minute)]);
        assert_eq!(
            chunks(25 * minute),
            vec![
                (0, 10 * minute),
                (10 * minute, 10 * minute),
                (20 * minute, 5 * minute)
            ]
        );
        assert_eq!(chunks(10 * minute + 100), vec![(0, 10 * minute + 100)]);
    }

    #[test]
    fn uploads_wav_with_bearer_and_reads_voiceprints() {
        let mut server = mockito::Server::new();
        let session = signed_in(&mut server);
        let mock = server
            .mock("POST", "/v1/audio/diarize")
            .match_header("authorization", "Bearer acc")
            .match_header("content-type", "audio/wav")
            .with_body(format!(
                r#"{{"model":"{EMBEDDING_ID}","turns":[{{"start_ms":0,"end_ms":900,"speaker":0}}],"speakers":[{{"speaker":0,"speech_ms":900,"embedding":[0.6,0.8]}}]}}"#
            ))
            .create();
        let result = diarize(&session, &[0.0; 16_000]).unwrap();
        mock.assert();
        assert_eq!(result.turns.len(), 1);
        assert_eq!(result.speakers[0].embedding, vec![0.6, 0.8]);
    }

    #[test]
    fn server_errors_and_model_mismatch_preserve_protocol_and_use_english_context() {
        let mut server = mockito::Server::new();
        let session = signed_in(&mut server);
        let quota = server
            .mock("POST", "/v1/audio/diarize")
            .with_status(429)
            .with_body(r#"{"error":"quota","message":"Je limiet is bereikt."}"#)
            .create();
        assert_eq!(
            diarize(&session, &[0.0; 16_000]).unwrap_err().to_string(),
            "Je limiet is bereikt."
        );
        quota.remove();
        let broken = server
            .mock("POST", "/v1/audio/diarize")
            .with_body(format!(
                r#"{{"model":"{EMBEDDING_ID}","turns":[{{"start_ms":0,"end_ms":900,"speaker":3}}],"speakers":[]}}"#
            ))
            .create();
        assert!(matches!(
            diarize(&session, &[0.0; 16_000]),
            Err(PennelError::InvalidResponse { .. })
        ));
        broken.remove();
        server
            .mock("POST", "/v1/audio/diarize")
            .with_body(r#"{"model":"other","turns":[],"speakers":[]}"#)
            .create();
        assert!(matches!(
            diarize(&session, &[0.0; 16_000]),
            Err(PennelError::Cloud { code, .. }) if code == "speaker_model"
        ));
    }

    #[test]
    fn signed_out_is_not_signed_in() {
        let server = mockito::Server::new();
        let session = Session::new(server.url(), Arc::new(MemorySecrets::default()));
        assert_eq!(
            diarize(&session, &[0.0; 16_000]),
            Err(PennelError::NotSignedIn)
        );
    }
}
