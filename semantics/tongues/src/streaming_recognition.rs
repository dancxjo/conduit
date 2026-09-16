//! Portable streaming recognition and the stable user-turn commit boundary.

use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{string::String, vec, vec::Vec};

use crate::{SpeechRecognitionContract, MAXIMUM_RECOGNIZED_TEXT_BYTES};

pub const STREAMING_SPEECH_RECOGNIZE_KIND: &str = "speech/recognize-stream";
pub const STREAMING_SPEECH_RECOGNIZE_REVISION: &str = "conduit.speech/recognize-stream@1";
pub const COMMIT_RECOGNIZED_TURN_KIND: &str = "speech/commit-recognized-turn";
pub const COMMIT_RECOGNIZED_TURN_REVISION: &str = "conduit.speech/commit-recognized-turn@1";
pub const COMMITTED_TURN_TO_TEXT_KIND: &str = "speech/committed-turn-to-text";
pub const COMMITTED_TURN_TO_TEXT_REVISION: &str = "conduit.speech/committed-turn-to-text@1";
pub const RECOGNITION_EVENT_VALUE_KIND: &str = "speech/recognition-event@1";
pub const CHAT_MESSAGE_VALUE_KIND: &str = "ChatMessage";
pub const MAXIMUM_RECOGNITION_EVENT_BYTES: usize = 1_024;
pub const MAXIMUM_COMMITTED_USER_MESSAGE_BYTES: usize = 1_024;
pub const MAXIMUM_STREAMING_AUDIO_BYTES: usize = 262_144;
pub const MAXIMUM_STREAMING_AUDIO_ITEMS: u16 = 32;
pub const MAXIMUM_RECOGNITION_EVENT_ITEMS: u16 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecognitionEventStatus {
    Provisional,
    Revised,
    Committed,
    NoSpeech,
    ProviderLost,
    Cancelled,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpeechOrigin {
    External,
    SelfSpeech,
    Ambiguous,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecognitionEvent {
    pub stream_id: String,
    pub sequence: u32,
    pub status: RecognitionEventStatus,
    pub origin: SpeechOrigin,
    pub text: Option<String>,
    pub audio_extent_bytes: u32,
    pub elapsed_milliseconds: u32,
    pub provider_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommittedUserMessage {
    pub turn_identity: String,
    pub role: String,
    pub source: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecognitionEvidence {
    pub stream_id: String,
    pub sequence: u32,
    pub status: RecognitionEventStatus,
    pub audio_extent_bytes: u32,
    pub elapsed_milliseconds: u32,
    pub provider_identity: String,
    pub turn_identity: Option<String>,
    pub text_sha256: Option<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnCommitOutcome {
    Provisional,
    Message,
    NoTurn,
    SelfSpeechRefused,
    AmbiguousWaits,
    ProviderLost,
    Cancelled,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BargeInDecision {
    KeepActiveAnswer,
    CancelActiveAnswerForCommittedExternalTurn,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamingRecognitionRefusal {
    BoundExceeded,
    InvalidEvent,
    StaleOrDuplicateSequence,
    EventAfterTerminal,
}

/// Bounded state for a clip-only provider realizing a streaming semantic face.
/// Windows are explicit realization state; they are not the authored Form value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcousticWindow {
    bytes: Vec<u8>,
    maximum_bytes: usize,
    terminal: bool,
}

impl AcousticWindow {
    pub fn new(maximum_bytes: usize) -> Result<Self, StreamingRecognitionRefusal> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_STREAMING_AUDIO_BYTES {
            return Err(StreamingRecognitionRefusal::BoundExceeded);
        }
        Ok(Self {
            bytes: Vec::with_capacity(maximum_bytes),
            maximum_bytes,
            terminal: false,
        })
    }

    pub fn push(&mut self, pcm: &[u8]) -> Result<(), StreamingRecognitionRefusal> {
        if self.terminal {
            return Err(StreamingRecognitionRefusal::EventAfterTerminal);
        }
        let length = self
            .bytes
            .len()
            .checked_add(pcm.len())
            .filter(|length| *length <= self.maximum_bytes)
            .ok_or(StreamingRecognitionRefusal::BoundExceeded)?;
        self.bytes.extend_from_slice(pcm);
        debug_assert_eq!(self.bytes.len(), length);
        Ok(())
    }

    pub fn window(&self) -> &[u8] {
        &self.bytes
    }

    /// Releases the completed provider window while retaining its admitted
    /// allocation for the next window.
    pub fn release(&mut self) {
        self.bytes.clear();
    }

    pub fn cancel(&mut self) {
        self.bytes.clear();
        self.terminal = true;
    }

    pub fn provider_lost(&mut self) {
        self.cancel();
    }

    pub fn retained_bytes(&self) -> usize {
        self.bytes.len()
    }
}

pub fn barge_in_decision(outcome: TurnCommitOutcome) -> BargeInDecision {
    if outcome == TurnCommitOutcome::Message {
        BargeInDecision::CancelActiveAnswerForCommittedExternalTurn
    } else {
        BargeInDecision::KeepActiveAnswer
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecognizedTurnCommitter {
    stream_id: String,
    next_sequence: u32,
    terminal: bool,
    committed: bool,
}

impl RecognizedTurnCommitter {
    pub fn new(stream_id: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            next_sequence: 0,
            terminal: false,
            committed: false,
        }
    }

    pub fn accept(
        &mut self,
        event: &RecognitionEvent,
    ) -> Result<
        (
            TurnCommitOutcome,
            Option<CommittedUserMessage>,
            RecognitionEvidence,
        ),
        StreamingRecognitionRefusal,
    > {
        validate_event(event)?;
        if self.terminal {
            return Err(StreamingRecognitionRefusal::EventAfterTerminal);
        }
        if event.stream_id != self.stream_id || event.sequence != self.next_sequence {
            return Err(StreamingRecognitionRefusal::StaleOrDuplicateSequence);
        }
        self.next_sequence = self.next_sequence.saturating_add(1);
        let mut message = None;
        let outcome = match event.status {
            RecognitionEventStatus::Provisional | RecognitionEventStatus::Revised => {
                TurnCommitOutcome::Provisional
            }
            RecognitionEventStatus::Committed if event.origin == SpeechOrigin::SelfSpeech => {
                TurnCommitOutcome::SelfSpeechRefused
            }
            RecognitionEventStatus::Committed if event.origin == SpeechOrigin::Ambiguous => {
                TurnCommitOutcome::AmbiguousWaits
            }
            RecognitionEventStatus::Committed if self.committed => {
                return Err(StreamingRecognitionRefusal::InvalidEvent);
            }
            RecognitionEventStatus::Committed => {
                self.committed = true;
                message = Some(CommittedUserMessage {
                    turn_identity: format!("{}/turn/{}", event.stream_id, event.sequence),
                    role: "user".into(),
                    source: "committed-external-speech".into(),
                    text: event
                        .text
                        .clone()
                        .ok_or(StreamingRecognitionRefusal::InvalidEvent)?,
                });
                TurnCommitOutcome::Message
            }
            RecognitionEventStatus::NoSpeech => TurnCommitOutcome::NoTurn,
            RecognitionEventStatus::ProviderLost => {
                self.terminal = true;
                TurnCommitOutcome::ProviderLost
            }
            RecognitionEventStatus::Cancelled => {
                self.terminal = true;
                TurnCommitOutcome::Cancelled
            }
            RecognitionEventStatus::Closed => {
                self.terminal = true;
                TurnCommitOutcome::Closed
            }
        };
        let evidence = RecognitionEvidence {
            stream_id: event.stream_id.clone(),
            sequence: event.sequence,
            status: event.status,
            audio_extent_bytes: event.audio_extent_bytes,
            elapsed_milliseconds: event.elapsed_milliseconds,
            provider_identity: event.provider_identity.clone(),
            turn_identity: message.as_ref().map(|value| value.turn_identity.clone()),
            text_sha256: event
                .text
                .as_ref()
                .map(|text| Sha256::digest(text.as_bytes()).into()),
        };
        Ok((outcome, message, evidence))
    }
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
            max_queue_bytes: MAXIMUM_RECOGNITION_EVENT_BYTES as u32,
        },
    }
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

fn validate_event(event: &RecognitionEvent) -> Result<(), StreamingRecognitionRefusal> {
    if event.stream_id.is_empty()
        || event.provider_identity.is_empty()
        || event.stream_id.len() > 128
        || event.provider_identity.len() > 128
        || event.audio_extent_bytes as usize > MAXIMUM_STREAMING_AUDIO_BYTES
    {
        return Err(StreamingRecognitionRefusal::InvalidEvent);
    }
    let has_text = event
        .text
        .as_ref()
        .is_some_and(|text| !text.is_empty() && text.len() <= MAXIMUM_RECOGNIZED_TEXT_BYTES);
    match event.status {
        RecognitionEventStatus::Provisional
        | RecognitionEventStatus::Revised
        | RecognitionEventStatus::Committed
            if has_text =>
        {
            Ok(())
        }
        RecognitionEventStatus::NoSpeech
        | RecognitionEventStatus::ProviderLost
        | RecognitionEventStatus::Cancelled
        | RecognitionEventStatus::Closed
            if event.text.is_none() =>
        {
            Ok(())
        }
        _ => Err(StreamingRecognitionRefusal::InvalidEvent),
    }
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

    fn event(
        sequence: u32,
        status: RecognitionEventStatus,
        text: Option<&str>,
    ) -> RecognitionEvent {
        RecognitionEvent {
            stream_id: "recognition/session-1".into(),
            sequence,
            status,
            origin: SpeechOrigin::External,
            text: text.map(Into::into),
            audio_extent_bytes: 320,
            elapsed_milliseconds: sequence * 20,
            provider_identity: "fixture/asr-1".into(),
        }
    }

    #[test]
    fn revisions_do_not_create_messages_before_one_commit() {
        let mut committer = RecognizedTurnCommitter::new("recognition/session-1");
        let mut messages = Vec::new();
        for value in [
            event(
                0,
                RecognitionEventStatus::Provisional,
                Some("what is the temper"),
            ),
            event(
                1,
                RecognitionEventStatus::Revised,
                Some("what is the temperature"),
            ),
            event(
                2,
                RecognitionEventStatus::Committed,
                Some("what is the temperature upstairs?"),
            ),
        ] {
            let (_, message, _) = committer.accept(&value).unwrap();
            messages.extend(message);
        }
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "what is the temperature upstairs?");
    }

    #[test]
    fn energy_and_self_speech_do_not_become_user_turns() {
        let mut committer = RecognizedTurnCommitter::new("recognition/session-1");
        let (_, message, _) = committer
            .accept(&event(
                0,
                RecognitionEventStatus::Provisional,
                Some("noise"),
            ))
            .unwrap();
        assert!(message.is_none());
        assert_eq!(
            barge_in_decision(TurnCommitOutcome::Provisional),
            BargeInDecision::KeepActiveAnswer
        );
        let mut echoed = event(1, RecognitionEventStatus::Committed, Some("our own answer"));
        echoed.origin = SpeechOrigin::SelfSpeech;
        let (outcome, message, _) = committer.accept(&echoed).unwrap();
        assert_eq!(outcome, TurnCommitOutcome::SelfSpeechRefused);
        assert!(message.is_none());
        assert_eq!(
            barge_in_decision(outcome),
            BargeInDecision::KeepActiveAnswer
        );
    }

    #[test]
    fn cancellation_and_provider_loss_release_bounded_audio() {
        let mut cancelled = AcousticWindow::new(16).unwrap();
        cancelled.push(&[1, 2, 3]).unwrap();
        cancelled.cancel();
        assert_eq!(cancelled.retained_bytes(), 0);
        let mut lost = AcousticWindow::new(16).unwrap();
        lost.push(&[1, 2, 3]).unwrap();
        lost.provider_lost();
        assert_eq!(lost.retained_bytes(), 0);
    }

    #[test]
    fn only_a_committed_external_turn_requests_barge_in() {
        let mut committer = RecognizedTurnCommitter::new("recognition/session-1");
        let (outcome, message, _) = committer
            .accept(&event(
                0,
                RecognitionEventStatus::Committed,
                Some("new question"),
            ))
            .unwrap();
        assert!(message.is_some());
        assert_eq!(
            barge_in_decision(outcome),
            BargeInDecision::CancelActiveAnswerForCommittedExternalTurn
        );
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

    #[test]
    fn only_committed_external_user_messages_project_to_conversation_text() {
        let message = CommittedUserMessage {
            turn_identity: "recognition/session-1/turn/2".into(),
            role: "user".into(),
            source: "committed-external-speech".into(),
            text: "Hello, Roseau.".into(),
        };
        assert_eq!(project_committed_turn_text(&message), Ok("Hello, Roseau."));
        let mut wrong_role = message;
        wrong_role.role = "assistant".into();
        assert_eq!(
            project_committed_turn_text(&wrong_role),
            Err(StreamingRecognitionRefusal::InvalidEvent)
        );
    }

    #[test]
    fn committed_message_wire_projection_preserves_escaped_text() {
        let message = CommittedUserMessage {
            turn_identity: "recognition/session-1/turn/3".into(),
            role: "user".into(),
            source: "committed-external-speech".into(),
            text: "Say \"hello\".\nThen listen.".into(),
        };
        let encoded = encode_committed_user_message(&message).unwrap();
        assert!(encoded.len() <= MAXIMUM_COMMITTED_USER_MESSAGE_BYTES);
        assert_eq!(
            project_encoded_committed_turn_text(&encoded).unwrap(),
            message.text.as_bytes()
        );
    }
}
