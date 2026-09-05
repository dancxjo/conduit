//! Exact projection from retained typed history into reusable replay input.

use alloc::vec::Vec;

use crate::{BoundedHistoricalTimeline, HistoricalReplayEntry, HistoricalRetentionGap};

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
