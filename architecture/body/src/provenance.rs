//! Attributable remote claims and bounded, progressive Host-offer disclosure.
//!
//! Authentication establishes who made a claim. It does not promote the
//! claim's proof class or make the claim physically correct.

use alloc::{collections::BTreeSet, vec::Vec};
use conduit_core::{
    ActivePlayId, BootId, CapabilityId, CapabilityOffer, HostBaseId, HostId, HostProfileId,
    ImplementationId, OfferGeneration, PlanId, ResourceOffer, ResourcePoolId, SignId,
    PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::{BodyMembership, CandidateObservation, MembershipState};

pub const MAX_DISCLOSED_CAPABILITIES: usize = 16;
pub const MAX_DISCLOSED_RESOURCES: usize = 16;
pub const MAX_ACCEPTED_PROOF_CLASSES: usize = 8;
pub const MAX_ACCEPTED_SOURCES: usize = 16;

/// A semantic evidence class, deliberately not an ordered reputation score.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RemoteProofClass {
    SelfReported,
    TransportAttributed,
    DeterministicConformance,
    PlatformObserved,
    PhysicalHil,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteClaimProvenance {
    pub asserting_host_id: HostId,
    pub asserting_boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: Option<CapabilityId>,
    pub implementation_id: Option<ImplementationId>,
    pub base_id: Option<HostBaseId>,
    pub resource_pool_id: Option<ResourcePoolId>,
    pub plan_id: Option<PlanId>,
    pub active_play_id: Option<ActivePlayId>,
    pub sign_id: SignId,
    pub freshness_sequence: u64,
    pub proof_class: RemoteProofClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteClaimPolicy {
    /// Empty means any exact source is eligible.
    pub accepted_sources: Vec<HostId>,
    /// Exact acceptable classes. There is no implicit stronger-than ordering.
    pub accepted_proof_classes: Vec<RemoteProofClass>,
    pub require_current_member: bool,
    /// One unless the semantic policy explicitly requires corroboration.
    pub minimum_independent_sources: u16,
    /// A claim admitted only for display cannot enter planning.
    pub use_class: ClaimUseClass,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClaimUseClass {
    DisplayOrDiagnostic,
    Planning,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClaimPolicyRefusal {
    InvalidProvenance,
    PolicyCapacityExceeded,
    SourceNotSelected,
    ProofClassNotAccepted,
    NotCurrentMember,
    DisplayOnly,
    InsufficientIndependentSources,
}

impl RemoteClaimPolicy {
    pub fn admits(
        &self,
        provenance: &RemoteClaimProvenance,
        membership: Option<&BodyMembership>,
        requested_use: ClaimUseClass,
    ) -> Result<(), ClaimPolicyRefusal> {
        self.admits_corroborated(core::slice::from_ref(provenance), membership, requested_use)
    }

    pub fn admits_corroborated(
        &self,
        claims: &[RemoteClaimProvenance],
        membership: Option<&BodyMembership>,
        requested_use: ClaimUseClass,
    ) -> Result<(), ClaimPolicyRefusal> {
        if self.accepted_sources.len() > MAX_ACCEPTED_SOURCES
            || self.accepted_proof_classes.is_empty()
            || self.accepted_proof_classes.len() > MAX_ACCEPTED_PROOF_CLASSES
            || self.minimum_independent_sources == 0
            || self.minimum_independent_sources as usize > MAX_ACCEPTED_SOURCES
        {
            return Err(ClaimPolicyRefusal::PolicyCapacityExceeded);
        }
        let mut independent_sources = BTreeSet::new();
        for provenance in claims {
            validate_provenance(provenance)?;
            if !self.accepted_sources.is_empty()
                && !self
                    .accepted_sources
                    .contains(&provenance.asserting_host_id)
            {
                return Err(ClaimPolicyRefusal::SourceNotSelected);
            }
            if !self
                .accepted_proof_classes
                .contains(&provenance.proof_class)
            {
                return Err(ClaimPolicyRefusal::ProofClassNotAccepted);
            }
            if self.require_current_member
                && !membership.is_some_and(|membership| {
                    membership.parts.iter().any(|part| {
                        part.state == MembershipState::Admitted
                            && part.current.as_ref().is_some_and(|current| {
                                current.host_id == provenance.asserting_host_id
                                    && current.boot_id == provenance.asserting_boot_id
                                    && current.offer_generation == provenance.offer_generation
                            })
                    })
                })
            {
                return Err(ClaimPolicyRefusal::NotCurrentMember);
            }
            independent_sources.insert(provenance.asserting_host_id.clone());
        }
        if requested_use == ClaimUseClass::Planning
            && self.use_class == ClaimUseClass::DisplayOrDiagnostic
        {
            return Err(ClaimPolicyRefusal::DisplayOnly);
        }
        if independent_sources.len() < self.minimum_independent_sources as usize {
            return Err(ClaimPolicyRefusal::InsufficientIndependentSources);
        }
        Ok(())
    }
}

fn validate_provenance(provenance: &RemoteClaimProvenance) -> Result<(), ClaimPolicyRefusal> {
    if provenance.asserting_host_id.as_str().is_empty()
        || provenance.asserting_boot_id.as_str().is_empty()
        || provenance.sign_id.as_str().is_empty()
        || provenance.freshness_sequence == 0
    {
        return Err(ClaimPolicyRefusal::InvalidProvenance);
    }
    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OfferDisclosureStage {
    Discovery,
    AdmittedMembership,
    Planning,
    PrepareSelected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferDisclosureRequest {
    pub stage: OfferDisclosureStage,
    pub capability_ids: Vec<CapabilityId>,
    pub resource_pool_ids: Vec<ResourcePoolId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySummary {
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
}

/// A filtered projection of `HostAdvertisement`, not a second source of truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostOfferProjection {
    pub stage: OfferDisclosureStage,
    pub protocol_version: u16,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub observation_sign_id: SignId,
    pub freshness_sequence: u64,
    pub proof_class: RemoteProofClass,
    pub profile: Option<HostProfileId>,
    pub capability_summary: Vec<CapabilitySummary>,
    pub capabilities: Vec<CapabilityOffer>,
    pub resources: Vec<ResourceOffer>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OfferDisclosureRefusal {
    WrongProtocol,
    InvalidProvenance,
    RequestCapacityExceeded,
    NonCanonicalRequest,
    DetailRequestedTooEarly,
    UnknownCapability,
    UnknownResource,
}

pub fn disclose_host_offer(
    observation: &CandidateObservation,
    proof_class: RemoteProofClass,
    request: &OfferDisclosureRequest,
) -> Result<HostOfferProjection, OfferDisclosureRefusal> {
    let advertisement = &observation.advertisement;
    if advertisement.protocol_version != PROTOCOL_VERSION {
        return Err(OfferDisclosureRefusal::WrongProtocol);
    }
    if advertisement.host_id.as_str().is_empty()
        || advertisement.boot_id.as_str().is_empty()
        || observation.observation_sign_id.as_str().is_empty()
        || observation.freshness_sequence == 0
    {
        return Err(OfferDisclosureRefusal::InvalidProvenance);
    }
    if request.capability_ids.len() > MAX_DISCLOSED_CAPABILITIES
        || request.resource_pool_ids.len() > MAX_DISCLOSED_RESOURCES
    {
        return Err(OfferDisclosureRefusal::RequestCapacityExceeded);
    }
    if !strictly_sorted(&request.capability_ids) || !strictly_sorted(&request.resource_pool_ids) {
        return Err(OfferDisclosureRefusal::NonCanonicalRequest);
    }
    if request.stage < OfferDisclosureStage::Planning
        && (!request.capability_ids.is_empty() || !request.resource_pool_ids.is_empty())
    {
        return Err(OfferDisclosureRefusal::DetailRequestedTooEarly);
    }

    let profile = (request.stage >= OfferDisclosureStage::AdmittedMembership)
        .then(|| advertisement.profile.clone());
    let mut capability_summary = Vec::new();
    if request.stage == OfferDisclosureStage::AdmittedMembership {
        capability_summary = advertisement
            .capabilities
            .iter()
            .take(MAX_DISCLOSED_CAPABILITIES)
            .map(|offer| CapabilitySummary {
                capability_id: offer.capability_id.clone(),
                implementation_id: offer.implementation.implementation_id.clone(),
            })
            .collect();
        capability_summary.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    }

    let mut capabilities = Vec::new();
    for capability_id in &request.capability_ids {
        let offer = advertisement
            .capabilities
            .iter()
            .find(|offer| &offer.capability_id == capability_id)
            .ok_or(OfferDisclosureRefusal::UnknownCapability)?;
        capabilities.push(offer.clone());
    }
    let mut resources = Vec::new();
    for pool_id in &request.resource_pool_ids {
        let offer = advertisement
            .resources
            .iter()
            .find(|offer| &offer.pool_id == pool_id)
            .ok_or(OfferDisclosureRefusal::UnknownResource)?;
        resources.push(offer.clone());
    }

    Ok(HostOfferProjection {
        stage: request.stage,
        protocol_version: advertisement.protocol_version,
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        offer_generation: advertisement.offer_generation,
        observation_sign_id: observation.observation_sign_id.clone(),
        freshness_sequence: observation.freshness_sequence,
        proof_class,
        profile,
        capability_summary,
        capabilities,
        resources,
    })
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
