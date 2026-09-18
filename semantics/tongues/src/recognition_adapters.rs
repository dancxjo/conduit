//! Explicit adapters between bounded single-shot speech and streaming speech.
//!
//! These Gears make temporal conversion visible in the expanded Form and Plan.
//! Provider mechanics such as Whisper process invocation and PCM resampling remain
//! realization truth below these portable Faces.

use std::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{
    check_syntax_document, parse_syntax_document, CanonicalBackCatalog, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use sha2::{Digest, Sha256};

use crate::{
    decode_speech_recognition_result, encode_recognition_event, RecognitionEvent,
    RecognitionEventStatus, SpeechOrigin, SpeechRecognitionDisposition,
    MAXIMUM_RECOGNITION_EVENT_BYTES, MAXIMUM_RECOGNITION_RESULT_BYTES,
    MAXIMUM_STREAMING_AUDIO_BYTES, STREAMING_SPEECH_RECOGNIZE_KIND,
};

pub const SPEECH_WINDOW_TO_CLIP_KIND: &str = "speech/window-to-clip";
pub const SPEECH_WINDOW_TO_CLIP_REVISION: &str = "conduit.speech/window-to-clip@1";
pub const SPEECH_RESULT_TO_EVENT_STREAM_KIND: &str = "speech/result-to-event-stream";
pub const SPEECH_RESULT_TO_EVENT_STREAM_REVISION: &str =
    "conduit.speech/result-to-event-stream@1";
const STREAMING_RECOGNITION_BACK: &str = r#"form speech/recognize-stream (
    > audio: audio/pcm-frames@1...|
    events: speech/recognition-event@1...| >
) {
    window: speech/window-to-clip
    recognize: speech/recognize-clip
    stream: speech/result-to-event-stream

    audio > window.frames
    window.clip > recognize.clip
    recognize.result > stream.result
    stream.events > events
}
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecognitionAdapterRefusal {
    InvalidResult,
    Encoding,
}

pub fn speech_window_to_clip_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(SPEECH_WINDOW_TO_CLIP_KIND),
        kind_contract_revision: KindContractRevision::from(SPEECH_WINDOW_TO_CLIP_REVISION),
        inputs: vec![port(
            "frames",
            conduit_audio::AUDIO_PCM_INFO_ID,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "clip",
            conduit_audio::AUDIO_PCM_CLIP_INFO_ID,
            PortDirection::Output,
            PortTemporal::Value,
        )],
        configuration: Vec::new(),
    }
}

pub fn speech_result_to_event_stream_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(SPEECH_RESULT_TO_EVENT_STREAM_KIND),
        kind_contract_revision: KindContractRevision::from(SPEECH_RESULT_TO_EVENT_STREAM_REVISION),
        inputs: vec![port(
            "result",
            crate::SPEECH_RECOGNITION_RESULT_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![port(
            "events",
            crate::RECOGNITION_EVENT_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        configuration: Vec::new(),
    }
}

pub fn speech_window_to_clip_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: crate::MAXIMUM_STREAMING_AUDIO_ITEMS,
        max_queue_bytes: MAXIMUM_STREAMING_AUDIO_BYTES as u32,
    }
}

pub fn speech_result_to_event_stream_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: MAXIMUM_RECOGNITION_RESULT_BYTES
            .max(MAXIMUM_RECOGNITION_EVENT_BYTES) as u32,
    }
}

pub fn install_speech_recognition_adapters(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for definition in [
        speech_window_to_clip_definition(),
        speech_result_to_event_stream_definition(),
    ] {
        startup.insert(KindSignature {
            kind: definition.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Reviewed realization Back for a single-shot recognizer serving the portable
/// streaming recognition Face. The Back is semantic and provider-neutral.
pub fn install_single_shot_streaming_recognition_back(
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
    backs: &mut CanonicalBackCatalog,
) -> Result<(), String> {
    let checked = check_syntax_document(
        &parse_syntax_document(STREAMING_RECOGNITION_BACK),
        startup,
    )
    .map_err(|error| format!("check streaming-recognition Back: {error:?}"))?;
    let definition = profile
        .get(&kind_id(STREAMING_SPEECH_RECOGNIZE_KIND))
        .ok_or_else(|| "missing streaming speech recognition definition".to_string())?;
    backs
        .insert(definition, &checked, STREAMING_SPEECH_RECOGNIZE_KIND)
        .map_err(|error| format!("install streaming-recognition Back: {error:?}"))
}

/// Convert one exact single-shot result into one terminal event and a closing
/// Flow. The stream identity is derived only from retained semantic provenance.
pub fn recognition_result_to_terminal_event(
    encoded_result: &[u8],
) -> Result<Vec<u8>, RecognitionAdapterRefusal> {
    let result = decode_speech_recognition_result(encoded_result)
        .map_err(|_| RecognitionAdapterRefusal::InvalidResult)?;
    let mut identity = Sha256::new();
    identity.update(b"conduit-single-shot-recognition-stream-v1\0");
    identity.update(result.audio_sha256);
    identity.update(result.provider_identity.as_bytes());
    let event = RecognitionEvent {
        stream_id: format!("recognition/single-shot/{:x}", identity.finalize()),
        sequence: 0,
        status: match result.disposition {
            SpeechRecognitionDisposition::Recognized => RecognitionEventStatus::Committed,
            SpeechRecognitionDisposition::NoSpeech => RecognitionEventStatus::NoSpeech,
        },
        origin: SpeechOrigin::External,
        text: result.text,
        audio_extent_bytes: result.audio_extent_bytes,
        elapsed_milliseconds: 0,
        provider_identity: result.provider_identity,
    };
    encode_recognition_event(&event).map_err(|_| RecognitionAdapterRefusal::Encoding)
}

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode_speech_recognition_result, SpeechRecognitionResult};

    #[test]
    fn explicit_adapters_preserve_temporal_direction() {
        let window = speech_window_to_clip_definition();
        assert_eq!(
            window.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(window.outputs[0].temporal, PortTemporal::Value);

        let stream = speech_result_to_event_stream_definition();
        assert_eq!(stream.inputs[0].temporal, PortTemporal::Value);
        assert_eq!(
            stream.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
    }

    #[test]
    fn single_shot_result_becomes_one_committed_event_with_exact_provenance() {
        let encoded = encode_speech_recognition_result(&SpeechRecognitionResult {
            disposition: SpeechRecognitionDisposition::Recognized,
            text: Some("Hello Margret".into()),
            audio_sha256: [7; 32],
            audio_extent_bytes: 320,
            provider_identity: "fixture/provider@1".into(),
        })
        .unwrap();
        let event = crate::decode_recognition_event(
            &recognition_result_to_terminal_event(&encoded).unwrap(),
        )
        .unwrap();
        assert_eq!(event.status, RecognitionEventStatus::Committed);
        assert_eq!(event.text.as_deref(), Some("Hello Margret"));
        assert_eq!(event.provider_identity, "fixture/provider@1");
        assert_eq!(event.audio_extent_bytes, 320);
    }

    #[test]
    fn streaming_face_expands_to_visible_single_shot_adapters() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        crate::install_speech_recognition_catalog(&mut startup, &mut profile).unwrap();
        let checked = check_syntax_document(
            &parse_syntax_document(
                "form main ( > audio: audio/pcm-frames@1...| events: speech/recognition-event@1...| > ) { recognize: speech/recognize-stream audio > recognize.audio recognize.events > events }",
            ),
            &startup,
        )
        .unwrap();
        let mut backs = CanonicalBackCatalog::new();
        install_single_shot_streaming_recognition_back(&startup, &profile, &mut backs).unwrap();
        let expanded = conduit_form::expand_canonical_form_for_authoring_with_backs(
            &checked,
            "main",
            &profile,
            &backs,
        )
        .unwrap()
        .expanded;
        let kinds = expanded
            .gears
            .iter()
            .map(|gear| gear.kind_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                SPEECH_WINDOW_TO_CLIP_KIND,
                crate::SPEECH_RECOGNIZE_CLIP_KIND,
                SPEECH_RESULT_TO_EVENT_STREAM_KIND,
            ]
        );
    }
}
