//! One admitted closing Flow per speech Play, with no invented per-segment PCM.
use super::*;

/// The existing installed stream Back consumes at most 32 ordered Tongues
/// segments in one Play. A WAV completes only after the whole Flow closes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenBatch {
    pub face_id: String,
    pub face_revision: u64,
    pub source_show_id: String,
    pub stream_identity: String,
    pub source_segments_sha256: String,
    pub segments: Vec<SpokenSegment>,
}

impl SpokenBatch {
    pub fn validate(&self, face: &Presentation, show: &MaskShow) -> Result<(), SpokenFaceRefusal> {
        check_show(face, show)?;
        if self.face_id != face.identity.as_str() || self.face_revision != face.revision {
            return Err(SpokenFaceRefusal::StaleFace);
        }
        if self.source_show_id != show.show_id.as_str() {
            return Err(SpokenFaceRefusal::StaleShow);
        }
        if self.segments.is_empty()
            || self.segments.len() > conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS
        {
            return Err(SpokenFaceRefusal::SpeechReceipt);
        }
        for (index, item) in self.segments.iter().enumerate() {
            let expected_reason = if index + 1 == self.segments.len() {
                SpeechCommitReason::FinalFlush
            } else {
                SpeechCommitReason::TonguesBoundary
            };
            if item.face_id != self.face_id
                || item.face_revision != self.face_revision
                || item.show_id != self.source_show_id
                || item.segment.stream_identity != self.stream_identity
                || item.segment.sequence != index as u32
                || item.segment.reason != expected_reason
                || item.text_sha256 != format!("{:x}", Sha256::digest(item.segment.text.as_bytes()))
                || item.encode_tongues().is_err()
            {
                return Err(SpokenFaceRefusal::SpeechReceipt);
            }
        }
        if self.source_segments_sha256 != source_digest(&self.segments) {
            return Err(SpokenFaceRefusal::SpeechReceipt);
        }
        Ok(())
    }

    pub fn encoded_inputs(&self) -> Result<Vec<crate::ExternalForeInput>, SpokenFaceRefusal> {
        self.segments
            .iter()
            .map(|item| {
                Ok(crate::ExternalForeInput {
                    front_port_id: conduit_core::port_id("segments"),
                    track: conduit_core::ConnectionTrack::Payload,
                    bytes: item.encode_tongues()?,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenBatchAudioReceipt {
    pub stream_identity: String,
    pub source_show_id: String,
    pub source_segments_sha256: String,
    pub speech_plan_id: String,
    pub speech_play_id: String,
    pub provider_sha256: String,
    pub wav_sha256: String,
    pub wav_bytes: u64,
    pub pcm_bytes: u32,
    pub pcm_blocks: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpokenBatchDelivery {
    Completed(SpokenBatchAudioReceipt),
    Cancelled,
    Failed(String),
}

impl SpokenFaceSession {
    /// Fill one planned closing Flow without claiming audio per input. A batch
    /// is pressure-bound until its actual output effect reaches a terminal
    /// outcome; later view content waits for the next Play.
    pub fn next_batch(&mut self) -> Result<Option<SpokenBatch>, SpokenFaceRefusal> {
        self.next_batch_with_limits(
            conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS,
            MAXIMUM_SPEAKABLE_SEGMENT_BYTES,
        )
    }

    /// A selected Back may require shorter text segments as well as fewer
    /// segments per closing Flow. The limits only shape demand; actual PCM
    /// admission remains the Host effect's terminal responsibility.
    pub fn next_batch_with_limits(
        &mut self,
        maximum_segments: usize,
        maximum_text_bytes: usize,
    ) -> Result<Option<SpokenBatch>, SpokenFaceRefusal> {
        if maximum_segments == 0 || maximum_segments > conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS {
            return Err(SpokenFaceRefusal::InvalidValue);
        }
        if !(4..=MAXIMUM_SPEAKABLE_SEGMENT_BYTES).contains(&maximum_text_bytes) {
            return Err(SpokenFaceRefusal::InvalidValue);
        }
        if self.pending.is_some() || self.pending_batch.is_some() {
            return Err(SpokenFaceRefusal::SpeechPressure);
        }
        let mut segments = Vec::with_capacity(maximum_segments);
        while segments.len() < maximum_segments {
            let Some(segment) = self.next_segment_up_to(maximum_text_bytes)? else {
                break;
            };
            self.pending = None;
            segments.push(segment);
        }
        if segments.len() == maximum_segments
            && self.reading.is_some()
            && maximum_segments < conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS
        {
            // A separate Play must receive a separately closed Flow. The
            // reader's semantic place is retained for the next batch.
            if let Some(last) = segments.last_mut() {
                last.segment.reason = SpeechCommitReason::FinalFlush;
            }
            self.batch += 1;
            self.sequence = 0;
        }
        let Some(first) = segments.first() else {
            return Ok(None);
        };
        let batch = SpokenBatch {
            face_id: first.face_id.clone(),
            face_revision: first.face_revision,
            source_show_id: first.show_id.clone(),
            stream_identity: first.segment.stream_identity.clone(),
            source_segments_sha256: source_digest(&segments),
            segments,
        };
        batch.validate(&self.face, &self.show)?;
        self.pending_batch = Some(batch.clone());
        Ok(Some(batch))
    }

    pub fn acknowledge_batch(
        &mut self,
        delivery: SpokenBatchDelivery,
    ) -> Result<Option<SpokenTurnReceipt>, SpokenFaceRefusal> {
        let batch = self
            .pending_batch
            .take()
            .ok_or(SpokenFaceRefusal::SpeechReceipt)?;
        match delivery {
            SpokenBatchDelivery::Completed(receipt) => {
                if receipt.stream_identity != batch.stream_identity
                    || receipt.source_show_id != batch.source_show_id
                    || receipt.source_segments_sha256 != batch.source_segments_sha256
                    || receipt.speech_plan_id.is_empty()
                    || receipt.speech_play_id.is_empty()
                    || !sha256_hex(&receipt.provider_sha256)
                    || !sha256_hex(&receipt.wav_sha256)
                    || receipt.pcm_bytes == 0
                    || receipt.pcm_bytes > 5_760_000
                    || receipt.pcm_blocks == 0
                    || receipt.wav_bytes != u64::from(receipt.pcm_bytes) + 44
                    || self
                        .provider_sha256
                        .as_ref()
                        .is_some_and(|provider| provider != &receipt.provider_sha256)
                {
                    self.pending_batch = Some(batch);
                    return Err(SpokenFaceRefusal::SpeechReceipt);
                }
                self.provider_sha256 = Some(receipt.provider_sha256.clone());
                self.completed_segments += batch.segments.len() as u32;
                self.produced_pcm_bytes += u64::from(receipt.pcm_bytes);
                self.correlation
                    .update(batch.source_segments_sha256.as_bytes());
                self.correlation.update(receipt.speech_plan_id.as_bytes());
                self.correlation.update(receipt.speech_play_id.as_bytes());
                self.correlation.update(receipt.wav_sha256.as_bytes());
                self.correlation.update(receipt.pcm_bytes.to_le_bytes());
                if self.reading.is_none() {
                    let outcome = if self.cancel_requested {
                        SpokenTurnOutcome::Cancelled
                    } else {
                        SpokenTurnOutcome::Completed
                    };
                    return Ok(Some(self.finish_turn(outcome)));
                }
                Ok(None)
            }
            SpokenBatchDelivery::Cancelled => {
                self.reading = None;
                Ok(Some(self.finish_turn(SpokenTurnOutcome::Cancelled)))
            }
            SpokenBatchDelivery::Failed(reason) => {
                self.reading = None;
                Ok(Some(self.finish_turn(SpokenTurnOutcome::Failed(reason))))
            }
        }
    }
}

fn source_digest(segments: &[SpokenSegment]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"conduit.spoken-face/segments@1\0");
    for item in segments {
        hash.update(item.segment.sequence.to_le_bytes());
        hash.update((item.segment.text.len() as u32).to_le_bytes());
        hash.update(item.segment.text.as_bytes());
        hash.update(item.text_sha256.as_bytes());
        hash.update([match item.segment.reason {
            SpeechCommitReason::TonguesBoundary => 0,
            SpeechCommitReason::FinalFlush => 1,
        }]);
        hash.update(item.face_id.as_bytes());
        hash.update(item.face_revision.to_le_bytes());
        hash.update(item.show_id.as_bytes());
        hash.update(
            item.clause_index
                .map(|index| index as u64)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hash.update(format!("{:?}", item.clause_provenance).as_bytes());
    }
    format!("{:x}", hash.finalize())
}
