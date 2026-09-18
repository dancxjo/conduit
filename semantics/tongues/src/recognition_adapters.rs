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
    SpeechRecognitionDisposition, MAXIMUM_RECOGNITION_EVENT_BYTES,
    MAXIMUM_RECOGNITION_RESULT_BYTES, MAXIMUM_STREAMING_AUDIO_BYTES,
    STREAMING_SPEECH_RECOGNIZE_KIND,
};
use speaking::{SegmentId, TextRole};

pub const SPEECH_WINDOW_TO_CLIP_KIND: &str = "speech/window-to-clip";
pub const SPEECH_WINDOW_TO_CLIP_REVISION: &str = "conduit.speech/window-to-clip@1";
pub const SPEECH_RESULT_TO_EVENT_STREAM_KIND: &str = "speech/result-to-event-stream";
pub const SPEECH_RESULT_TO_EVENT_STREAM_REVISION: &str =
    "conduit.speech/result-to-event-stream@1";
const STREAMING_RECOGNITION_BACK: &str = r#"form speech/recognize-stream (
    audio: audio/pcm-frames@1...| > events: speech/recognition-event@1...|
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

/// Maximum source blocks retained by the clip-only recognition adapter.
///
/// This is finite realization state for adapting a streaming Face to a
/// single-shot provider, not part of Tongues' recognition event semantics.
pub const MAXIMUM_ACOUSTIC_WINDOW_ITEMS: usize = 8_192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcousticWindowRefusal {
    BoundExceeded,
    Closed,
}

/// Bounded PCM retention for the explicit streaming-to-single-shot adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcousticWindow {
    bytes: Vec<u8>,
    maximum_bytes: usize,
    items: usize,
    closed: bool,
}

impl AcousticWindow {
    pub fn new(maximum_bytes: usize) -> Result<Self, AcousticWindowRefusal> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_STREAMING_AUDIO_BYTES {
            return Err(AcousticWindowRefusal::BoundExceeded);
        }
        Ok(Self {
            bytes: Vec::with_capacity(maximum_bytes),
            maximum_bytes,
            items: 0,
            closed: false,
        })
    }

    pub fn push(&mut self, pcm: &[u8]) -> Result<(), AcousticWindowRefusal> {
        if self.closed {
            return Err(AcousticWindowRefusal::Closed);
        }
        if self.items >= MAXIMUM_ACOUSTIC_WINDOW_ITEMS {
            return Err(AcousticWindowRefusal::BoundExceeded);
        }
        let length = self
            .bytes
            .len()
            .checked_add(pcm.len())
            .filter(|length| *length <= self.maximum_bytes)
            .ok_or(AcousticWindowRefusal::BoundExceeded)?;
        self.bytes.extend_from_slice(pcm);
        self.items += 1;
        debug_assert_eq!(self.bytes.len(), length);
        Ok(())
    }

    pub fn window(&self) -> &[u8] {
        &self.bytes
    }

    /// Releases one completed provider window while retaining its allocation.
    pub fn release(&mut self) {
        self.bytes.clear();
        self.items = 0;
    }

    pub fn cancel(&mut self) {
        self.bytes.clear();
        self.items = 0;
        self.closed = true;
    }

    pub fn provider_lost(&mut self) {
        self.cancel();
    }

    pub fn retained_bytes(&self) -> usize {
        self.bytes.len()
    }

    pub const fn retained_items(&self) -> usize {
        self.items
    }
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

/// Convert one exact single-shot result into one Tongues event and a closing
/// Flow. Recognized text becomes one immutable recognition commit; no-speech
/// closes normally without manufacturing a user turn.
pub fn recognition_result_to_terminal_event(
    encoded_result: &[u8],
) -> Result<Vec<u8>, RecognitionAdapterRefusal> {
    let result = decode_speech_recognition_result(encoded_result)
        .map_err(|_| RecognitionAdapterRefusal::InvalidResult)?;
    let event = match result.disposition {
        SpeechRecognitionDisposition::Recognized => {
            let text = result
                .text
                .ok_or(RecognitionAdapterRefusal::InvalidResult)?;
            let mut identity = Sha256::new();
            identity.update(b"conduit-single-shot-recognition-segment-v1\0");
            identity.update(result.audio_sha256);
            identity.update(result.provider_identity.as_bytes());
            RecognitionEvent::CommittedSegment {
                role: TextRole::Recognition,
                segment_id: SegmentId(format!(
                    "recognition/single-shot/{:x}",
                    identity.finalize()
                )),
                text,
                words: Vec::new(),
                language: None,
                speaker_id: None,
                confidence: None,
            }
        }
        SpeechRecognitionDisposition::NoSpeech => RecognitionEvent::Completed,
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
    fn single_shot_result_becomes_one_tongues_recognition_commit() {
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
        match &event {
            speaking::StreamEvent::CommittedSegment {
                role,
                segment_id,
                text,
                ..
            } => {
                assert_eq!(*role, TextRole::Recognition);
                assert!(segment_id.0.starts_with("recognition/single-shot/"));
                assert_eq!(text, "Hello Margret");
            }
            other => panic!("expected committed recognition segment, got {other:?}"),
        }
        let message = crate::committed_user_message(&event)
            .unwrap()
            .expect("recognition commit crosses the Body turn boundary");
        assert_eq!(message.text, "Hello Margret");
    }

    #[test]
    fn single_shot_no_speech_completes_without_a_user_turn() {
        let encoded = encode_speech_recognition_result(&SpeechRecognitionResult {
            disposition: SpeechRecognitionDisposition::NoSpeech,
            text: None,
            audio_sha256: [0; 32],
            audio_extent_bytes: 320,
            provider_identity: "fixture/provider@1".into(),
        })
        .unwrap();
        let event = crate::decode_recognition_event(
            &recognition_result_to_terminal_event(&encoded).unwrap(),
        )
        .unwrap();
        assert_eq!(event, speaking::StreamEvent::Completed);
        assert!(crate::committed_user_message(&event).unwrap().is_none());
    }

    #[test]
    fn acoustic_window_is_bounded_reusable_and_cancellable() {
        let mut window = AcousticWindow::new(4).unwrap();
        window.push(&[1, 2]).unwrap();
        assert_eq!(window.retained_bytes(), 2);
        assert_eq!(window.retained_items(), 1);
        assert_eq!(
            window.push(&[3, 4, 5]),
            Err(AcousticWindowRefusal::BoundExceeded)
        );
        window.release();
        assert_eq!(window.retained_bytes(), 0);
        assert_eq!(window.retained_items(), 0);
        window.push(&[3, 4]).unwrap();
        window.cancel();
        assert_eq!(window.retained_bytes(), 0);
        assert_eq!(window.push(&[5]), Err(AcousticWindowRefusal::Closed));
    }

    #[test]
    fn streaming_back_preserves_the_exact_checked_face_including_shorthand() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        crate::install_speech_recognition_catalog(&mut startup, &mut profile).unwrap();
        let checked = check_syntax_document(
            &parse_syntax_document(STREAMING_RECOGNITION_BACK),
            &startup,
        )
        .unwrap();
        let form = checked
            .forms
            .iter()
            .find(|form| form.name == STREAMING_SPEECH_RECOGNIZE_KIND)
            .unwrap();
        let definition = profile
            .get(&kind_id(STREAMING_SPEECH_RECOGNIZE_KIND))
            .unwrap();
        let expected = conduit_core::CheckedFace::new(
            Vec::new(),
            definition.inputs.clone(),
            definition.outputs.clone(),
            Some((
                definition.inputs[0].port_id.clone(),
                definition.outputs[0].port_id.clone(),
            )),
        );
        assert_eq!(form.checked_face(), expected);
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
