//! Exact projection from retained typed history into reusable replay input.

use alloc::vec::Vec;

use crate::{
    encode_historical_retention_gap_into, encode_replay_timeline_fields_into,
    BoundedHistoricalTimeline, HistoricalReplayEntry, HistoricalRetentionGap,
    HistoricalRetentionGapCodecRefusal, ReplayTimelineCodecRefusal, HISTORICAL_RETENTION_GAP_BYTES,
    MAXIMUM_REPLAY_TIMELINE_BYTES,
};

pub const REPLAY_SOURCE_KIND: &str = "history/replay-source";
pub const REPLAY_SOURCE_CONTRACT_REVISION: &str = "conduit.history/replay-source@1";

/// The finite replayable view of one retained semantic history. A retention
/// gap remains explicit beside the replay entries and is never fabricated as
/// an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaySourceProjection {
    pub entries: Vec<HistoricalReplayEntry>,
    pub retention_gap: Option<HistoricalRetentionGap>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReplaySourceOutput {
    pub replay_bytes: usize,
    pub gap_bytes: Option<usize>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplaySourceRefusal {
    ReplayOutputTooSmall,
    GapOutputTooSmall,
    Replay(ReplayTimelineCodecRefusal),
    Gap(HistoricalRetentionGapCodecRefusal),
}

pub struct BoundedReplaySourceOperation<'a> {
    timeline: &'a BoundedHistoricalTimeline,
}

impl<'a> BoundedReplaySourceOperation<'a> {
    pub const fn new(timeline: &'a BoundedHistoricalTimeline) -> Self {
        Self { timeline }
    }

    pub fn project_into(
        &self,
        replay_output: &mut [u8],
        gap_output: &mut [u8],
    ) -> Result<ReplaySourceOutput, ReplaySourceRefusal> {
        if replay_output.len() < MAXIMUM_REPLAY_TIMELINE_BYTES {
            return Err(ReplaySourceRefusal::ReplayOutputTooSmall);
        }
        if gap_output.len() < HISTORICAL_RETENTION_GAP_BYTES {
            return Err(ReplaySourceRefusal::GapOutputTooSmall);
        }
        let replay_bytes = encode_replay_timeline_fields_into(
            self.timeline.len(),
            |index| {
                let entry = self
                    .timeline
                    .entry(index)
                    .expect("a retained replay-source index names one entry");
                (entry.identity.as_str(), &entry.event_time)
            },
            replay_output,
        )
        .map_err(ReplaySourceRefusal::Replay)?;
        let gap_bytes = self
            .timeline
            .retention_gap()
            .map(|gap| {
                encode_historical_retention_gap_into(gap, gap_output)
                    .map_err(ReplaySourceRefusal::Gap)
            })
            .transpose()?;
        Ok(ReplaySourceOutput {
            replay_bytes,
            gap_bytes,
        })
    }
}

pub fn project_replay_source(timeline: &BoundedHistoricalTimeline) -> ReplaySourceProjection {
    ReplaySourceProjection {
        entries: timeline.replay_metadata(),
        retention_gap: timeline.retention_gap(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn replay_source_kind_definition() -> conduit_form::KindDefinition {
    use conduit_core::{
        kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
        StructuredInfoType,
    };

    let port = |name, value_kind, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: StructuredInfoType::leaf(kind_id(value_kind))
            .expect("reviewed replay-source value identity")
            .profile()
            .expect("reviewed replay-source value profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    };
    conduit_form::KindDefinition {
        kind_id: kind_id(REPLAY_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(REPLAY_SOURCE_CONTRACT_REVISION),
        inputs: alloc::vec![port(
            "timeline",
            "history/typed-timeline@1",
            PortDirection::Input,
        )],
        outputs: alloc::vec![
            port("replay", "history/replay-timeline@1", PortDirection::Output,),
            port("gap", "history/retention-gap@1", PortDirection::Output,),
        ],
        configuration: alloc::vec![],
    }
}
