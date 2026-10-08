//! Host-owned forwarding of an authorized immutable Face snapshot.
//!
//! The caller owns the observed facts and any actions. This ordinary Plot emits
//! those supplied facts through an admitted Fore; it does not compute Birth
//! logic, grant authority, or start a Body workload. Producer and Mask Plans
//! remain distinct. Only a completed kernel invocation returns a publication.

use alloc::string::String;
use conduit_core::{ActivePlayId, BootId, HostId, OfferGeneration, Plan, PlanId, PlotIdentity};
use conduit_presentation::{Presentation, PresentationBasis};
use serde::Serialize;

mod execution;
mod planning;
#[cfg(test)]
mod tests;
pub use execution::PreparedFaceSnapshot;

/// Explicit native forwarding profile: one value on each of two Fore cords.
/// Larger semantic Faces need a separately admitted storage profile.
pub const MAX_SNAPSHOT_BYTES: usize = 48 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaceSnapshotRefusal {
    HostIdentity,
    Catalog,
    Plan,
    Shape,
    Presentation,
    ProducerBasis,
    Capacity,
    Kernel,
    UnexpectedEffect,
    ForeInput,
    ForeOutput,
    WorkBound,
    PlaySequence,
}

pub struct NativeFaceSnapshotProducer {
    plan: Plan,
    last_play_sequence: Option<u64>,
    lowered: conduit_plan_lowering::lowering::LoweredPlanFragment,
}

/// Provenance for a trusted local caller's observation, not authentication or
/// proof that the forwarding Plot itself computed the supplied facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FaceSnapshotReceipt {
    /// Basis supplied by the trusted Face owner, distinct from this forwarding Plot.
    pub observed_face_basis: PresentationBasis,
    pub producer_plot: PlotIdentity,
    pub producer_plan_id: PlanId,
    pub producer_active_play_id: ActivePlayId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub observation_sequence: u64,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub encoded_bytes: u32,
    pub value_sha256: [u8; 32],
    pub kernel_signs: u16,
    pub fore_endpoints: u16,
}

/// Publication is returned only after the actual output was delivered and its
/// Fore closed normally. Rendering and Show acknowledgement are separate work.
pub struct PublishedFaceSnapshot {
    pub presentation: Presentation,
    pub receipt: FaceSnapshotReceipt,
}

impl NativeFaceSnapshotProducer {
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    pub fn plot_identity(&self) -> PlotIdentity {
        PlotIdentity {
            source_document_id: self.plan.source_document_id.clone(),
            checked_plot_id: self.plan.checked_plot_id.clone(),
            expanded_plot_id: self.plan.expanded_plot_id.clone(),
        }
    }

    pub fn forward(
        &mut self,
        presentation: Presentation,
        observation_sequence: u64,
        play_sequence: u64,
    ) -> Result<PublishedFaceSnapshot, FaceSnapshotRefusal> {
        self.prepare_snapshot(presentation, observation_sequence, play_sequence)?
            .run()
    }
}
