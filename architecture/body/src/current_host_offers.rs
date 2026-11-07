//! Fresh Host offers observed for one Body planning interval.
//!
//! Membership proves which Host incarnation is a current Part. An advertisement
//! describes what that incarnation offers now. Keeping those facts separate
//! prevents durable membership from becoming stale capability truth.

use alloc::vec::Vec;
use conduit_core::{AuthorityGrant, HostAdvertisement, PROTOCOL_VERSION};

use crate::{
    BodyBiographyEvidence, MembershipState, MAX_BODY_PARTS, MAX_CANDIDATE_CAPABILITIES,
    MAX_CANDIDATE_PLANNER_CAPABILITIES, MAX_CANDIDATE_RESOURCES,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurrentHostOffers {
    hosts: Vec<HostAdvertisement>,
    grants: Vec<AuthorityGrant>,
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

    /// Read only exact current incarnations, even if membership changed since
    /// the last explicit reconciliation. This is planning evidence, not a lease,
    /// resource reservation, Line, or permission to execute.
    pub fn current_hosts<'a>(
        &'a self,
        evidence: &'a BodyBiographyEvidence,
    ) -> impl Iterator<Item = &'a HostAdvertisement> + 'a {
        self.hosts.iter().filter(move |ad| is_current(evidence, ad))
    }

    /// Observe one exact offer only while its Host/Boot/generation is the
    /// authenticated current incarnation of an admitted Body Part.
    pub fn observe(
        &mut self,
        evidence: &BodyBiographyEvidence,
        advertisement: HostAdvertisement,
    ) -> Result<(), CurrentHostOfferError> {
        self.observe_authorized(evidence, advertisement, Vec::new())
    }

    /// Caller must independently authenticate these original descriptions and
    /// commit the fresh member return before publishing this staged collection.
    /// This method checks correspondence; it does not create execution rights.
    pub fn observe_authorized(
        &mut self,
        evidence: &BodyBiographyEvidence,
        advertisement: HostAdvertisement,
        grants: Vec<AuthorityGrant>,
    ) -> Result<(), CurrentHostOfferError> {
        validate_shape(&advertisement)?;
        crate::authority_grant_descriptions::validate(&advertisement, &grants)
            .map_err(|_| CurrentHostOfferError::MalformedAdvertisement)?;
        for grant in &grants {
            let mut matching = advertisement
                .capabilities
                .iter()
                .filter(|offer| offer.capability_id == grant.capability_id);
            let offer = matching
                .next()
                .ok_or(CurrentHostOfferError::MalformedAdvertisement)?;
            if matching.next().is_some()
                || !offer.authority_requirements.iter().any(|requirement| {
                    requirement.contract_id == grant.contract_id
                        && requirement.host_call_contract_id == grant.host_call_contract_id
                        && requirement.subject_kind == grant.subject_kind
                })
            {
                return Err(CurrentHostOfferError::MalformedAdvertisement);
            }
        }
        if !is_current(evidence, &advertisement) {
            return Err(CurrentHostOfferError::NotCurrentMember);
        }
        if self.hosts.len() == MAX_BODY_PARTS
            && !self
                .hosts
                .iter()
                .any(|host| host.host_id == advertisement.host_id)
        {
            return Err(CurrentHostOfferError::CapacityExceeded);
        }
        self.grants
            .retain(|grant| grant.host_id != advertisement.host_id);
        self.grants.extend(grants);
        if let Some(index) = self
            .hosts
            .iter()
            .position(|host| host.host_id == advertisement.host_id)
        {
            self.hosts[index] = advertisement;
        } else {
            self.hosts.push(advertisement);
        }
        self.hosts
            .sort_by(|left, right| left.host_id.cmp(&right.host_id));
        Ok(())
    }

    /// Descriptions are usable only beside the exact current offer observation.
    pub fn current_grants<'a>(
        &'a self,
        evidence: &'a BodyBiographyEvidence,
    ) -> impl Iterator<Item = &'a AuthorityGrant> + 'a {
        self.grants.iter().filter(move |grant| {
            self.current_hosts(evidence)
                .any(|ad| ad.host_id == grant.host_id && ad.boot_id == grant.boot_id)
        })
    }

    /// Remove observations whose exact incarnation is no longer current.
    pub fn reconcile(&mut self, evidence: &BodyBiographyEvidence) {
        self.hosts
            .retain(|advertisement| is_current(evidence, advertisement));
        self.grants.retain(|grant| {
            self.hosts
                .iter()
                .any(|ad| ad.host_id == grant.host_id && ad.boot_id == grant.boot_id)
        });
    }
}

fn is_current(evidence: &BodyBiographyEvidence, advertisement: &HostAdvertisement) -> bool {
    evidence.membership.parts.iter().any(|part| {
        part.state == MembershipState::Admitted
            && part.current.as_ref().is_some_and(|current| {
                current.host_id == advertisement.host_id
                    && current.boot_id == advertisement.boot_id
                    && current.offer_generation == advertisement.offer_generation
            })
    })
}

fn validate_shape(advertisement: &HostAdvertisement) -> Result<(), CurrentHostOfferError> {
    if advertisement.offer_generation.0 == 0
        || advertisement.protocol_version != PROTOCOL_VERSION
        || advertisement.host_id.as_str().is_empty()
        || advertisement.boot_id.as_str().is_empty()
        || advertisement.profile.as_str().is_empty()
        || advertisement.resources.len() > MAX_CANDIDATE_RESOURCES
        || advertisement.capabilities.len() > MAX_CANDIDATE_CAPABILITIES
        || advertisement.planner_capabilities.len() > MAX_CANDIDATE_PLANNER_CAPABILITIES
    {
        return Err(CurrentHostOfferError::MalformedAdvertisement);
    }
    Ok(())
}
