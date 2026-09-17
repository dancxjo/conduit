//! Bounded language-aware commit from generated text Flow to irreversible speech segments.
//!
//! The boundary policy adapts Tongues' `StableTextChunker` and live
//! `StreamingSegmenter` responsibility. It deliberately preserves provisional
//! initials, abbreviations, decimals, ellipses, and unfinished quotations.

use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{string::String, vec, vec::Vec};

use crate::SpeechRecognitionContract;

pub const SPEECH_COMMIT_KIND: &str = "speech/commit-generated-text";
pub const SPEECH_COMMIT_REVISION: &str = "conduit.speech/commit-generated-text@1";
pub const SPEAKABLE_TEXT_VALUE_KIND: &str = "speech/speakable-text@1";
pub const MAXIMUM_PENDING_SPEECH_BYTES: usize = 1_024;
pub const MAXIMUM_SPEAKABLE_SEGMENT_BYTES: usize = 512;
pub const MAXIMUM_ENCODED_SPEAKABLE_SEGMENT_BYTES: usize = 1_024;
pub const MAXIMUM_COMMITTED_SEGMENTS: usize = 8;
pub const SPEECH_COMMIT_QUEUE_BYTES: u32 = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpeechCommitReason {
    Sentence,
    ClauseBound,
    ExtentBound,
    FinalFlush,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpeakableSegment {
    pub stream_identity: String,
    pub sequence: u32,
    pub text: String,
    pub reason: SpeechCommitReason,
}

pub fn encode_speakable_segment(
    segment: &SpeakableSegment,
) -> Result<Vec<u8>, SpeechCommitRefusal> {
    if segment.stream_identity.is_empty()
        || segment.text.is_empty()
        || segment.text.len() > MAXIMUM_SPEAKABLE_SEGMENT_BYTES
    {
        return Err(SpeechCommitRefusal::SegmentBoundExceeded);
    }
    let encoded =
        serde_json::to_vec(segment).map_err(|_| SpeechCommitRefusal::SegmentBoundExceeded)?;
    if encoded.len() > MAXIMUM_ENCODED_SPEAKABLE_SEGMENT_BYTES {
        return Err(SpeechCommitRefusal::SegmentBoundExceeded);
    }
    Ok(encoded)
}

pub fn decode_speakable_segment(encoded: &[u8]) -> Result<SpeakableSegment, SpeechCommitRefusal> {
    if encoded.len() > MAXIMUM_ENCODED_SPEAKABLE_SEGMENT_BYTES {
        return Err(SpeechCommitRefusal::SegmentBoundExceeded);
    }
    let segment: SpeakableSegment =
        serde_json::from_slice(encoded).map_err(|_| SpeechCommitRefusal::SegmentBoundExceeded)?;
    encode_speakable_segment(&segment)?;
    Ok(segment)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechCommitEvidence {
    pub stream_identity: String,
    pub segment_count: u16,
    pub committed_text_bytes: u32,
    pub normalized_whitespace_bytes: u32,
    pub committed_text_sha256: [u8; 32],
    pub first_audio_latency_milliseconds: Option<u32>,
    pub maximum_inter_segment_gap_milliseconds: Option<u32>,
    pub pcm_extent_bytes: u64,
    pub cancelled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeechCommitRefusal {
    EmptyStreamIdentity,
    EmptyDelta,
    NonMonotonicRevision,
    PendingBoundExceeded,
    SegmentBoundExceeded,
    SegmentQueueFull,
    AlreadyClosed,
}

#[derive(Clone, Debug)]
pub struct StreamingSpeechCommitter {
    stream_identity: String,
    pending: String,
    sequence: u32,
    committed_segments: usize,
    committed_text_bytes: usize,
    normalized_whitespace_bytes: usize,
    committed_hasher: Sha256,
    closed: bool,
    cancelled: bool,
}

impl StreamingSpeechCommitter {
    pub fn new(stream_identity: impl Into<String>) -> Result<Self, SpeechCommitRefusal> {
        let stream_identity = stream_identity.into();
        if stream_identity.is_empty() {
            return Err(SpeechCommitRefusal::EmptyStreamIdentity);
        }
        Ok(Self {
            stream_identity,
            pending: String::with_capacity(MAXIMUM_PENDING_SPEECH_BYTES),
            sequence: 0,
            committed_segments: 0,
            committed_text_bytes: 0,
            normalized_whitespace_bytes: 0,
            committed_hasher: Sha256::new(),
            closed: false,
            cancelled: false,
        })
    }

    pub fn push(&mut self, delta: &str) -> Result<Vec<SpeakableSegment>, SpeechCommitRefusal> {
        if self.closed {
            return Err(SpeechCommitRefusal::AlreadyClosed);
        }
        if delta.is_empty() {
            return Err(SpeechCommitRefusal::EmptyDelta);
        }
        if self.pending.len().saturating_add(delta.len()) > MAXIMUM_PENDING_SPEECH_BYTES {
            return Err(SpeechCommitRefusal::PendingBoundExceeded);
        }
        self.pending.push_str(delta);
        self.release(false)
    }

    /// Replaces only text that remains behind the irreversible commit boundary.
    pub fn repair_pending(
        &mut self,
        expected: &str,
        replacement: &str,
    ) -> Result<(), SpeechCommitRefusal> {
        if self.closed {
            return Err(SpeechCommitRefusal::AlreadyClosed);
        }
        if self.pending != expected || replacement.len() > MAXIMUM_PENDING_SPEECH_BYTES {
            return Err(SpeechCommitRefusal::NonMonotonicRevision);
        }
        self.pending.clear();
        self.pending.push_str(replacement);
        Ok(())
    }

    pub fn close(&mut self) -> Result<Vec<SpeakableSegment>, SpeechCommitRefusal> {
        if self.closed {
            return Err(SpeechCommitRefusal::AlreadyClosed);
        }
        let segments = self.release(true)?;
        self.closed = true;
        Ok(segments)
    }

    pub fn cancel(&mut self) {
        self.pending.clear();
        self.closed = true;
        self.cancelled = true;
    }

    pub fn pending_text(&self) -> &str {
        &self.pending
    }

    pub fn evidence(
        &self,
        first_audio_latency_milliseconds: Option<u32>,
        maximum_inter_segment_gap_milliseconds: Option<u32>,
        pcm_extent_bytes: u64,
    ) -> SpeechCommitEvidence {
        SpeechCommitEvidence {
            stream_identity: self.stream_identity.clone(),
            segment_count: self.committed_segments as u16,
            committed_text_bytes: self.committed_text_bytes as u32,
            normalized_whitespace_bytes: self.normalized_whitespace_bytes as u32,
            committed_text_sha256: self.committed_hasher.clone().finalize().into(),
            first_audio_latency_milliseconds,
            maximum_inter_segment_gap_milliseconds,
            pcm_extent_bytes,
            cancelled: self.cancelled,
        }
    }

    fn release(&mut self, closing: bool) -> Result<Vec<SpeakableSegment>, SpeechCommitRefusal> {
        let mut released = Vec::with_capacity(MAXIMUM_COMMITTED_SEGMENTS);
        loop {
            let boundary = next_speech_commit_boundary(&self.pending, closing);
            let (end, reason) = match boundary {
                Some(value) => value,
                None => break,
            };
            if self.committed_segments == MAXIMUM_COMMITTED_SEGMENTS {
                return Err(SpeechCommitRefusal::SegmentQueueFull);
            }
            let remainder = self.pending.split_off(end);
            let raw = core::mem::replace(&mut self.pending, remainder);
            if raw.trim().is_empty() {
                self.normalized_whitespace_bytes += raw.len();
                continue;
            }
            let text = raw;
            if text.len() > MAXIMUM_SPEAKABLE_SEGMENT_BYTES {
                return Err(SpeechCommitRefusal::SegmentBoundExceeded);
            }
            self.committed_hasher.update(text.as_bytes());
            self.committed_text_bytes += text.len();
            released.push(SpeakableSegment {
                stream_identity: self.stream_identity.clone(),
                sequence: self.sequence,
                text,
                reason,
            });
            self.sequence = self.sequence.saturating_add(1);
            self.committed_segments += 1;
        }
        Ok(released)
    }
}

/// Returns the next irreversible prefix without retaining or allocating text.
/// Installed realizations use this same policy with their own admitted storage.
pub fn next_speech_commit_boundary(
    text: &str,
    closing: bool,
) -> Option<(usize, SpeechCommitReason)> {
    stable_boundary(text)
        .or_else(|| bounded_fallback(text))
        .or_else(|| {
            (closing && !text.trim().is_empty())
                .then_some((text.len(), SpeechCommitReason::FinalFlush))
        })
}

pub fn speech_commit_contract() -> SpeechRecognitionContract {
    SpeechRecognitionContract {
        kind_id: kind_id(SPEECH_COMMIT_KIND),
        kind_contract_revision: KindContractRevision::from(SPEECH_COMMIT_REVISION),
        inputs: vec![flow_port(
            "generated",
            conduit_text::TEXT_VALUE_KIND,
            PortDirection::Input,
        )],
        outputs: vec![flow_port(
            "segments",
            SPEAKABLE_TEXT_VALUE_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_COMMITTED_SEGMENTS as u16,
            max_queue_bytes: SPEECH_COMMIT_QUEUE_BYTES,
        },
    }
}

fn stable_boundary(text: &str) -> Option<(usize, SpeechCommitReason)> {
    let mut quote_open = false;
    for (byte, character) in text.char_indices() {
        if matches!(character, '"' | '“' | '”') {
            quote_open = !quote_open;
            continue;
        }
        if !matches!(character, '.' | '!' | '?') {
            continue;
        }
        if character == '.'
            && (ellipsis_period(text, byte)
                || decimal_period(text, byte)
                || provisional_period(text, byte))
        {
            continue;
        }
        let after = byte + character.len_utf8();
        let next = text[after..].chars().next();
        let closes_quote = next.is_some_and(|value| matches!(value, '"' | '”' | '\''));
        if quote_open && !closes_quote {
            continue;
        }
        let mut end = after;
        for (offset, next_character) in text[after..].char_indices() {
            if next_character.is_whitespace() || matches!(next_character, '"' | '”' | '\'') {
                end = after + offset + next_character.len_utf8();
            } else {
                break;
            }
        }
        return Some((end, SpeechCommitReason::Sentence));
    }
    None
}

fn bounded_fallback(text: &str) -> Option<(usize, SpeechCommitReason)> {
    if text.len() < MAXIMUM_SPEAKABLE_SEGMENT_BYTES {
        return None;
    }
    let prefix_end = text
        .char_indices()
        .map(|(index, character)| index + character.len_utf8())
        .take_while(|end| *end <= MAXIMUM_SPEAKABLE_SEGMENT_BYTES)
        .last()?;
    let prefix = &text[..prefix_end];
    for (index, character) in prefix.char_indices().rev() {
        if matches!(character, ',' | ';' | ':') {
            return Some((
                index + character.len_utf8(),
                SpeechCommitReason::ClauseBound,
            ));
        }
    }
    prefix
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map(|(index, character)| {
            (
                index + character.len_utf8(),
                SpeechCommitReason::ExtentBound,
            )
        })
}

fn ellipsis_period(text: &str, index: usize) -> bool {
    let bytes = text.as_bytes();
    bytes.get(index + 1) == Some(&b'.') || index > 0 && bytes.get(index - 1) == Some(&b'.')
}

fn decimal_period(text: &str, index: usize) -> bool {
    matches!(
        (text[..index].chars().next_back(), text[index + 1..].chars().next()),
        (Some(left), Some(right)) if left.is_alphanumeric() && right.is_alphanumeric()
    )
}

fn provisional_period(text: &str, index: usize) -> bool {
    let token = text[..index]
        .split(|character: char| !character.is_alphanumeric())
        .next_back()
        .unwrap_or_default();
    token.len() == 1
        && token
            .chars()
            .all(|character| character.is_ascii_uppercase())
        || [
            "dr", "mr", "mrs", "ms", "prof", "sr", "jr", "st", "vs", "e.g", "i.e",
        ]
        .iter()
        .any(|candidate| token.eq_ignore_ascii_case(candidate))
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

    #[test]
    fn commits_two_sentences_before_generation_closes_in_order() {
        let mut commit = StreamingSpeechCommitter::new("answer/1").unwrap();
        let first = commit.push("First answer. ").unwrap();
        assert_eq!(
            first
                .iter()
                .map(|value| value.text.as_str())
                .collect::<Vec<_>>(),
            ["First answer. "]
        );
        let second = commit
            .push("Second answer! More is still generating")
            .unwrap();
        assert_eq!(second[0].text, "Second answer! ");
        assert!(!commit.pending_text().is_empty());
        let final_segment = commit.close().unwrap();
        assert_eq!(final_segment[0].text, "More is still generating");
    }

    #[test]
    fn initials_quotes_ellipses_and_decimals_wait_for_safe_context() {
        let mut commit = StreamingSpeechCommitter::new("answer/2").unwrap();
        assert!(commit.push("Who shot John F.").unwrap().is_empty());
        assert_eq!(
            commit.push(" Kennedy? ").unwrap()[0].text,
            "Who shot John F. Kennedy? "
        );
        assert!(commit.push("She said, \"Use 3.14... ").unwrap().is_empty());
        assert_eq!(
            commit.push("now!\" ").unwrap()[0].text,
            "She said, \"Use 3.14... now!\" "
        );
    }

    #[test]
    fn pending_repair_never_mutates_committed_speech() {
        let mut commit = StreamingSpeechCommitter::new("answer/3").unwrap();
        let spoken = commit.push("Stable sentence. provisional").unwrap();
        commit
            .repair_pending("provisional", "repaired ending")
            .unwrap();
        assert_eq!(spoken[0].text, "Stable sentence. ");
        assert_eq!(commit.close().unwrap()[0].text, "repaired ending");
    }

    #[test]
    fn cancellation_drops_only_pending_text_and_evidence_retains_no_content() {
        let mut commit = StreamingSpeechCommitter::new("answer/4").unwrap();
        let spoken = commit.push("Spoken already. private remainder").unwrap();
        assert_eq!(spoken.len(), 1);
        commit.cancel();
        assert!(commit.pending_text().is_empty());
        let evidence = commit.evidence(Some(120), Some(45), 8_192);
        assert_eq!(evidence.segment_count, 1);
        assert_eq!(
            evidence.committed_text_bytes,
            "Spoken already. ".len() as u32
        );
        assert!(evidence.cancelled);
        assert!(!format!("{evidence:?}").contains("Spoken already"));
    }

    #[test]
    fn extent_fallback_stays_on_utf8_boundary_without_exceeding_segment_bound() {
        let text = format!("{} é", "é".repeat(255));
        let (end, reason) = next_speech_commit_boundary(&text, false).unwrap();
        assert_eq!(reason, SpeechCommitReason::ExtentBound);
        assert!(end <= MAXIMUM_SPEAKABLE_SEGMENT_BYTES);
        assert!(text.is_char_boundary(end));
    }
}
