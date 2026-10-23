//! Explicitly selected forwarding storage; incoming Face size never upgrades it.
use crate::native_face_snapshot::{
    CollectionFaceSnapshotProducer, FaceSnapshotRefusal, NativeFaceSnapshotProducer,
    PublishedFaceSnapshot,
};
use conduit_core::{BootId, HostId, OfferGeneration, Plan, PlotIdentity};
use conduit_face::Face;

pub(super) enum Producer {
    Standard(NativeFaceSnapshotProducer),
    Collection(CollectionFaceSnapshotProducer),
}
impl Producer {
    pub fn prepare(
        collection: bool,
        host: HostId,
        boot: BootId,
        generation: OfferGeneration,
        build: &str,
    ) -> Result<Self, FaceSnapshotRefusal> {
        if collection {
            CollectionFaceSnapshotProducer::prepare(host, boot, generation, build)
                .map(Self::Collection)
        } else {
            NativeFaceSnapshotProducer::prepare(host, boot, generation, build).map(Self::Standard)
        }
    }
    pub fn storage_profile(&self) -> conduit_conduitos_mask_offer::NativeMaskStorageProfile {
        use conduit_conduitos_mask_offer::NativeMaskStorageProfile;
        match self {
            Self::Standard(_) => NativeMaskStorageProfile::Standard,
            Self::Collection(_) => NativeMaskStorageProfile::Collection,
        }
    }
    pub fn plan(&self) -> &Plan {
        match self {
            Self::Standard(producer) => producer.plan(),
            Self::Collection(producer) => producer.plan(),
        }
    }
    pub fn plot_identity(&self) -> PlotIdentity {
        match self {
            Self::Standard(producer) => producer.plot_identity(),
            Self::Collection(producer) => producer.plot_identity(),
        }
    }
    pub fn forward(
        &mut self,
        face: Face,
        observation_sequence: u64,
        play_sequence: u64,
    ) -> Result<PublishedFaceSnapshot, FaceSnapshotRefusal> {
        match self {
            Self::Standard(producer) => producer.forward(face, observation_sequence, play_sequence),
            Self::Collection(producer) => {
                producer.forward(face, observation_sequence, play_sequence)
            }
        }
    }
}

#[cfg(test)]
#[path = "collection_tests.rs"]
mod collection_tests;

impl super::NativeFaceMask {
    /// Actual initialized Mask storage and completed renderer custody. Absent
    /// before successful acknowledgement and after retirement; never a label.
    pub fn render_receipt(&self) -> Option<&crate::native_mask_play::NativeMaskPlayReceipt> {
        self.session
            .as_ref()
            .map(|session| session.render_receipt())
    }
    pub fn birth_basis(&self, encounter_id: &str) -> conduit_birth_plot::BirthFaceBasis {
        let fragment = &self.producer.plan().fragments[0];
        conduit_birth_plot::BirthFaceBasis {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            encounter_id: encounter_id.into(),
            producer_plot: self.producer.plot_identity(),
            producer_plan_id: self.producer.plan().plan_id.clone(),
        }
    }
}
