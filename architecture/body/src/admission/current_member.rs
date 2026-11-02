//! Read-only attribution of one currently attached member to its retained key.
//! No return admission, membership change, challenge or Line is issued here.
use super::{AdmissionManager, AdmissionRefusal};
use crate::{BodyId, BodyMembership, FederationPeer, MembershipRefusal, PartId};
use conduit_core::HostAdvertisement;

/// Owner-local evidence, constructible only from the admission manager's stored
/// continuity key and the exact current Part observation. Deliberately neither
/// deserializable nor a capability/Line grant. Public peer facts remain descriptive.
pub struct CurrentMemberContinuity {
    body_id: BodyId,
    part_id: PartId,
    peer: FederationPeer,
}
impl CurrentMemberContinuity {
    pub fn body_id(&self) -> &BodyId {
        &self.body_id
    }
    pub fn part_id(&self) -> &PartId {
        &self.part_id
    }
    pub fn peer(&self) -> &FederationPeer {
        &self.peer
    }
}
impl AdmissionManager {
    /// Resolve the peer key from retained admission custody, never from a
    /// received key claim. This is intentionally separate from offline return.
    pub fn current_member_continuity(
        &self,
        membership: &BodyMembership,
        part_id: &PartId,
        advertisement: &HostAdvertisement,
    ) -> Result<CurrentMemberContinuity, AdmissionRefusal> {
        if membership.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        let mut parts = membership
            .parts
            .iter()
            .filter(|part| &part.part_id == part_id);
        let part = parts
            .next()
            .ok_or(AdmissionRefusal::Membership(MembershipRefusal::UnknownPart))?;
        if parts.next().is_some() || !part.is_present() {
            return Err(AdmissionRefusal::CandidateNotEligible);
        }
        let current = part
            .current
            .as_ref()
            .ok_or(AdmissionRefusal::CandidateNotEligible)?;
        if current.host_id != advertisement.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if current.boot_id != advertisement.boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        if current.offer_generation != advertisement.offer_generation {
            return Err(AdmissionRefusal::StaleOfferGeneration);
        }
        let mut keys = self
            .continuity_keys
            .iter()
            .filter(|key| &key.part_id == part_id);
        let key = keys.next().ok_or(AdmissionRefusal::InvalidProof)?;
        if keys.next().is_some() {
            return Err(AdmissionRefusal::InvalidProof);
        }
        if key.host_id != current.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        Ok(CurrentMemberContinuity {
            body_id: self.body_id.clone(),
            part_id: part_id.clone(),
            peer: FederationPeer {
                host_id: current.host_id.clone(),
                boot_id: current.boot_id.clone(),
                offer_generation: current.offer_generation,
                verifying_key: key.verifying_key,
            },
        })
    }
}
