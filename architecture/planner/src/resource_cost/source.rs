//! Immutable attribution for already-admitted producer evidence.
//! This binds snapshots and freshness; it neither authenticates a producer nor
//! grants resource possession. Hard work admission remains the Planner's gate.
use super::{CostEvidenceRefusal as Refusal, CostObservationWindow};
use conduit_core::{
    ArtifactId, CapabilityOffer, HostAdvertisement, MonotonicInstant, ResourceOffer, SignId,
};

pub const MAXIMUM_COST_ID_BYTES: usize = 256;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostCalibrationRevision<'a> {
    identity: &'a ArtifactId,
    revision: u64,
}
impl<'a> CostCalibrationRevision<'a> {
    pub fn new(identity: &'a ArtifactId, revision: u64) -> Result<Self, Refusal> {
        identity_bound(identity.as_str())?;
        if revision == 0 {
            return Err(Refusal::InvalidIdentity);
        }
        Ok(Self { identity, revision })
    }
    pub fn identity(self) -> &'a ArtifactId {
        self.identity
    }
    pub const fn revision(self) -> u64 {
        self.revision
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostScopeChange {
    Host,
    Boot,
    OfferGeneration,
    HostProfile,
    Bases,
    Offer,
    Resource,
    Calibration,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostEvidenceSource<'a> {
    host: &'a HostAdvertisement,
    offer: &'a CapabilityOffer,
    resource: Option<&'a ResourceOffer>,
    sign: &'a SignId,
    calibration: CostCalibrationRevision<'a>,
    window: CostObservationWindow,
}
impl<'a> CostEvidenceSource<'a> {
    pub fn bind(
        host: &'a HostAdvertisement,
        offer: &'a CapabilityOffer,
        resource: Option<&'a ResourceOffer>,
        sign: &'a SignId,
        calibration: CostCalibrationRevision<'a>,
        window: CostObservationWindow,
    ) -> Result<Self, Refusal> {
        for identity in [
            host.host_id.as_str(),
            host.boot_id.as_str(),
            host.profile.as_str(),
            offer.capability_id.as_str(),
            offer.kind_id.as_str(),
            offer.kind_contract_revision.as_str(),
            offer.implementation.execution_profile_id.as_str(),
            offer.implementation.implementation_id.as_str(),
            offer.implementation.artifact_id.as_str(),
            sign.as_str(),
        ] {
            identity_bound(identity)?;
        }
        let mut matching = host
            .capabilities
            .iter()
            .filter(|value| value.capability_id == offer.capability_id);
        if matching.next() != Some(offer) || matching.next().is_some() {
            return Err(Refusal::UnadvertisedSubject);
        }
        if let Some(resource) = resource {
            identity_bound(resource.pool_id.as_str())?;
            identity_bound(resource.class_id.as_str())?;
            let mut matching = host
                .resources
                .iter()
                .filter(|value| value.pool_id == resource.pool_id);
            if matching.next() != Some(resource)
                || matching.next().is_some()
                || !offer
                    .resource_requirements
                    .iter()
                    .any(|need| need.class_id == resource.class_id)
            {
                return Err(Refusal::UnadvertisedSubject);
            }
        }
        if window.observed().clock().host_id() != &host.host_id {
            return Err(Refusal::Scope(CostScopeChange::Host));
        }
        if window.observed().clock().boot_id() != &host.boot_id {
            return Err(Refusal::Scope(CostScopeChange::Boot));
        }
        Ok(Self {
            host,
            offer,
            resource,
            sign,
            calibration,
            window,
        })
    }
    pub fn host(&self) -> &'a HostAdvertisement {
        self.host
    }
    pub fn offer(&self) -> &'a CapabilityOffer {
        self.offer
    }
    pub fn resource(&self) -> Option<&'a ResourceOffer> {
        self.resource
    }
    pub fn sign(&self) -> &'a SignId {
        self.sign
    }
    pub fn calibration(&self) -> CostCalibrationRevision<'a> {
        self.calibration
    }
    pub fn window(&self) -> &CostObservationWindow {
        &self.window
    }
    /// Revalidate the exact immutable subjects before using a sampled cost.
    /// Even a same-generation mutation cannot silently reuse the old profile.
    pub fn validate_at(
        &self,
        current: &HostAdvertisement,
        calibration: CostCalibrationRevision<'_>,
        now: &MonotonicInstant,
    ) -> Result<(), Refusal> {
        use CostScopeChange as Change;
        let changed = if current.host_id != self.host.host_id {
            Some(Change::Host)
        } else if current.boot_id != self.host.boot_id {
            Some(Change::Boot)
        } else if current.offer_generation != self.host.offer_generation {
            Some(Change::OfferGeneration)
        } else if current.protocol_version != self.host.protocol_version
            || current.profile != self.host.profile
        {
            Some(Change::HostProfile)
        } else if current.bases != self.host.bases {
            Some(Change::Bases)
        } else if calibration != self.calibration {
            Some(Change::Calibration)
        } else {
            None
        };
        if let Some(change) = changed {
            return Err(Refusal::Scope(change));
        }
        let mut offers = current
            .capabilities
            .iter()
            .filter(|value| value.capability_id == self.offer.capability_id);
        if offers.next() != Some(self.offer) || offers.next().is_some() {
            return Err(Refusal::Scope(Change::Offer));
        }
        if let Some(resource) = self.resource {
            let mut resources = current
                .resources
                .iter()
                .filter(|value| value.pool_id == resource.pool_id);
            if resources.next() != Some(resource) || resources.next().is_some() {
                return Err(Refusal::Scope(Change::Resource));
            }
        }
        self.window.validate_at(now)
    }
}
fn identity_bound(value: &str) -> Result<(), Refusal> {
    if value.is_empty() || value.len() > MAXIMUM_COST_ID_BYTES {
        Err(Refusal::InvalidIdentity)
    } else {
        Ok(())
    }
}
