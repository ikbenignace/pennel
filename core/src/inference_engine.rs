use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use serde::{Deserialize, Serialize};

use crate::inference;
use crate::{
    ApiKeys, DecisionQuestion, DecisionQuestionKind, DecisionRequest, DecisionResult,
    InferenceCancellation, InferenceCapability, InferenceConfig, InferenceIdentity,
    InferenceProtocol, PennelEngine, PennelError, ProviderAccount, ProviderKind, RerankPassage,
    RerankRequest, RerankResult,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct InferenceAvailability {
    pub capability: InferenceCapability,
    pub configured: bool,
    pub available: bool,
    pub reason: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub protocol: Option<InferenceProtocol>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct InferenceVerification {
    pub verified: bool,
    pub message: String,
    pub identity: Option<InferenceIdentity>,
}

#[derive(Default)]
pub(crate) struct InferenceState {
    pub(crate) requests: Vec<Weak<InferenceCancellation>>,
    pub(crate) mutation_depth: usize,
}

/// Keeps inference admission closed throughout a native configuration or credential write.
#[derive(uniffi::Object)]
pub struct InferenceMutation {
    state: Arc<Mutex<InferenceState>>,
    released: AtomicBool,
}

#[uniffi::export]
impl InferenceMutation {
    pub fn release(&self) {
        if !self.released.swap(true, Ordering::AcqRel) {
            self.state.lock().unwrap().mutation_depth -= 1;
        }
    }
}

impl Drop for InferenceMutation {
    fn drop(&mut self) {
        self.release();
    }
}

impl PennelEngine {
    pub(crate) fn inference_state(&self) -> Arc<Mutex<InferenceState>> {
        self.inference_requests.clone()
    }
}

#[uniffi::export]
impl PennelEngine {
    pub fn begin_inference_mutation(&self) -> Arc<InferenceMutation> {
        let state = self.inference_state();
        {
            let mut active = state.lock().unwrap();
            active.mutation_depth += 1;
            for request in &active.requests {
                if let Some(cancellation) = request.upgrade() {
                    cancellation.cancel();
                }
            }
        }
        Arc::new(InferenceMutation {
            state,
            released: AtomicBool::new(false),
        })
    }
}

fn invalid(message: &str) -> PennelError {
    PennelError::InvalidResponse {
        message: message.into(),
    }
}

struct InferenceRegistration<'a> {
    engine: &'a PennelEngine,
    cancellation: &'a Arc<InferenceCancellation>,
}

impl Drop for InferenceRegistration<'_> {
    fn drop(&mut self) {
        self.engine.finish_inference(self.cancellation);
    }
}

impl PennelEngine {
    fn inference_selection(
        &self,
        capability: InferenceCapability,
    ) -> Result<(InferenceConfig, ProviderAccount), PennelError> {
        let settings = self.settings();
        let config = match capability {
            InferenceCapability::Decision => settings.decision,
            InferenceCapability::Reranking => settings.reranking,
        }
        .ok_or_else(|| invalid("Configure this optional connection first."))?;
        let account = settings
            .providers
            .into_iter()
            .find(|p| p.id == config.provider_id)
            .ok_or_else(|| invalid("The selected provider is missing."))?;
        if !matches!(
            account.kind,
            ProviderKind::OpenRouter | ProviderKind::Custom
        ) {
            return Err(invalid(
                "Choose an OpenRouter or custom server account for this connection.",
            ));
        }
        if account.kind == ProviderKind::OpenRouter
            && !matches!(
                config.protocol,
                InferenceProtocol::OpenRouterDecisions | InferenceProtocol::OpenRouterSystemOne
            )
        {
            return Err(invalid("Choose an OpenRouter API format for this account."));
        }
        inference::validate_config(&config, &account.base_url, capability)?;
        Ok((config, account))
    }

    fn inference_unchanged(
        &self,
        capability: InferenceCapability,
        config: &InferenceConfig,
        account: &ProviderAccount,
        cancellation: &InferenceCancellation,
    ) -> Result<(), PennelError> {
        let unchanged = self
            .inference_selection(capability)
            .is_ok_and(|(current, provider)| current == *config && provider == *account);
        if cancellation.is_cancelled() || !unchanged {
            return Err(PennelError::Cancelled);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;
    use std::collections::HashMap;

    fn configured(
        server: &mockito::Server,
        protocol: InferenceProtocol,
    ) -> (tempfile::TempDir, Arc<PennelEngine>, ApiKeys) {
        let dir = tempfile::tempdir().unwrap();
        let engine = PennelEngine::new_preview(dir.path().to_string_lossy().into_owned()).unwrap();
        let mut settings = engine.settings();
        settings.providers.push(ProviderAccount {
            id: "typed-fixture".into(),
            kind: ProviderKind::Custom,
            name: "Synthetic server".into(),
            base_url: format!("{}/v1", server.url()),
        });
        let mut config = crate::new_inference_config("typed-fixture".into(), protocol);
        config.model = "fixture-model".into();
        settings.decision = Some(config);
        settings.dictionary = vec!["private-vocabulary-must-not-be-sent".into()];
        engine.update_settings(settings).unwrap();
        let keys = ApiKeys {
            by_provider: HashMap::from([("typed-fixture".into(), "synthetic-typed-key".into())]),
        };
        (dir, engine, keys)
    }

    fn response() -> &'static str {
        r#"{"model":"fixture-model","answers":{"sample":{"type":"noul","noul":0.99}},"usage":{"input_tokens":12,"output_tokens":0,"cost":0.0001}}"#
    }

    #[test]
    fn missing_configuration_and_cancellation_never_enable_a_provider() {
        let dir = tempfile::tempdir().unwrap();
        let engine = PennelEngine::new_preview(dir.path().to_string_lossy().into_owned()).unwrap();
        for capability in [
            InferenceCapability::Decision,
            InferenceCapability::Reranking,
        ] {
            let availability = engine.inference_availability(capability);
            assert!(!availability.configured);
            assert!(!availability.available);
            assert!(availability.endpoint.is_none());
        }
        let cancel = InferenceCancellation::new();
        cancel.cancel();
        assert_eq!(
            engine.verify_inference(
                InferenceCapability::Decision,
                cancel,
                ApiKeys {
                    by_provider: HashMap::new()
                }
            ),
            Err(PennelError::Cancelled)
        );
    }

    #[test]
    fn synthetic_verification_preserves_choices_and_sends_no_user_vocabulary() {
        let mut server = mockito::Server::new();
        let (_dir, engine, keys) = configured(&server, InferenceProtocol::Perplexity);
        let before = engine.settings();
        let mock = server.mock("POST", "/v1/decisions")
            .match_header("authorization", "Bearer synthetic-typed-key")
            .match_body(Matcher::Json(serde_json::json!({"model":"fixture-model","state":"Synthetic Pennel sample: the meeting starts at ten.","questions":{"sample":{"type":"noul","instructions":"Does the sample state that the meeting starts at ten?"}}})))
            .with_status(200).with_body(response()).create();
        let result = engine
            .verify_inference(
                InferenceCapability::Decision,
                InferenceCancellation::new(),
                keys,
            )
            .unwrap();
        assert!(result.verified);
        assert_eq!(
            result.identity.unwrap().actual_model.as_deref(),
            Some("fixture-model")
        );
        assert_eq!(engine.settings(), before);
        mock.assert();
    }

    #[test]
    fn a_changed_configuration_cannot_verify_the_new_selection() {
        let mut server = mockito::Server::new();
        let (_dir, engine, keys) = configured(&server, InferenceProtocol::Perplexity);
        let changed = engine.clone();
        let mock = server
            .mock("POST", "/v1/decisions")
            .with_status(200)
            .with_body_from_request(move |_| {
                let mut settings = changed.settings();
                settings.decision.as_mut().unwrap().model = "changed-model".into();
                changed.update_settings(settings).unwrap();
                response().as_bytes().to_vec()
            })
            .create();
        assert_eq!(
            engine.verify_inference(
                InferenceCapability::Decision,
                InferenceCancellation::new(),
                keys
            ),
            Err(PennelError::Cancelled)
        );
        mock.assert();
    }

    #[test]
    fn malformed_requests_and_provider_errors_do_not_echo_content() {
        let mut server = mockito::Server::new();
        let (_dir, engine, keys) = configured(&server, InferenceProtocol::Perplexity);
        let error = engine
            .decide_json(
                "private input not valid JSON".into(),
                InferenceCancellation::new(),
                keys.clone(),
            )
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private input"));
        let mock = server
            .mock("POST", "/v1/decisions")
            .with_status(401)
            .with_body("private transcript synthetic-typed-key")
            .create();
        let result = engine
            .verify_inference(
                InferenceCapability::Decision,
                InferenceCancellation::new(),
                keys,
            )
            .unwrap();
        assert!(!result.verified);
        assert!(!result.message.contains("private transcript"));
        assert!(!result.message.contains("synthetic-typed-key"));
        mock.assert();
    }

    #[test]
    fn settings_upgrades_and_account_removal_do_not_select_another_connection() {
        let mut server = mockito::Server::new();
        let (_dir, engine, _) = configured(&server, InferenceProtocol::Perplexity);
        let settings = engine.settings();
        let mut previous = serde_json::to_value(&settings).unwrap();
        previous.as_object_mut().unwrap().remove("decision");
        previous.as_object_mut().unwrap().remove("reranking");
        let migrated: crate::Settings = serde_json::from_value(previous).unwrap();
        assert!(migrated.decision.is_none());
        assert!(migrated.reranking.is_none());
        assert_eq!(migrated.llm, settings.llm);
        assert_eq!(migrated.dictation_stt, settings.dictation_stt);
        assert_eq!(migrated.meeting_stt, settings.meeting_stt);
        let mut removed = settings.clone();
        removed.providers.retain(|p| p.id != "typed-fixture");
        let removed = removed.normalized();
        assert!(removed.decision.is_none());
        assert!(removed.reranking.is_none());
        assert_eq!(removed.llm, settings.llm);
        assert_eq!(
            removed.providers,
            migrated
                .providers
                .into_iter()
                .filter(|p| p.id != "typed-fixture")
                .collect::<Vec<_>>()
        );
        // The account is gone, so no later operation can send a sample to another account.
        engine.update_settings(removed).unwrap();
        server
            .mock("POST", "/v1/decisions")
            .expect(0)
            .create()
            .assert();
    }

    #[test]
    fn mobile_patch_roundtrips_optional_configuration_without_secrets() {
        let server = mockito::Server::new();
        let (_dir, engine, _) = configured(&server, InferenceProtocol::Perplexity);
        let serialized = engine.mobile_settings_json().unwrap();
        assert!(serialized.contains("fixture-model"));
        assert!(!serialized.contains("synthetic-typed-key"));
        let updated = engine
            .patch_mobile_settings(r#"{"decision":null,"dictionary":["Pennel"]}"#.into())
            .unwrap();
        let updated: crate::Settings = serde_json::from_str(&updated).unwrap();
        assert!(updated.decision.is_none());
        assert_eq!(updated.dictionary, vec!["Pennel"]);
    }

    #[test]
    fn changed_settings_cancel_registered_work_before_it_can_send() {
        let mut server = mockito::Server::new();
        let (_dir, engine, keys) = configured(&server, InferenceProtocol::Perplexity);
        let cancellation = InferenceCancellation::new();
        engine.register_inference(&cancellation);
        let mut settings = engine.settings();
        settings.decision.as_mut().unwrap().model = "replacement".into();
        engine.update_settings(settings).unwrap();
        assert!(cancellation.is_cancelled());
        let mock = server.mock("POST", "/v1/decisions").expect(0).create();
        assert_eq!(
            engine.verify_inference(InferenceCapability::Decision, cancellation.clone(), keys),
            Err(PennelError::Cancelled)
        );
        engine.finish_inference(&cancellation);
        mock.assert();
    }

    #[test]
    fn completing_one_call_keeps_a_shared_cancellation_token_registered() {
        let server = mockito::Server::new();
        let (_dir, engine, _) = configured(&server, InferenceProtocol::Perplexity);
        let cancellation = InferenceCancellation::new();
        engine.register_inference(&cancellation);
        engine.register_inference(&cancellation);
        engine.finish_inference(&cancellation);
        let mut settings = engine.settings();
        settings.decision = None;
        engine.update_settings(settings).unwrap();
        assert!(cancellation.is_cancelled());
        engine.finish_inference(&cancellation);
    }

    #[test]
    fn changing_a_tei_account_during_metadata_lookup_prevents_content_post() {
        let mut server = mockito::Server::new();
        let (_dir, engine, keys) = configured(&server, InferenceProtocol::Tei);
        let mut settings = engine.settings();
        settings.reranking = settings.decision.take();
        settings
            .providers
            .iter_mut()
            .find(|p| p.id == "typed-fixture")
            .unwrap()
            .base_url = server.url();
        engine.update_settings(settings).unwrap();
        let changed = engine.clone();
        let info = server
            .mock("GET", "/info")
            .with_status(200)
            .with_body_from_request(move |_| {
                let mut settings = changed.settings();
                settings.reranking = None;
                changed.update_settings(settings).unwrap();
                br#"{"model_id":"fixture-model","model_sha":"pinned-revision"}"#.to_vec()
            })
            .create();
        let post = server.mock("POST", "/rerank").expect(0).create();
        assert_eq!(
            engine.verify_inference(
                InferenceCapability::Reranking,
                InferenceCancellation::new(),
                keys
            ),
            Err(PennelError::Cancelled)
        );
        info.assert();
        post.assert();
    }
}

#[uniffi::export]
impl PennelEngine {
    pub fn inference_availability(&self, capability: InferenceCapability) -> InferenceAvailability {
        let settings = self.settings();
        let config = match capability {
            InferenceCapability::Decision => settings.decision,
            InferenceCapability::Reranking => settings.reranking,
        };
        let mut result = InferenceAvailability {
            capability,
            configured: config.is_some(),
            available: false,
            reason: "Configure this optional connection first.".into(),
            provider_id: config.as_ref().map(|c| c.provider_id.clone()),
            model: config.as_ref().map(|c| c.model.clone()),
            protocol: config.as_ref().map(|c| c.protocol),
            endpoint: None,
        };
        match self.inference_selection(capability) {
            Ok((config, account)) => {
                result.available = true;
                result.reason =
                    "Configured. Test with the synthetic sample to verify this connection.".into();
                result.endpoint =
                    inference::validate_config(&config, &account.base_url, capability).ok();
            }
            Err(PennelError::InvalidResponse { message }) => result.reason = message,
            Err(_) => result.reason = "This optional connection is unavailable.".into(),
        }
        result
    }

    pub fn decide(
        &self,
        request: DecisionRequest,
        cancellation: Arc<InferenceCancellation>,
        keys: ApiKeys,
    ) -> Result<DecisionResult, PennelError> {
        let capability = InferenceCapability::Decision;
        if cancellation.is_cancelled() {
            return Err(PennelError::Cancelled);
        }
        self.register_inference(&cancellation);
        let _registration = InferenceRegistration {
            engine: self,
            cancellation: &cancellation,
        };
        let (config, account) = self.inference_selection(capability)?;
        let key = self.credential(&account, &keys)?;
        let result = inference::decide(
            &config,
            &account.base_url,
            key.as_deref(),
            request,
            cancellation.clone(),
        )?;
        self.inference_unchanged(capability, &config, &account, &cancellation)?;
        Ok(result)
    }

    pub fn rerank(
        &self,
        request: RerankRequest,
        cancellation: Arc<InferenceCancellation>,
        keys: ApiKeys,
    ) -> Result<RerankResult, PennelError> {
        let capability = InferenceCapability::Reranking;
        if cancellation.is_cancelled() {
            return Err(PennelError::Cancelled);
        }
        self.register_inference(&cancellation);
        let _registration = InferenceRegistration {
            engine: self,
            cancellation: &cancellation,
        };
        let (config, account) = self.inference_selection(capability)?;
        let key = self.credential(&account, &keys)?;
        let result = inference::rerank(
            &config,
            &account.base_url,
            key.as_deref(),
            request,
            cancellation.clone(),
        )?;
        self.inference_unchanged(capability, &config, &account, &cancellation)?;
        Ok(result)
    }

    pub fn decide_json(
        &self,
        request_json: String,
        cancellation: Arc<InferenceCancellation>,
        keys: ApiKeys,
    ) -> Result<String, PennelError> {
        if request_json.len() > 256 * 1024 {
            return Err(invalid("The decision request is too large."));
        }
        let request = serde_json::from_str(&request_json)
            .map_err(|_| invalid("The decision request is invalid."))?;
        serde_json::to_string(&self.decide(request, cancellation, keys)?)
            .map_err(|_| invalid("The decision response could not be encoded."))
    }

    pub fn rerank_json(
        &self,
        request_json: String,
        cancellation: Arc<InferenceCancellation>,
        keys: ApiKeys,
    ) -> Result<String, PennelError> {
        if request_json.len() > 256 * 1024 {
            return Err(invalid("The reranking request is too large."));
        }
        let request = serde_json::from_str(&request_json)
            .map_err(|_| invalid("The reranking request is invalid."))?;
        serde_json::to_string(&self.rerank(request, cancellation, keys)?)
            .map_err(|_| invalid("The reranking response could not be encoded."))
    }

    pub fn verify_inference(
        &self,
        capability: InferenceCapability,
        cancellation: Arc<InferenceCancellation>,
        keys: ApiKeys,
    ) -> Result<InferenceVerification, PennelError> {
        if cancellation.is_cancelled() {
            return Err(PennelError::Cancelled);
        }
        let outcome = match capability {
            InferenceCapability::Decision => self
                .decide(
                    DecisionRequest {
                        state: "Synthetic Pennel sample: the meeting starts at ten.".into(),
                        questions: vec![DecisionQuestion {
                            id: "sample".into(),
                            kind: DecisionQuestionKind::Binary,
                            instructions: "Does the sample state that the meeting starts at ten?"
                                .into(),
                            options: vec![],
                        }],
                    },
                    cancellation,
                    keys,
                )
                .map(|r| {
                    let valid = r.answers.len() == 1
                        && r.answers[0].id == "sample"
                        && !r.answers[0].abstained
                        && r.answers[0].value.is_some_and(|v| v > 0.5);
                    (valid, r.identity)
                }),
            InferenceCapability::Reranking => self
                .rerank(
                    RerankRequest {
                        query: "When does the meeting start?".into(),
                        passages: vec![
                            RerankPassage {
                                id: "meeting".into(),
                                text: "The meeting starts at ten.".into(),
                            },
                            RerankPassage {
                                id: "weather".into(),
                                text: "The sky is clear.".into(),
                            },
                        ],
                    },
                    cancellation,
                    keys,
                )
                .map(|r| {
                    let meeting = r.scores.iter().find(|s| s.passage_id == "meeting");
                    let other = r.scores.iter().find(|s| s.passage_id == "weather");
                    (
                        meeting.zip(other).is_some_and(|(a, b)| a.score > b.score),
                        r.identity,
                    )
                }),
        };
        match outcome {
            Ok((verified, identity)) => Ok(InferenceVerification {
                verified,
                message: if verified { "The selected connection processed the synthetic sample. This does not establish model accuracy." } else { "The selected connection did not produce the expected sample result." }.into(),
                identity: Some(identity),
            }),
            Err(PennelError::Cancelled) => Err(PennelError::Cancelled),
            Err(error) => {
                let message = match error {
                    PennelError::MissingApiKey { .. } => "Configure this provider's API key first.",
                    PennelError::Http { status: 401 | 403, .. } => "The provider denied access. Check the key and model access.",
                    PennelError::Http { status: 402, .. } => "The provider reports insufficient credit.",
                    PennelError::Http { status: 429, .. } => "The provider has reached its usage limit. Try again later.",
                    PennelError::InvalidResponse { .. } => "The connection or typed response is invalid. Check its configuration and model.",
                    _ => "The provider could not process the sample. Check the connection and try again.",
                };
                Ok(InferenceVerification { verified: false, message: message.into(), identity: None })
            }
        }
    }
}
