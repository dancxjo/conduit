//! Conduit envelope for Tongues' irreversible generated-speech boundary.
//!
//! Tongues owns streaming text segmentation. Conduit adds only the portable
//! Front, bounded segment envelope, and content-private execution evidence.

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
pub const MAXIMUM_SPEAKABLE_SEGMENT_BYTES: usize = 1_024;
pub const MAXIMUM_ENCODED_SPEAKABLE_SEGMENT_BYTES: usize = 2_048;
pub const MAXIMUM_COMMITTED_SEGMENTS: usize = 32;
pub const SPEECH_COMMIT_QUEUE_BYTES: u32 = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpeechCommitReason {
    TonguesBoundary,
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
    if encode_speakable_segment(&segment)? != encoded {
        return Err(SpeechCommitRefusal::SegmentBoundExceeded);
    }
    Ok(segment)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechCommitEvidence {
    pub stream_identity: String,
    pub segment_count: u16,
    pub committed_text_bytes: u32,
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
    PendingBoundExceeded,
    SegmentBoundExceeded,
    SegmentQueueFull,
    AlreadyClosed,
}

#[derive(Clone, Debug)]
pub struct StreamingSpeechCommitter {
    stream_identity: String,
    segmenter: speaking::StreamingSegmenter,
    sequence: u32,
    committed_segments: usize,
    committed_text_bytes: usize,
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
            segmenter: speaking::StreamingSegmenter::default(),
            sequence: 0,
            committed_segments: 0,
            committed_text_bytes: 0,
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
        if self
            .segmenter
            .pending_text()
            .len()
            .saturating_add(delta.len())
            > MAXIMUM_PENDING_SPEECH_BYTES
        {
            return Err(SpeechCommitRefusal::PendingBoundExceeded);
        }
        let segments = self.segmenter.push(delta);
        self.wrap(segments, SpeechCommitReason::TonguesBoundary)
    }

    pub fn close(&mut self) -> Result<Vec<SpeakableSegment>, SpeechCommitRefusal> {
        if self.closed {
            return Err(SpeechCommitRefusal::AlreadyClosed);
        }
        let segments = self.segmenter.finish();
        self.closed = true;
        self.wrap(segments, SpeechCommitReason::FinalFlush)
    }

    pub fn cancel(&mut self) {
        self.segmenter.cancel();
        self.closed = true;
        self.cancelled = true;
    }

    pub fn pending_text(&self) -> &str {
        self.segmenter.pending_text()
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
            committed_text_sha256: self.committed_hasher.clone().finalize().into(),
            first_audio_latency_milliseconds,
            maximum_inter_segment_gap_milliseconds,
            pcm_extent_bytes,
            cancelled: self.cancelled,
        }
    }

    fn wrap(
        &mut self,
        segments: Vec<String>,
        reason: SpeechCommitReason,
    ) -> Result<Vec<SpeakableSegment>, SpeechCommitRefusal> {
        let mut output = Vec::with_capacity(segments.len());
        for text in segments {
            if text.is_empty() || text.len() > MAXIMUM_SPEAKABLE_SEGMENT_BYTES {
                return Err(SpeechCommitRefusal::SegmentBoundExceeded);
            }
            if self.committed_segments == MAXIMUM_COMMITTED_SEGMENTS {
                return Err(SpeechCommitRefusal::SegmentQueueFull);
            }
            self.committed_hasher.update(text.as_bytes());
            self.committed_text_bytes += text.len();
            output.push(SpeakableSegment {
                stream_identity: self.stream_identity.clone(),
                sequence: self.sequence,
                text,
                reason,
            });
            self.sequence = self.sequence.saturating_add(1);
            self.committed_segments += 1;
        }
        Ok(output)
    }
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
    fn conduit_envelopes_exact_tongues_boundaries_without_resegmenting() {
        let input =
            "First phrase, with enough material to commit safely after the clause; second ends.";
        let mut tongues = speaking::StreamingSegmenter::default();
        let mut expected = Vec::new();
        for delta in input.as_bytes().chunks(7) {
            expected.extend(tongues.push(std::str::from_utf8(delta).unwrap()));
        }
        expected.extend(tongues.finish());

        let mut conduit = StreamingSpeechCommitter::new("answer/test").unwrap();
        let mut actual = Vec::new();
        for delta in input.as_bytes().chunks(7) {
            actual.extend(
                conduit
                    .push(std::str::from_utf8(delta).unwrap())
                    .unwrap()
                    .into_iter()
                    .map(|segment| segment.text),
            );
        }
        actual.extend(
            conduit
                .close()
                .unwrap()
                .into_iter()
                .map(|segment| segment.text),
        );
        assert_eq!(actual, expected);
        assert_eq!(actual.concat(), input);
    }

    #[test]
    fn cancellation_delegates_pending_text_release_to_tongues() {
        let mut commit = StreamingSpeechCommitter::new("answer/cancel").unwrap();
        assert!(commit.push("not stable yet").unwrap().is_empty());
        assert!(!commit.pending_text().is_empty());
        commit.cancel();
        assert!(commit.pending_text().is_empty());
        assert!(commit.push("must not resume").is_err());
        assert!(commit.evidence(None, None, 0).cancelled);
    }
}
