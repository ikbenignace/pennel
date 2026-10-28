//! Speaker recognition: who spoke in a meeting, including the user's own trained voice and the
//! voices they chose to remember. The exported surface is the same with and without the `speakers`
//! feature; without it, local recognition answers with a Dutch error and Pennel Cloud still works.

use std::path::Path;

use pennel_speakers::{Diarization, Identity};

use crate::diarize::{Track, apply, carry_names, from_sortformer, mostly_silent};
use crate::engine::PennelEngine;
use crate::error::PennelError;
#[cfg(test)]
use crate::meetings::SpeakerName;
use crate::meetings::VOICES;
use crate::models::{DIARIZER_ID, EMBEDDING_ID, SEGMENTATION_ID};
use crate::settings::SpeakerRecognitionMode;
use crate::voices::{KnownVoice, MeetingVoiceprint, VoiceProfile, VoiceStore};

pub(crate) const UNAVAILABLE: &str = "Sprekers herkennen op dit toestel zit niet in deze versie van Pennel. Kies ‘Via Pennel Cloud’ in Instellingen → Meetings.";

/// About 25 seconds of calm reading, with the sounds of everyday Dutch.
pub const ENROLLMENT_TEXT: &str = "Ik lees deze tekst voor zodat Pennel mijn stem leert kennen. Zo weet Pennel in elke meeting wanneer ik aan het woord ben, ook als we samen rond één microfoon zitten. Vandaag is een rustige dag: de koffie is nog warm, de zon schijnt door het raam en op mijn bureau ligt een schrift vol ideeën. Straks overlopen we de planning, verdelen we de taken en spreken we af wie welke vragen beantwoordt.";

const TOO_LITTLE_SPEECH: &str = "Pennel hoorde te weinig spraak. Lees de tekst rustig en volledig voor, liefst in een stille ruimte.";
const NO_VOICEPRINT: &str =
    "Pennel heeft van deze stem geen stemafdruk. Herken de sprekers van deze meeting opnieuw.";

pub(crate) fn unavailable() -> PennelError {
    PennelError::InvalidResponse {
        message: UNAVAILABLE.into(),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record, serde::Serialize)]
pub struct SpeakerRecognition {
    /// Distinct other voices in the transcript after recognition.
    pub voices: u32,
    /// Voices that carry a name: a remembered voice or one the user named before.
    pub named: u32,
    /// The user's trained voice was heard on a shared microphone.
    pub me_found: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize)]
pub struct MeetingVoice {
    pub voice: u32,
    pub name: Option<String>,
    /// Linked to a voice the user chose to remember.
    pub remembered: bool,
    /// This device holds the voiceprint, so "Onthoud deze stem" can work.
    pub can_remember: bool,
}

/// One recognition or "Onthoud deze stem" at a time: each reads, relabels and writes the transcript,
/// its names and the meeting's voiceprints, which must not interleave.
pub(crate) static RECOGNITION: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(feature = "speakers")]
pub(crate) fn diarize_local(
    segmentation: &Path,
    embedding: &Path,
    samples: &[f32],
) -> Result<Diarization, PennelError> {
    if !segmentation.is_file() || !embedding.is_file() {
        return Err(PennelError::ModelNotDownloaded);
    }
    let models = pennel_speakers::Models {
        segmentation,
        embedding,
    };
    pennel_speakers::diarize(&models, samples, &pennel_speakers::Options::default()).map_err(
        |error| PennelError::InvalidResponse {
            message: error.to_string(),
        },
    )
}

#[cfg(not(feature = "speakers"))]
pub(crate) fn diarize_local(
    _segmentation: &Path,
    _embedding: &Path,
    _samples: &[f32],
) -> Result<Diarization, PennelError> {
    Err(unavailable())
}

/// The speaker that spoke most, if it spoke long enough for a reliable voiceprint.
fn enrollment_voiceprint(diarization: &Diarization) -> Result<(Vec<f32>, u64), PennelError> {
    let main = diarization
        .speakers
        .iter()
        .max_by_key(|s| s.speech_ms)
        .filter(|s| s.speech_ms >= pennel_speakers::MIN_ENROLLMENT_MS && !s.embedding.is_empty())
        .ok_or_else(|| PennelError::InvalidResponse {
            message: TOO_LITTLE_SPEECH.into(),
        })?;
    Ok((main.embedding.clone(), main.speech_ms))
}

impl PennelEngine {
    /// Whether [`PennelEngine::recognize_speakers`] would use the microphone track, so a caller can
    /// skip decoding it (an hour of 16 kHz audio is 230 MB as samples).
    pub fn wants_microphone_track(&self, system: Option<&[f32]>) -> bool {
        system.is_none_or(mostly_silent)
            && self.voiceprints_ready()
            && self.voice_store().my_voiceprint().is_some()
    }

    /// Whether the chosen route yields voiceprints (Sortformer does not).
    fn voiceprints_ready(&self) -> bool {
        match self.settings().speaker_recognition {
            SpeakerRecognitionMode::Cloud => self.cloud_signed_in(),
            SpeakerRecognitionMode::Local => {
                cfg!(feature = "speakers")
                    && self.models.path(SEGMENTATION_ID).is_some()
                    && self.models.path(EMBEDDING_ID).is_some()
            }
        }
    }

    pub(crate) fn voice_store(&self) -> VoiceStore {
        VoiceStore::new(&self.data_dir())
    }

    /// Meeting voiceprints follow the meeting's sync identity, which survives a rename.
    fn meeting_key(&self, id: &str) -> String {
        self.meeting_store()
            .uid_of(id)
            .unwrap_or_else(|| id.to_owned())
    }

    /// Diarization by the configured route. `voiceprints` refuses the Sortformer fallback, which
    /// cannot tell whose voice it heard.
    fn speaker_diarization(
        &self,
        samples: &[f32],
        voiceprints: bool,
    ) -> Result<Diarization, PennelError> {
        if self.settings().speaker_recognition == SpeakerRecognitionMode::Cloud {
            return crate::cloud::diarize::diarize(&*self.cloud_session()?, samples);
        }
        let models = (
            self.models.path(SEGMENTATION_ID),
            self.models.path(EMBEDDING_ID),
        );
        if let (Some(segmentation), Some(embedding)) = models {
            return diarize_local(&segmentation, &embedding, samples);
        }
        if !cfg!(feature = "speakers") {
            return Err(unavailable());
        }
        match self.models.path(DIARIZER_ID) {
            Some(sortformer) if !voiceprints => Ok(from_sortformer(&crate::diarize::diarize(
                &sortformer,
                samples,
            )?)),
            _ => Err(PennelError::ModelNotDownloaded),
        }
    }
}

#[uniffi::export]
impl PennelEngine {
    /// The two downloads of local speaker recognition: segmentation, then voiceprints.
    pub fn speaker_model_ids(&self) -> Vec<String> {
        vec![SEGMENTATION_ID.to_owned(), EMBEDDING_ID.to_owned()]
    }

    /// Whether this build can recognise speakers on the device.
    pub fn local_speaker_recognition_available(&self) -> bool {
        cfg!(feature = "speakers")
    }

    /// Whether recognition can run now with the chosen route.
    pub fn speaker_recognition_ready(&self) -> bool {
        match self.settings().speaker_recognition {
            SpeakerRecognitionMode::Cloud => self.cloud_signed_in(),
            SpeakerRecognitionMode::Local => {
                let pipeline = self.models.path(SEGMENTATION_ID).is_some()
                    && self.models.path(EMBEDDING_ID).is_some();
                cfg!(feature = "speakers") && (pipeline || self.models.path(DIARIZER_ID).is_some())
            }
        }
    }

    pub fn voice_enrollment_text(&self) -> String {
        ENROLLMENT_TEXT.to_owned()
    }

    /// Learns the user's voice from the enrollment text read aloud (16 kHz mono). Only the
    /// voiceprint is kept, never the audio.
    pub fn enroll_voice(&self, samples: Vec<f32>) -> Result<VoiceProfile, PennelError> {
        if (samples.len() as u64) * 1000 / u64::from(pennel_speakers::SAMPLE_RATE)
            < pennel_speakers::MIN_ENROLLMENT_MS
        {
            return Err(PennelError::InvalidResponse {
                message: TOO_LITTLE_SPEECH.into(),
            });
        }
        let diarization = self.speaker_diarization(&samples, true)?;
        let (embedding, speech_ms) = enrollment_voiceprint(&diarization)?;
        self.voice_store().set_my_voiceprint(embedding, speech_ms)
    }

    pub fn voice_profile(&self) -> Option<VoiceProfile> {
        self.voice_store().profile()
    }

    pub fn forget_voice_profile(&self) -> Result<(), PennelError> {
        let _edit = self.edit_voice_settings()?;
        self.voice_store().forget_my_voiceprint()
    }

    pub fn known_voices(&self) -> Vec<KnownVoice> {
        self.voice_store().known()
    }

    pub fn rename_known_voice(&self, id: String, name: String) -> Result<(), PennelError> {
        self.voice_store().rename(&id, &name)
    }

    pub fn forget_known_voice(&self, id: String) -> Result<(), PennelError> {
        let _edit = self.edit_voice_settings()?;
        self.voice_store().forget(&id)
    }

    /// Removes the user's voice, every remembered voice and every meeting voiceprint.
    pub fn forget_all_voices(&self) -> Result<(), PennelError> {
        let _edit = self.edit_voice_settings()?;
        self.discard_speaker_recognition_data()?;
        match std::fs::remove_dir_all(self.data_dir().join("voices")) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(PennelError::storage(error))
            }
            _ => Ok(()),
        }
    }

    /// Recognises the speakers of a finished meeting from its tracks (16 kHz mono). The system
    /// track splits "Anderen". Without one (an in-person meeting on one microphone) the microphone
    /// track is split instead, once the user trained their voice, so their lines stay "Ik".
    pub fn recognize_speakers(
        &self,
        id: String,
        microphone: Option<Vec<f32>>,
        system: Option<Vec<f32>>,
    ) -> Result<SpeakerRecognition, PennelError> {
        self.claim_library()?;
        let _edit = self.edit_meeting(&id)?;
        let store = self.meeting_store();
        store.info(&id).ok_or(PennelError::NotFound)?;
        if store.unparsed_transcript_lines(&id) > 0 {
            return Err(PennelError::InvalidResponse {
                message: "transcript bevat regels die Pennel niet herkent".into(),
            });
        }
        let voices = self.voice_store();
        let my_voice = voices.my_voiceprint();
        let system = system.filter(|samples| !mostly_silent(samples));
        let (track, samples) = match (system, microphone) {
            (Some(samples), _) => (Track::System, samples),
            (None, Some(samples)) if my_voice.is_some() && !mostly_silent(&samples) => {
                (Track::Microphone, samples)
            }
            _ => return Ok(SpeakerRecognition::default()),
        };
        let revision = (
            store.transcript_fingerprint(&id)?,
            crate::speaker_jobs::speaker_names_digest(&store.speaker_names(&id))?,
        );
        let diarization = self.speaker_diarization(&samples, track == Track::Microphone)?;
        drop(samples);
        let _sync = self.sync_gate.lock().unwrap();
        if store.transcript_fingerprint(&id)? != revision.0
            || crate::speaker_jobs::speaker_names_digest(&store.speaker_names(&id))? != revision.1
        {
            return Err(PennelError::storage(
                "De meeting is ondertussen gewijzigd. De sprekernamen zijn niet vervangen.",
            ));
        }
        self.apply_recognized_speakers(&id, diarization, track, my_voice.as_deref())
    }

    /// The other voices of a meeting, with their names and whether they can be remembered.
    pub fn meeting_voices(&self, id: String) -> Vec<MeetingVoice> {
        let store = self.meeting_store();
        let names = store.speaker_names(&id);
        let prints = self.voice_store().meeting_voiceprints(
            &self.meeting_key(&id),
            Some(&store.transcript_fingerprint(&id).unwrap_or_default()),
        );
        let known: Vec<String> = self.known_voices().into_iter().map(|v| v.id).collect();
        let mut voices: Vec<u32> = store
            .read_transcript(&id)
            .iter()
            .filter_map(|s| s.voice)
            .collect();
        voices.sort_unstable();
        voices.dedup();
        voices
            .into_iter()
            .map(|voice| {
                let print = prints.iter().find(|p| p.voice == voice);
                MeetingVoice {
                    voice,
                    name: names
                        .iter()
                        .find(|n| n.voice == voice)
                        .map(|n| n.name.clone()),
                    remembered: print
                        .and_then(|p| p.known_id.as_ref())
                        .is_some_and(|id| known.contains(id)),
                    can_remember: print.is_some(),
                }
            })
            .collect()
    }

    /// Names a voice in this meeting and remembers its voiceprint under that name, so later
    /// meetings name it automatically.
    pub fn remember_speaker(
        &self,
        id: String,
        voice: u32,
        name: String,
    ) -> Result<(), PennelError> {
        self.claim_library()?;
        let _edit = self.edit_meeting(&id)?;
        if !VOICES.contains(&voice) {
            return Err(PennelError::NotFound);
        }
        let _gate = RECOGNITION.lock().unwrap_or_else(|e| e.into_inner());
        let voices = self.voice_store();
        let key = self.meeting_key(&id);
        let transcript_hash = self.meeting_store().transcript_fingerprint(&id)?;
        let mut prints = voices.meeting_voiceprints(&key, Some(&transcript_hash));
        let print = prints
            .iter_mut()
            .find(|p| p.voice == voice)
            .ok_or_else(|| PennelError::InvalidResponse {
                message: NO_VOICEPRINT.into(),
            })?;
        let clean = crate::voices::clean_name(&name).to_lowercase();
        let already = print.known_id.as_ref().is_some_and(|known| {
            voices
                .known()
                .iter()
                .any(|v| v.id == *known && v.name.to_lowercase() == clean)
        });
        // The same meeting voice learned twice would weigh one meeting double.
        if !already {
            print.known_id = Some(voices.remember(&name, &print.embedding)?);
        }
        self.meeting_store().rename_speaker(&id, voice, &name)?;
        voices.set_meeting_voiceprints(&key, prints, Some(&transcript_hash))
    }
}

impl PennelEngine {
    pub(crate) fn apply_recognized_speakers(
        &self,
        id: &str,
        diarization: Diarization,
        track: Track,
        my_voice: Option<&[f32]>,
    ) -> Result<SpeakerRecognition, PennelError> {
        self.apply_recognized_speakers_observed(id, diarization, track, my_voice, None, |_, _| {
            Ok(())
        })
    }

    pub(crate) fn apply_recognized_speakers_observed(
        &self,
        id: &str,
        diarization: Diarization,
        track: Track,
        my_voice: Option<&[f32]>,
        job_directory: Option<&Path>,
        before_commit: impl FnOnce(&[crate::Segment], &[crate::SpeakerName]) -> Result<(), PennelError>,
    ) -> Result<SpeakerRecognition, PennelError> {
        let store = self.meeting_store();
        let voices = self.voice_store();
        let known = voices.known_voiceprints();
        let prints: Vec<&[f32]> = known.iter().map(|(_, _, print)| print.as_slice()).collect();
        let me = my_voice.filter(|_| track == Track::Microphone);
        let identities = pennel_speakers::identify(&diarization.speakers, me, &prints);
        // Without the user's own voice in a shared-microphone recording there is no telling which
        // lines are theirs; relabelling would turn every "Ik" into somebody else.
        if track == Track::Microphone && !identities.contains(&Identity::Me) {
            return Ok(SpeakerRecognition::default());
        }

        let _gate = RECOGNITION.lock().unwrap_or_else(|e| e.into_inner());
        if store.unparsed_transcript_lines(id) > 0 {
            return Err(PennelError::InvalidResponse {
                message: "transcript bevat regels die Pennel niet herkent".into(),
            });
        }
        store.ensure_uid(id)?;
        let before = store.read_transcript(id);
        store.preserve_transcript_tracks(id)?;
        let mut after = before.clone();
        let applied = apply(&mut after, &diarization.turns, &identities, track);
        let known_of = |speaker: usize| match identities.get(speaker) {
            Some(Identity::Known(index, _)) => known.get(*index),
            _ => None,
        };
        let recognised: Vec<(u32, String)> = applied
            .voices
            .iter()
            .filter_map(|(voice, speaker)| Some((*voice, known_of(*speaker)?.1.clone())))
            .collect();
        let names = carry_names(&before, &after, &store.speaker_names(id), &recognised);
        let prints = applied
            .voices
            .iter()
            .filter_map(|(voice, speaker)| {
                let embedding = diarization.speakers.get(*speaker)?.embedding.clone();
                (!embedding.is_empty()).then(|| MeetingVoiceprint {
                    voice: *voice,
                    embedding,
                    known_id: known_of(*speaker).map(|(id, _, _)| id.clone()),
                })
            })
            .collect();
        before_commit(&after, &names)?;
        self.commit_speaker_result(id, &after, &names, prints, job_directory)?;
        Ok(SpeakerRecognition {
            voices: applied.voices.len() as u32,
            named: names.len() as u32,
            me_found: applied.me_found,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::{Segment, Speaker};
    use pennel_speakers::{Speaker as Voice, Turn};

    fn name(voice: u32, name: &str) -> SpeakerName {
        SpeakerName {
            voice,
            name: name.into(),
        }
    }

    #[test]
    fn enrollment_needs_enough_speech_and_takes_the_main_voice() {
        let speaker = |speaker, speech_ms, embedding: Vec<f32>| Voice {
            speaker,
            speech_ms,
            embedding,
        };
        let short = Diarization {
            turns: vec![],
            speakers: vec![speaker(0, 8_000, vec![1.0])],
        };
        assert!(enrollment_voiceprint(&short).is_err());
        let fine = Diarization {
            turns: vec![],
            speakers: vec![
                speaker(0, 3_000, vec![0.0, 1.0]),
                speaker(1, 21_000, vec![1.0, 0.0]),
            ],
        };
        assert_eq!(
            enrollment_voiceprint(&fine).unwrap(),
            (vec![1.0, 0.0], 21_000)
        );
        let no_print = Diarization {
            turns: vec![],
            speakers: vec![speaker(0, 21_000, vec![])],
        };
        assert!(enrollment_voiceprint(&no_print).is_err());
    }

    #[cfg(feature = "speakers")]
    #[test]
    fn local_diarization_without_models_needs_a_download() {
        let missing = Path::new("/nonexistent/pennel/model.onnx");
        assert_eq!(
            diarize_local(missing, missing, &[0.0; 32_000]),
            Err(PennelError::ModelNotDownloaded)
        );
    }

    #[cfg(not(feature = "speakers"))]
    #[test]
    fn local_diarization_without_feature_is_the_dutch_error() {
        let missing = Path::new("/nonexistent/pennel/model.onnx");
        assert_eq!(
            diarize_local(missing, missing, &[0.0; 32_000]),
            Err(unavailable())
        );
    }

    fn engine_with_cloud(
        dir: &tempfile::TempDir,
        server: &mut mockito::ServerGuard,
    ) -> std::sync::Arc<PennelEngine> {
        use crate::cloud::SecretStore;
        let engine =
            PennelEngine::new(dir.path().join("data").to_string_lossy().into_owned()).unwrap();
        let mut settings = engine.settings();
        settings.meetings_dir = dir.path().join("meetings").to_string_lossy().into_owned();
        settings.speaker_recognition = SpeakerRecognitionMode::Cloud;
        engine.update_settings(settings).unwrap();
        let secrets = std::sync::Arc::new(crate::cloud::tests::MemorySecrets::default());
        secrets.set(crate::cloud::REFRESH_ACCOUNT.into(), Some("ref".into()));
        server
            .mock("POST", "/v1/auth/refresh")
            .with_body(r#"{"access_token":"acc","refresh_token":"ref2","expires_in":900}"#)
            .create();
        engine.use_cloud_session(crate::cloud::account::Session::new(server.url(), secrets));
        engine
    }

    fn cloud_result(turns: &[Turn], speakers: &[(u32, Vec<f32>)]) -> String {
        let speakers: Vec<Voice> = speakers
            .iter()
            .map(|(speaker, embedding)| Voice {
                speaker: *speaker,
                speech_ms: 20_000,
                embedding: embedding.clone(),
            })
            .collect();
        serde_json::json!({ "model": EMBEDDING_ID, "turns": turns, "speakers": speakers })
            .to_string()
    }

    fn turn(start_ms: u64, end_ms: u64, speaker: u32) -> Turn {
        Turn {
            start_ms,
            end_ms,
            speaker,
        }
    }

    fn line(start_ms: u64, speaker: Speaker, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms: start_ms,
            speaker,
            text: text.into(),
            failed: false,
            voice: None,
        }
    }

    fn speech(seconds: usize) -> Vec<f32> {
        (0..seconds * 16_000)
            .map(|i| (i as f32 * 0.05).sin() * 0.2)
            .collect()
    }

    #[test]
    fn enrolls_then_finds_me_and_a_remembered_voice_on_one_microphone() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let engine = engine_with_cloud(&dir, &mut server);
        let me = vec![1.0, 0.0, 0.0];
        let jan = vec![0.0, 1.0, 0.0];
        let room = vec![0.0, 0.0, 1.0];

        let enrollment = server
            .mock("POST", "/v1/audio/diarize")
            .with_body(cloud_result(&[turn(0, 20_000, 0)], &[(0, me.clone())]))
            .create();
        let profile = engine.enroll_voice(speech(22)).unwrap();
        assert_eq!(profile.speech_seconds, 20);
        enrollment.remove();
        engine.voice_store().remember("Jan", &jan).unwrap();

        let id = engine
            .meeting_store()
            .create("Overleg", chrono::Local::now())
            .unwrap();
        engine
            .meeting_store()
            .write_transcript(
                &id,
                &[
                    line(0, Speaker::Me, "goedemorgen allemaal"),
                    line(5_000, Speaker::Me, "dag iedereen"),
                    line(10_000, Speaker::Me, "zullen we beginnen"),
                ],
            )
            .unwrap();
        server
            .mock("POST", "/v1/audio/diarize")
            .with_body(cloud_result(
                &[
                    turn(0, 4_500, 0),
                    turn(5_000, 9_500, 1),
                    turn(10_000, 14_000, 2),
                ],
                &[(0, me.clone()), (1, jan.clone()), (2, room.clone())],
            ))
            .create();
        let result = engine
            .recognize_speakers(id.clone(), Some(speech(15)), Some(vec![0.0; 16_000 * 15]))
            .unwrap();
        assert_eq!(
            result,
            SpeakerRecognition {
                voices: 2,
                named: 1,
                me_found: true
            }
        );
        let labels: Vec<(Speaker, Option<u32>)> = engine
            .meeting_store()
            .read_transcript(&id)
            .iter()
            .map(|s| (s.speaker, s.voice))
            .collect();
        assert_eq!(
            labels,
            [
                (Speaker::Me, None),
                (Speaker::Others, Some(1)),
                (Speaker::Others, Some(2))
            ]
        );
        assert_eq!(engine.meeting_speakers(id.clone()), [name(1, "Jan")]);
        assert_eq!(
            engine.meeting_store().transcript_tracks(&id).unwrap(),
            Some(vec![Speaker::Me])
        );

        let voices = engine.meeting_voices(id.clone());
        assert!(voices[0].remembered && voices[0].can_remember);
        assert!(!voices[1].remembered && voices[1].can_remember);
        engine
            .remember_speaker(id.clone(), 2, "Anna".into())
            .unwrap();
        engine
            .remember_speaker(id.clone(), 2, "anna".into())
            .unwrap();
        engine
            .remember_speaker(id.clone(), 1, "Jan".into())
            .unwrap();
        assert!(engine.known_voices().iter().all(|v| v.meetings == 1));
        assert!(engine.meeting_voices(id.clone())[1].remembered);
        let known: Vec<String> = engine.known_voices().into_iter().map(|v| v.name).collect();
        assert_eq!(known, ["Anna", "Jan"]);
        assert_eq!(
            engine.meeting_speakers(id),
            [name(1, "Jan"), name(2, "anna")]
        );
    }

    #[test]
    fn microphone_is_left_alone_without_a_trained_voice_or_with_a_call() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let engine = engine_with_cloud(&dir, &mut server);
        let id = engine
            .meeting_store()
            .create("Call", chrono::Local::now())
            .unwrap();
        engine
            .meeting_store()
            .write_transcript(&id, &[line(0, Speaker::Me, "hallo")])
            .unwrap();
        let untouched = server.mock("POST", "/v1/audio/diarize").expect(0).create();
        assert!(!engine.wants_microphone_track(None));
        engine
            .voice_store()
            .set_my_voiceprint(vec![1.0], 12_000)
            .unwrap();
        assert!(engine.wants_microphone_track(None));
        assert!(engine.wants_microphone_track(Some(&[0.0; 16_000])));
        assert!(!engine.wants_microphone_track(Some(&speech(2))));
        engine.forget_voice_profile().unwrap();
        assert_eq!(
            engine
                .recognize_speakers(id.clone(), Some(speech(5)), None)
                .unwrap(),
            SpeakerRecognition::default()
        );
        untouched.assert();
    }

    #[test]
    fn a_shared_microphone_without_my_voice_keeps_every_line() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let engine = engine_with_cloud(&dir, &mut server);
        engine
            .voice_store()
            .set_my_voiceprint(vec![1.0, 0.0], 20_000)
            .unwrap();
        let id = engine
            .meeting_store()
            .create("Zaal", chrono::Local::now())
            .unwrap();
        let lines = [
            line(0, Speaker::Me, "eerste"),
            line(5_000, Speaker::Me, "tweede"),
        ];
        engine
            .meeting_store()
            .write_transcript(&id, &lines)
            .unwrap();
        server
            .mock("POST", "/v1/audio/diarize")
            .with_body(cloud_result(
                &[turn(0, 4_500, 0), turn(5_000, 9_500, 1)],
                &[(0, vec![0.0, 1.0]), (1, vec![-1.0, 0.0])],
            ))
            .create();
        let result = engine
            .recognize_speakers(id.clone(), Some(speech(10)), None)
            .unwrap();
        assert_eq!(result, SpeakerRecognition::default());
        assert_eq!(engine.meeting_store().read_transcript(&id), lines);
    }

    #[test]
    fn remembering_needs_a_voiceprint() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let engine = engine_with_cloud(&dir, &mut server);
        let id = engine
            .meeting_store()
            .create("Oud", chrono::Local::now())
            .unwrap();
        assert_eq!(
            engine.remember_speaker(id, 1, "Jan".into()),
            Err(PennelError::InvalidResponse {
                message: NO_VOICEPRINT.into()
            })
        );
    }

    #[test]
    fn forgetting_everything_removes_the_voice_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let engine = engine_with_cloud(&dir, &mut server);
        engine
            .voice_store()
            .set_my_voiceprint(vec![1.0], 12_000)
            .unwrap();
        engine.voice_store().remember("Jan", &[1.0]).unwrap();
        engine.forget_all_voices().unwrap();
        assert!(engine.voice_profile().is_none());
        assert!(engine.known_voices().is_empty());
        engine.forget_all_voices().unwrap();
    }

    #[test]
    fn local_mode_without_models_asks_for_the_download() {
        let dir = tempfile::tempdir().unwrap();
        let engine = PennelEngine::new(dir.path().to_string_lossy().into_owned()).unwrap();
        assert!(!engine.speaker_recognition_ready());
        let expected = if cfg!(feature = "speakers") {
            PennelError::ModelNotDownloaded
        } else {
            unavailable()
        };
        assert_eq!(engine.enroll_voice(speech(12)).unwrap_err(), expected);
        assert_eq!(engine.speaker_model_ids(), [SEGMENTATION_ID, EMBEDDING_ID]);
    }
}
