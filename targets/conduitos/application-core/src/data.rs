use crate::SLOTS;
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
// Keep the admitted finite payload inline in the shared application wire.
#[allow(clippy::large_enum_variant)]
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
/// Fixed preparation declaration; target bits name exact Target positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum InitSlot {
    Tutorial,
    Tour,
    Patchbay { targets: u8, selected: u8 },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitManifest {
    pub slots: [Option<InitSlot>; SLOTS],
}
impl InitManifest {
    pub fn from_initial(initial: &[Option<Initial>; SLOTS]) -> Self {
        Self {
            slots: core::array::from_fn(|slot| {
                initial[slot].as_ref().map(|initial| match initial {
                    Initial::Tutorial { .. } => InitSlot::Tutorial,
                    Initial::Tour => InitSlot::Tour,
                    Initial::Patchbay { targets, selected } => InitSlot::Patchbay {
                        targets: targets.iter().enumerate().fold(0, |mask, (index, target)| {
                            mask | (u8::from(target.is_some()) << index)
                        }),
                        selected: *selected,
                    },
                })
            }),
        }
    }
}
/// Read-only custody of one biography projection request, never effect authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TutorialScope {
    pub body: conduit_body::BodyId,
    pub outer_revision: u64,
    pub evidence_revision: u32,
    pub lulled: bool,
}
impl TutorialScope {
    pub fn new(
        evidence: &BodyBiographyEvidence,
        outer_revision: u64,
        lulled: bool,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            body: evidence.body_id.clone(),
            outer_revision,
            evidence_revision: u32::try_from(evidence.records.len())
                .map_err(|_| Refusal::Bounds)?,
            lulled,
        })
    }
    pub fn validate(&self, evidence: &BodyBiographyEvidence) -> Result<(), Refusal> {
        if self.body != evidence.body_id
            || usize::try_from(self.evidence_revision).ok() != Some(evidence.records.len())
        {
            return Err(Refusal::Bounds);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
// Keep the admitted finite payload inline in the shared application wire.
#[allow(clippy::large_enum_variant)]
pub enum Command {
    PrepareGraphProjection {
        graph: PatchbayGraph,
        scope: crate::graph_facts::Scope,
    },
    FinishGraphProjection,
    ProjectTutorial {
        evidence: BodyBiographyEvidence,
        scope: TutorialScope,
    },
    PrepareInit {
        manifest: InitManifest,
    },
    PrepareTarget {
        slot: u8,
        target: u8,
        value: Target,
    },
    PrepareTutorial {
        slot: u8,
        evidence: BodyBiographyEvidence,
    },
    PrepareTour {
        slot: u8,
    },
    FinishInit,
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
    GraphFacts {
        contribution: crate::graph_facts::Contribution,
    },
    Tutorial {
        scope: TutorialScope,
        view: Vec<u8>,
    },
    Prepared,
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
mod codec;
mod transport_codec;
pub use codec::{MAX_DECODED_JSON_BYTES, Workspace, decode, decode_reply, encode, encode_command};
