//! Finite port-facing operation for an ordinary bounded typed history.

use crate::{
    decode_historical_timeline_command, encode_historical_timeline_into, BoundedHistoricalTimeline,
    HistoricalTimelineCommandCodecRefusal, HistoricalTimelineOutcome, HistoricalTimelineRefusal,
    MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalOperationOutput {
    pub outcome: HistoricalTimelineOutcome,
    pub timeline_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalOperationRefusal {
    TimelineOutputTooSmall,
    Command(HistoricalTimelineCommandCodecRefusal),
    Timeline(HistoricalTimelineRefusal),
}

/// Owns one already-admitted semantic history. Commands and snapshots cross
/// the exact value contracts named by the checked Form; storage realization
/// and scheduling remain outside this operation.
pub struct BoundedHistoricalOperation {
    timeline: BoundedHistoricalTimeline,
}

impl BoundedHistoricalOperation {
    pub const fn new(timeline: BoundedHistoricalTimeline) -> Self {
        Self { timeline }
    }

    pub fn apply_command(
        &mut self,
        encoded_command: &[u8],
        timeline_output: &mut [u8],
    ) -> Result<HistoricalOperationOutput, HistoricalOperationRefusal> {
        if timeline_output.len() < MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES {
            return Err(HistoricalOperationRefusal::TimelineOutputTooSmall);
        }
        let command = decode_historical_timeline_command(encoded_command)
            .map_err(HistoricalOperationRefusal::Command)?;
        let outcome = self
            .timeline
            .apply(command)
            .map_err(HistoricalOperationRefusal::Timeline)?;
        let timeline_bytes = encode_historical_timeline_into(&self.timeline, timeline_output)
            .expect("an admitted timeline fits the preflighted maximum snapshot buffer");
        Ok(HistoricalOperationOutput {
            outcome,
            timeline_bytes,
        })
    }

    pub const fn timeline(&self) -> &BoundedHistoricalTimeline {
        &self.timeline
    }
}
