//! Conduit Faces for Tongues streaming recognition.
//!
//! Tongues owns the speech event lifecycle, recognition commitment, segmentation,
//! and barge-in semantics. Conduit owns only the portable Face and the boundary
//! where one immutable Tongues recognition commit becomes a Body user message.

use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};
use serde::{Deserialize, Serialize};
use std::{string::String, vec, vec::Vec};

use crate::{SpeechRecognitionContract, MAXIMUM_RECOGNIZED_TEXT_BYTES};

pub use speaking::{SegmentId, StreamEvent, TextRole};
pub use speaking::StreamEvent as RecognitionEvent;

pub const STREAMING_SPEECH_RECOGNIZE_KIND: &str = "speech/recognize-stream";
pub const STREAMING_SPEECH_RECOGNIZE_REVISION: &str = "conduit.speech/recognize-stream@1";
pub const COMMIT_RECOGNIZED_TURN_KIND: &str = "speech/commit-recognized-turn";
pub const COMMIT_RECOGNIZED_TURN_REVISION: &str = "conduit.speech/commit-recognized-turn@1";
pub const COMMITTED_TURN_TO_TEXT_KIND: &str = "speech/committed-turn-to-text";
pub const COMMITTED_TURN_TO_TEXT_REVISION: &str = "conduit.speech/committed-turn-to-text@1";
pub const RECOGNITION_EVENT_VALUE_KIND: &str = "speech/recognition-event@1";
pub const CHAT_MESSAGE_VALUE_KIND: &str = "ChatMessage";

/// The Conduit transport envelope for one Tongues StreamEvent.
///
/// Tongues owns event semantics. Conduit owns this finite carrier bound.
pub const MAXIMUM_RECOGNITION_EVENT_BYTES: usize = 64 * 1024;
pub const MAXIMUM_COMMITTED_USER_MESSAGE_BYTES: usize = 4_096;
pub const MAXIMUM_STREAMING_AUDIO_BYTES: usize = 262_144;
pub const MAXIMUM_STREAMING_AUDIO_ITEMS: u16 = 32;
pub const MAXIMUM_RECOGNITION_EVENT_ITEMS: u16 = 32;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommittedUserMessage {
    pub turn_identity: String,
    pub role: String,
    pub source: String,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamingRecognitionRefusal {
    BoundExceeded,
    InvalidEvent,
}

pub fn streaming_speech_recognition_contract() -> SpeechRecognitionContract {
    SpeechRecognitionContract {
        kind_id: kind_id(STREAMING_SPEECH_RECOGNIZE_KIND),
        kind_contract_revision: KindContractRevision::from(STREAMING_SPEECH_RECOGNIZE_REVISION),
        inputs: vec![flow_port(
            "audio",
            conduit_audio::AUDIO_PCM_INFO_ID,
            PortDirection::Input,
        )],
        outputs: vec![flow_port(
            "events",
            RECOGNITION_EVENT_VALUE_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_STREAMING_AUDIO_ITEMS,
            max_queue_bytes: MAXIMUM_STREAMING_AUDIO_BYTES as u32,
        },
    }
}

pub fn committed_recognition_turn_contract() -> SpeechRecognitionContract {
    SpeechRecognitionContract {
        kind_id: kind_id(COMMIT_RECOGNIZED_TURN_KIND),
        kind_contract_revision: KindContractRevision::from(COMMIT_RECOGNIZED_TURN_REVISION),
        inputs: vec![flow_port(
            "events",
            RECOGNITION_EVENT_VALUE_KIND,
            PortDirection::Input,
        )],
        outputs: vec![flow_port(
            "message",
            CHAT_MESSAGE_VALUE_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_RECOGNITION_EVENT_ITEMS,
            max_queue_bytes: (MAXIMUM_RECOGNITION_EVENT_ITEMS as u32)
                * (MAXIMUM_RECOGNITION_EVENT_BYTES as u32),
        },
    }
}

pub fn committed_turn_to_text_contract() -> SpeechRecognitionContract {
    SpeechRecognitionContract {
        kind_id: kind_id(COMMITTED_TURN_TO_TEXT_KIND),
        kind_contract_revision: KindContractRevision::from(COMMITTED_TURN_TO_TEXT_REVISION),
        inputs: vec![flow_port(
            "message",
            CHAT_MESSAGE_VALUE_KIND,
            PortDirection::Input,
        )],
        outputs: vec![flow_port(
            "text",
            conduit_text::TEXT_VALUE_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_RECOGNITION_EVENT_ITEMS,
            max_queue_bytes: MAXIMUM_COMMITTED_USER_MESSAGE_BYTES as u32,
        },
    }
}

pub fn encode_recognition_event(
    event: &speaking::StreamEvent,
) -> Result<Vec<u8>, StreamingRecognitionRefusal> {
    let encoded =
        serde_json::to_vec(event).map_err(|_| StreamingRecognitionRefusal::InvalidEvent)?;
    if encoded.len() > MAXIMUM_RECOGNITION_EVENT_BYTES {
        return Err(StreamingRecognitionRefusal::BoundExceeded);
    }
    Ok(encoded)
}

pub fn decode_recognition_event(
    encoded: &[u8],
) -> Result<speaking::StreamEvent, StreamingRecognitionRefusal> {
    if encoded.len() > MAXIMUM_RECOGNITION_EVENT_BYTES {
        return Err(StreamingRecognitionRefusal::BoundExceeded);
    }
    let event: speaking::StreamEvent =
        serde_json::from_slice(encoded).map_err(|_| StreamingRecognitionRefusal::InvalidEvent)?;
    if encode_recognition_event(&event)? != encoded {
        return Err(StreamingRecognitionRefusal::InvalidEvent);
    }
    Ok(event)
}

/// Crosses the Conduit Body boundary only for an immutable recognition commit
/// selected by Tongues. Partial/revised recognition and generated speech remain
/// speech-runtime events and never become user messages here.
pub fn committed_user_message(
    event: &speaking::StreamEvent,
) -> Result<Option<CommittedUserMessage>, StreamingRecognitionRefusal> {
    let Some((segment_id, text)) = speaking::committed_recognition_segment(event) else {
        return Ok(None);
    };
    if text.is_empty() || text.len() > MAXIMUM_RECOGNIZED_TEXT_BYTES {
        return Err(StreamingRecognitionRefusal::InvalidEvent);
    }
    Ok(Some(CommittedUserMessage {
        turn_identity: format!("tongues/{}/turn", segment_id.0),
        role: "user".into(),
        source: "committed-external-speech".into(),
        text: text.into(),
    }))
}

pub fn project_committed_turn_text(
    message: &CommittedUserMessage,
) -> Result<&str, StreamingRecognitionRefusal> {
    if message.role != "user"
        || message.source != "committed-external-speech"
        || message.turn_identity.is_empty()
        || message.turn_identity.len() > 256
        || message.text.is_empty()
        || message.text.len() > MAXIMUM_RECOGNIZED_TEXT_BYTES
    {
        return Err(StreamingRecognitionRefusal::InvalidEvent);
    }
    Ok(&message.text)
}

pub fn encode_committed_user_message(
    message: &CommittedUserMessage,
) -> Result<Vec<u8>, StreamingRecognitionRefusal> {
    project_committed_turn_text(message)?;
    let encoded =
        serde_json::to_vec(message).map_err(|_| StreamingRecognitionRefusal::InvalidEvent)?;
    if encoded.len() > MAXIMUM_COMMITTED_USER_MESSAGE_BYTES {
        return Err(StreamingRecognitionRefusal::BoundExceeded);
    }
    Ok(encoded)
}

pub fn project_encoded_committed_turn_text(
    encoded: &[u8],
) -> Result<Vec<u8>, StreamingRecognitionRefusal> {
    if encoded.len() > MAXIMUM_COMMITTED_USER_MESSAGE_BYTES {
        return Err(StreamingRecognitionRefusal::BoundExceeded);
    }
    let message: CommittedUserMessage =
        serde_json::from_slice(encoded).map_err(|_| StreamingRecognitionRefusal::InvalidEvent)?;
    project_committed_turn_text(&message)?;
    Ok(message.text.into_bytes())
}

fn flow_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speaking::{SegmentId, StreamEvent, TextRole};

    fn committed(role: TextRole, text: &str) -> StreamEvent {
        StreamEvent::CommittedSegment {
            role,
            segment_id: SegmentId("segment-1".into()),
            text: text.into(),
            words: Vec::new(),
            language: None,
            speaker_id: None,
            confidence: None,
        }
    }

    #[test]
    fn only_tongues_committed_recognition_crosses_the_body_turn_boundary() {
        let partial = StreamEvent::PartialHypothesis {
            role: TextRole::Recognition,
            segment_id: SegmentId("segment-1".into()),
            text: "hel".into(),
            confidence: None,
        };
        assert!(committed_user_message(&partial).unwrap().is_none());
        assert!(
            committed_user_message(&committed(TextRole::Generation, "response"))
                .unwrap()
                .is_none()
        );

        let message = committed_user_message(&committed(TextRole::Recognition, "hello"))
            .unwrap()
            .unwrap();
        assert_eq!(message.text, "hello");
        assert_eq!(project_committed_turn_text(&message), Ok("hello"));
    }

    #[test]
    fn recognition_wire_is_the_tongues_stream_event_ir() {
        let event = committed(TextRole::Recognition, "Hello, Body.");
        let encoded = encode_recognition_event(&event).unwrap();
        assert_eq!(decode_recognition_event(&encoded).unwrap(), event);
    }

    #[test]
    fn portable_stream_ports_are_bounded_closing_flows() {
        let recognize = streaming_speech_recognition_contract();
        let commit = committed_recognition_turn_contract();
        assert!(recognize
            .inputs
            .iter()
            .chain(recognize.outputs.iter())
            .all(|port| { port.temporal == PortTemporal::Flow { closes: true } }));
        assert_eq!(
            recognize.limits.max_queue_items,
            MAXIMUM_STREAMING_AUDIO_ITEMS
        );
        assert_eq!(
            commit.outputs[0].value_kind.as_str(),
            CHAT_MESSAGE_VALUE_KIND
        );
    }
}
