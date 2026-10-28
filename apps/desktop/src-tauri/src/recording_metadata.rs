use pennel_core::RecordingEnvironment;

pub fn environment(app_version: &str) -> RecordingEnvironment {
    #[cfg(target_os = "macos")]
    let command = ("sw_vers", vec!["-productVersion"]);
    #[cfg(windows)]
    let command = ("cmd", vec!["/C", "ver"]);
    #[cfg(not(any(target_os = "macos", windows)))]
    let command = ("uname", vec!["-r"]);
    let os_version = std::process::Command::new(command.0)
        .args(command.1)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "unknown".into());
    RecordingEnvironment {
        app_version: app_version.into(),
        build: option_env!("PENNEL_BUILD_REVISION")
            .unwrap_or("unknown")
            .into(),
        platform: std::env::consts::OS.into(),
        os_version,
    }
}

#[cfg(any(target_os = "macos", test))]
fn capture_event(value: &serde_json::Value) -> Option<pennel_core::MeetingCaptureEvent> {
    let source = match value.get("track")? {
        serde_json::Value::Number(number) if number.as_u64() == Some(0) => "microphone",
        serde_json::Value::Number(number) if number.as_u64() == Some(1) => "system_audio",
        serde_json::Value::String(track) if track == "microphone" => "microphone",
        serde_json::Value::String(track) if track == "system" => "system_audio",
        _ => return None,
    };
    let mut event = serde_json::Map::new();
    event.insert("source".into(), source.into());
    for key in [
        "sequence",
        "elapsed_ms",
        "kind",
        "input_sample_rate",
        "input_channels",
        "input_interleaved",
        "transport",
        "captured_frames",
        "output_samples",
        "inserted_silence_samples",
        "gap_count",
        "rate_mismatch_count",
        "dropped_frames",
        "error_code",
        "dropped_events",
        "tap_sample_rate",
        "aggregate_sample_rate",
    ] {
        if let Some(value) = value.get(key) {
            event.insert(key.into(), value.clone());
        }
    }
    let kind = match event.get("kind")?.as_str()? {
        "format" => "format_changed",
        "gap" => "silence_inserted",
        "error"
            if event.get("error_code").and_then(serde_json::Value::as_str)
                == Some("capture_rate_mismatch") =>
        {
            "rate_mismatch"
        }
        "error" => "capture_error",
        "rebuild" => "restarted",
        kind => kind,
    }
    .to_owned();
    event.insert("kind".into(), kind.into());
    if !event.contains_key("error_code")
        && let Some(reason) = value.get("reason").and_then(serde_json::Value::as_str)
    {
        event.insert("error_code".into(), reason.into());
    }
    if event.get("transport").and_then(serde_json::Value::as_str) == Some("other") {
        event.insert("transport".into(), "unknown".into());
    }
    for key in [
        "input_sample_rate",
        "input_channels",
        "tap_sample_rate",
        "aggregate_sample_rate",
    ] {
        if let Some(number) = event.get(key).and_then(serde_json::Value::as_f64) {
            if !number.is_finite()
                || number < 1.0
                || number > u32::MAX as f64
                || number.fract() != 0.0
            {
                return None;
            }
            event.insert(key.into(), (number as u32).into());
        }
    }
    if let Some(code) = event.get("error_code").and_then(serde_json::Value::as_str)
        && (code.len() > 64
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
    {
        return None;
    }
    serde_json::from_value(serde_json::Value::Object(event)).ok()
}

#[cfg(target_os = "macos")]
pub fn persist_capture(
    engine: &pennel_core::PennelEngine,
    id: &str,
    diagnostics: &serde_json::Value,
    elapsed_ms: u64,
    last_sequence: &mut u64,
    lost_events: &mut u64,
) -> Result<(), String> {
    if let Some(events) = diagnostics
        .get("events")
        .and_then(serde_json::Value::as_array)
    {
        if let Some(first) = events
            .first()
            .and_then(|event| event.get("sequence"))
            .and_then(serde_json::Value::as_u64)
        {
            let missing = first.saturating_sub(last_sequence.saturating_add(1));
            *lost_events = lost_events.saturating_add(missing);
            if missing > 0 {
                *last_sequence = first - 1;
            }
        }
        for value in events {
            let sequence = value
                .get("sequence")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if sequence <= *last_sequence {
                continue;
            }
            let values = if value.get("track").and_then(serde_json::Value::as_str) == Some("all") {
                ["microphone", "system"]
                    .into_iter()
                    .map(|track| {
                        let mut event = value.clone();
                        event["track"] = track.into();
                        event
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![value.clone()]
            };
            for value in values {
                if let Some(event) = capture_event(&value) {
                    engine
                        .record_meeting_capture_event(id.to_owned(), event)
                        .map_err(|error| error.to_string())?;
                }
            }
            *last_sequence = sequence;
        }
    }
    if let Some(tracks) = diagnostics
        .get("tracks")
        .and_then(serde_json::Value::as_array)
    {
        for value in tracks {
            let mut value = value.clone();
            let Some(fields) = value.as_object_mut() else {
                continue;
            };
            fields.insert("kind".into(), "metrics".into());
            fields.insert("sequence".into(), (*last_sequence).into());
            fields.insert("elapsed_ms".into(), elapsed_ms.into());
            fields.insert("dropped_events".into(), (*lost_events).into());
            if let Some(event) = capture_event(&value) {
                engine
                    .record_meeting_capture_event(id.to_owned(), event)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pennel_core::{CaptureEventKind, RecordingSource, RecordingTransport};
    use serde_json::json;

    #[test]
    fn native_capture_events_preserve_physical_track_and_only_safe_fields() {
        let event = capture_event(&json!({
            "sequence": 3, "elapsed_ms": 1500, "kind": "format_changed", "track": 1,
            "input_sample_rate": 24000, "input_channels": 2, "input_interleaved": true,
            "transport": "bluetooth", "device_uid": "private", "text": "private"
        }))
        .unwrap();
        assert_eq!(event.source, RecordingSource::SystemAudio);
        assert_eq!(event.kind, CaptureEventKind::FormatChanged);
        assert_eq!(event.input_sample_rate, Some(24000));
        assert_eq!(event.transport, Some(RecordingTransport::Bluetooth));
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(!serialized.contains("private"));
    }

    #[test]
    fn unknown_track_or_freeform_error_does_not_enter_diagnostics() {
        for value in [
            json!({"sequence":1,"elapsed_ms":0,"kind":"started","track":9}),
            json!({"sequence":1,"elapsed_ms":0,"kind":"capture_error","track":0,"error_code":"/Users/private recording"}),
        ] {
            assert!(capture_event(&value).is_none());
        }
    }

    #[test]
    fn native_format_and_error_shapes_map_to_shared_diagnostics() {
        let format = capture_event(&json!({
            "sequence":1,"elapsed_ms":0,"kind":"format","track":"system",
            "input_sample_rate":24000.0,"tap_sample_rate":48000.0,
            "aggregate_sample_rate":24000.0,"input_channels":2,"transport":"bluetooth"
        }))
        .unwrap();
        assert_eq!(format.kind, CaptureEventKind::FormatChanged);
        assert_eq!(format.input_sample_rate, Some(24000));
        let failure = capture_event(&json!({
            "sequence":2,"elapsed_ms":1000,"kind":"error","track":"system",
            "error_code":"capture_rate_mismatch"
        }))
        .unwrap();
        assert_eq!(failure.kind, CaptureEventKind::RateMismatch);
        let other_transport = capture_event(&json!({
            "sequence":3,"elapsed_ms":2000,"kind":"metrics","track":"system",
            "transport":"other","output_samples":32000
        }))
        .unwrap();
        assert_eq!(other_transport.transport, Some(RecordingTransport::Unknown));
        assert_eq!(other_transport.output_samples, Some(32000));
        let rebuilt = capture_event(&json!({
            "sequence":4,"elapsed_ms":3500,"kind":"rebuild","track":"system",
            "reason":"configuration_changed"
        }))
        .unwrap();
        assert_eq!(rebuilt.kind, CaptureEventKind::Restarted);
        assert_eq!(rebuilt.error_code.as_deref(), Some("configuration_changed"));
    }

    #[test]
    fn recording_environment_uses_the_running_app_version() {
        assert_eq!(environment("0.1.12").app_version, "0.1.12");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn persisted_ring_evictions_are_not_reported_as_missing_diagnostics() {
        let root = tempfile::tempdir().unwrap();
        let engine =
            pennel_core::PennelEngine::new_preview(root.path().to_string_lossy().into_owned())
                .unwrap();
        let session = engine
            .clone()
            .start_meeting(
                "Example".into(),
                pennel_core::ApiKeys {
                    by_provider: Default::default(),
                },
            )
            .unwrap();
        let id = session.id();
        engine
            .prepare_meeting_recording(id.clone(), environment("0.1.12"), false)
            .unwrap();
        let mut last = 1023;
        let mut lost = 0;
        for sequence in [1024, 1030] {
            persist_capture(&engine, &id, &json!({
                "events":[{"sequence":sequence,"elapsed_ms":1000,"kind":"started","track":"system"}],
                "tracks":[{"track":"system","output_samples":16000}],
                "dropped_events":1000
            }), 1000, &mut last, &mut lost).unwrap();
            let expected = if sequence == 1024 { 0 } else { 5 };
            assert_eq!(
                engine
                    .meeting_recording_metadata(id.clone())
                    .unwrap()
                    .unwrap()
                    .diagnostic_events_dropped,
                expected
            );
        }
        session.stop().unwrap();
    }
}
