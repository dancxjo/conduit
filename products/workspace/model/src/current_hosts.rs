//! Fresh Host offers available to one Workspace planning interval.
//!
//! Membership proves which Host incarnation is a current Part. An advertisement
//! describes what that incarnation offers now. Keeping those facts separate
//! prevents durable membership from becoming stale capability truth.
use alloc::vec::Vec;
use conduit_body::{BodyBiographyEvidence, MAX_BODY_PARTS, MembershipState};
use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurrentHostOffers {
    hosts: Vec<HostAdvertisement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrentHostOfferError {
    NotCurrentMember,
    MalformedAdvertisement,
    CapacityExceeded,
}

impl CurrentHostOffers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hosts(&self) -> &[HostAdvertisement] {
        &self.hosts
    }

    /// Observe one exact offer only while its host/Boot/generation is the
    /// authenticated current incarnation of an admitted body Part.
    pub fn observe(
        &mut self,
        evidence: &BodyBiographyEvidence,
        advertisement: HostAdvertisement,
    ) -> Result<(), CurrentHostOfferError> {
        validate_shape(&advertisement)?;
        let current = evidence.membership.parts.iter().any(|part| {
            part.state == MembershipState::Admitted
                && part.current.as_ref().is_some_and(|current| {
                    current.host_id == advertisement.host_id
                        && current.boot_id == advertisement.boot_id
                        && current.offer_generation == advertisement.offer_generation
                })
        });
        if !current {
            return Err(CurrentHostOfferError::NotCurrentMember);
        }
        if let Some(index) = self
            .hosts
            .iter()
            .position(|host| host.host_id == advertisement.host_id)
        {
            self.hosts[index] = advertisement;
        } else {
            if self.hosts.len() == MAX_BODY_PARTS {
                return Err(CurrentHostOfferError::CapacityExceeded);
            }
            self.hosts.push(advertisement);
        }
        self.hosts
            .sort_by(|left, right| left.host_id.cmp(&right.host_id));
        Ok(())
    }

    /// Remove observations whose exact incarnation is no longer current.
    pub fn reconcile(&mut self, evidence: &BodyBiographyEvidence) {
        self.hosts.retain(|advertisement| {
            evidence.membership.parts.iter().any(|part| {
                part.state == MembershipState::Admitted
                    && part.current.as_ref().is_some_and(|current| {
                        current.host_id == advertisement.host_id
                            && current.boot_id == advertisement.boot_id
                            && current.offer_generation == advertisement.offer_generation
                    })
            })
        });
    }
}

fn validate_shape(advertisement: &HostAdvertisement) -> Result<(), CurrentHostOfferError> {
    if advertisement.protocol_version != PROTOCOL_VERSION
        || advertisement.host_id.as_str().is_empty()
        || advertisement.boot_id.as_str().is_empty()
        || advertisement.profile.as_str().is_empty()
        || advertisement.resources.len() > conduit_body::MAX_CANDIDATE_RESOURCES
        || advertisement.capabilities.len() > conduit_body::MAX_CANDIDATE_CAPABILITIES
        || advertisement.planner_capabilities.len()
            > conduit_body::MAX_CANDIDATE_PLANNER_CAPABILITIES
    {
        return Err(CurrentHostOfferError::MalformedAdvertisement);
    }
    Ok(())
}
