//! Prompt-free historical attribution without retaining a Host inventory.
use super::{CostEvidenceSource, CostObservationWindow};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, ExecutionProfileId, HostId, HostProfileId, ImplementationId,
    KindId, KindIdentity, OfferGeneration, ResourceClassId, ResourcePoolId, SignId,
};

/// Read-only explanatory data derived from an admitted source. This is not an
/// Offer, authenticated Sign, current Observation or resource authorization.
/// Its finite identity bounds come from source admission, not request contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostSourceAttribution {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub host_profile: HostProfileId,
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub implementation_artifact: ArtifactId,
    pub resource: Option<(ResourcePoolId, ResourceClassId)>,
    pub sign_id: SignId,
    pub calibration_identity: ArtifactId,
    pub calibration_revision: u64,
    pub window: CostObservationWindow,
}
impl CostSourceAttribution {
    pub fn from_source(source: &CostEvidenceSource<'_>) -> Self {
        let host = source.host();
        let offer = source.offer();
        Self {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            host_profile: host.profile.clone(),
            capability_id: offer.capability_id.clone(),
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            execution_profile_id: offer.implementation.execution_profile_id.clone(),
            implementation_id: offer.implementation.implementation_id.clone(),
            implementation_artifact: offer.implementation.artifact_id.clone(),
            resource: source
                .resource()
                .map(|resource| (resource.pool_id.clone(), resource.class_id.clone())),
            sign_id: source.sign().clone(),
            calibration_identity: source.calibration().identity().clone(),
            calibration_revision: source.calibration().revision(),
            window: source.window().clone(),
        }
    }
}
