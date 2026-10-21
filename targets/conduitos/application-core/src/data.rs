use crate::{MAX_BLOB, SLOTS};
use alloc::vec::Vec;
use conduit_body::BodyBiographyEvidence;
use conduit_core::PlanId;
use patchbay_application::{PatchbayApplicationRequest, PatchbayMaskTopology};
use patchbay_graph::PatchbayGraph;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Tutorial,
    Tour,
    Patchbay,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TutorialAction {
    Wake,
    InspectLifecycle,
    OpenLibrary,
    InviteHost,
    UseCurrent,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Proposal {
    Tutorial(TutorialAction),
    RunTour { chapter: u8, stage: u8 },
    OpenPatchbay,
    EditCurrent(PatchbayApplicationRequest),
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub graph: PatchbayGraph,
    pub plan: PlanId,
    pub body: PlanId,
}
#[derive(Clone, Serialize, Deserialize)]
pub enum Initial {
    Tutorial {
        evidence: BodyBiographyEvidence,
    },
    Tour,
    Patchbay {
        targets: [Option<Target>; SLOTS],
        selected: u8,
    },
}
impl Initial {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Tutorial { .. } => Kind::Tutorial,
            Self::Tour => Kind::Tour,
            Self::Patchbay { .. } => Kind::Patchbay,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub enum Command {
    Init {
        applications: [Option<Initial>; SLOTS],
    },
    Apply {
        slot: u8,
        input: Vec<u8>,
    },
    Refresh {
        evidence: BodyBiographyEvidence,
    },
    Select {
        slot: u8,
        target: u8,
    },
    Topology {
        topology: PatchbayMaskTopology,
    },
    CompleteTour {
        slot: u8,
        proof: conduit_tour_model::TourRunProof,
    },
    Graph {
        slot: u8,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Context {
    None,
    Tour { chapter: u8, stage: u8 },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub slot: u8,
    pub view: Vec<u8>,
    pub context: Context,
}
#[derive(Serialize, Deserialize)]
pub enum Reply {
    Initialized,
    Applied {
        snapshot: Snapshot,
        proposal: Option<Proposal>,
    },
    Updated {
        snapshots: [Option<Snapshot>; SLOTS],
    },
    Graph {
        graph: Option<PatchbayGraph>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Bounds,
    Malformed,
    Order,
    Capacity,
    Application,
}
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    if bytes.is_empty() || bytes.len() > MAX_BLOB {
        return Err(Refusal::Bounds);
    }
    serde_json::from_slice(bytes).map_err(|_| Refusal::Malformed)
}
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Refusal> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_BLOB)
        .map_err(|_| Refusal::Capacity)?;
    bytes.resize(MAX_BLOB, 0);
    let length = serde_json_core::to_slice(value, &mut bytes).map_err(|_| Refusal::Capacity)?;
    bytes.truncate(length);
    Ok(bytes)
}
