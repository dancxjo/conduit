//! One complete decoded software offer at a time; no readiness is manufactured.
use crate::PlannerError;
use alloc::borrow::Cow;
use conduit_core::{CapabilityId, CapabilityOffer, HostAdvertisement, HostId};

pub(crate) enum OfferAccess<'a> {
    Ordinary(&'a [HostAdvertisement]),
    #[cfg(feature = "indexed-source")]
    Indexed(&'a [crate::indexed_source::IndexedHostOffers<'a>]),
}
impl<'a> OfferAccess<'a> {
    pub fn len(&self) -> usize {
        match self {
            Self::Ordinary(h) => h.len(),
            #[cfg(feature = "indexed-source")]
            Self::Indexed(h) => h.len(),
        }
    }
    pub fn host(&self, index: usize) -> &'a HostAdvertisement {
        match self {
            Self::Ordinary(h) => &h[index],
            #[cfg(feature = "indexed-source")]
            Self::Indexed(h) => h[index].native,
        }
    }
    pub fn count(&self, index: usize) -> Result<usize, PlannerError> {
        let native = self.host(index).capabilities.len();
        match self {
            Self::Ordinary(_) => Ok(native),
            #[cfg(feature = "indexed-source")]
            Self::Indexed(h) => native
                .checked_add(h[index].software.len())
                .ok_or_else(|| PlannerError::PlannerLimitExceeded("indexed offer count".into())),
        }
    }
    pub fn offer(
        &self,
        host: usize,
        index: usize,
    ) -> Result<Cow<'a, CapabilityOffer>, PlannerError> {
        let native = self.host(host);
        if let Some(offer) = native.capabilities.get(index) {
            return Ok(Cow::Borrowed(offer));
        }
        match self {
            Self::Ordinary(_) => Err(PlannerError::UnknownCapability("offer index".into())),
            #[cfg(feature = "indexed-source")]
            Self::Indexed(h) => h[host]
                .software
                .decode(index - native.capabilities.len())
                .map(Cow::Owned),
        }
    }
    pub fn find(
        &self,
        host: usize,
        id: &CapabilityId,
    ) -> Result<Cow<'a, CapabilityOffer>, PlannerError> {
        for i in 0..self.count(host)? {
            let offer = self.offer(host, i)?;
            if &offer.capability_id == id {
                return Ok(offer);
            }
        }
        Err(PlannerError::UnknownCapability(id.as_str().into()))
    }
    pub fn host_index(&self, id: &HostId, reverse: bool) -> Result<usize, PlannerError> {
        let found = if reverse {
            (0..self.len()).rev().find(|i| &self.host(*i).host_id == id)
        } else {
            (0..self.len()).find(|i| &self.host(*i).host_id == id)
        };
        found.ok_or_else(|| PlannerError::UnknownHost(id.as_str().into()))
    }
}

/// Only policy/search evidence survives a candidate scan, not its complete
/// semantic contract, startup defaults, port descriptors or implementation body.
pub(crate) struct CandidateEvidence<'a> {
    pub native: &'a HostAdvertisement,
    pub host: usize,
    pub capability_id: CapabilityId,
    pub artifact_id: conduit_core::ArtifactId,
    pub properties: Cow<'a, [conduit_core::StructuredConfigurationValue]>,
    pub maximum_instances: u16,
    pub different_kind: bool,
}
impl<'a> CandidateEvidence<'a> {
    pub fn capture(
        host: usize,
        native: &'a HostAdvertisement,
        kind: &conduit_core::KindId,
        offer: Cow<'a, CapabilityOffer>,
    ) -> Self {
        let different_kind = offer.kind_id != *kind;
        match offer {
            Cow::Borrowed(offer) => Self {
                host,
                native,
                capability_id: offer.capability_id.clone(),
                artifact_id: offer.implementation.artifact_id.clone(),
                properties: Cow::Borrowed(&offer.realization_properties),
                maximum_instances: offer.limits.max_active_instances,
                different_kind,
            },
            Cow::Owned(offer) => Self {
                host,
                native,
                capability_id: offer.capability_id,
                artifact_id: offer.implementation.artifact_id,
                properties: Cow::Owned(offer.realization_properties),
                maximum_instances: offer.limits.max_active_instances,
                different_kind,
            },
        }
    }
    pub fn fields(&self) -> RealizationCandidateRef<'_> {
        RealizationCandidateRef {
            host: self.native,
            capability_id: &self.capability_id,
            artifact_id: &self.artifact_id,
            realization_properties: &self.properties,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct RealizationCandidateRef<'a> {
    pub host: &'a HostAdvertisement,
    pub capability_id: &'a CapabilityId,
    pub artifact_id: &'a conduit_core::ArtifactId,
    pub realization_properties: &'a [conduit_core::StructuredConfigurationValue],
}
impl<'a> From<(&'a HostAdvertisement, &'a CapabilityOffer)> for RealizationCandidateRef<'a> {
    fn from((host, offer): (&'a HostAdvertisement, &'a CapabilityOffer)) -> Self {
        Self {
            host,
            capability_id: &offer.capability_id,
            artifact_id: &offer.implementation.artifact_id,
            realization_properties: &offer.realization_properties,
        }
    }
}
