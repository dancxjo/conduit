//! Fresh member equipment observations, shared by inspection and Mask selection.
//! Retained membership never reconstructs live offers after owner restart.
use super::Owner;
use conduit_body::{BodyBiographyEvidence, CurrentHostOffers};
use conduit_core::HostAdvertisement;

impl Owner {
    /// Stage against the future authenticated biography. Publish only after its
    /// durable commit; capacity or persistence refusal preserves current offers.
    pub(super) fn next_member_offers(
        &self,
        evidence: &BodyBiographyEvidence,
        advertisement: HostAdvertisement,
    ) -> Result<CurrentHostOffers, String> {
        self.next_member_offers_with_grants(evidence, advertisement, Vec::new())
    }

    pub(super) fn next_member_offers_with_grants(
        &self,
        evidence: &BodyBiographyEvidence,
        advertisement: HostAdvertisement,
        grants: Vec<conduit_core::AuthorityGrant>,
    ) -> Result<CurrentHostOffers, String> {
        let mut offers = self.current_host_offers.clone();
        offers.reconcile(evidence);
        offers
            .observe_authorized(evidence, advertisement, grants)
            .map_err(|error| format!("current-member-offer-refused:{error:?}"))?;
        Ok(offers)
    }

    pub(crate) fn require_observed_member_offer(
        &self,
        advertisement: &HostAdvertisement,
    ) -> Result<(), String> {
        if self
            .current_host_offers
            .current_hosts(self.session.evidence())
            .any(|observed| observed == advertisement)
        {
            Ok(())
        } else {
            Err("member offer differs from current admitted observation".into())
        }
    }

    /// Local offers keep the ordinary planner's first-choice position. Remote
    /// equipment is evidence only; this creates no Lines or preparation rights.
    pub(super) fn planning_hosts(&self) -> Vec<HostAdvertisement> {
        let local = self.host.advertisement();
        let mut hosts = vec![local.clone()];
        hosts.extend(
            self.current_host_offers
                .current_hosts(self.session.evidence())
                .filter(|host| host.host_id != local.host_id)
                .cloned(),
        );
        hosts
    }

    /// Bounded inspection avoids copying every member's complete catalog into
    /// each owner control response. These counts do not assert Play readiness.
    pub(super) fn current_offer_summary(&self) -> Vec<serde_json::Value> {
        self.current_host_offers
            .current_hosts(self.session.evidence())
            .map(|host| {
                serde_json::json!({
                    "host_id":host.host_id,
                    "boot_id":host.boot_id,
                    "offer_generation":host.offer_generation,
                    "profile":host.profile,
                    "capabilities":host.capabilities.len(),
                    "resources":host.resources.len(),
                })
            })
            .collect()
    }
}

/// Compact pin into independently retained admitted offer custody. Never an
/// advertisement reconstruction or authority witness; each use resolves actual
/// current records and rechecks complete content, including same-generation
/// replacement. No positional index can dangle or select a different member.
#[derive(Clone)]
pub(super) struct PinnedMemberOffer {
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
    generation: conduit_core::OfferGeneration,
    digest: [u8; 32],
}
fn offer_digest(offer: &HostAdvertisement) -> Option<[u8; 32]> {
    use sha2::{Digest, Sha256};
    struct Writer(Sha256);
    impl std::io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    // Stream the exact ordinary offer encoding directly into the digest. No
    // encoded advertisement buffer is retained or allocated for this pin.
    let mut writer = Writer(Sha256::new());
    serde_json::to_writer(&mut writer, offer).ok()?;
    Some(writer.0.finalize().into())
}
impl PinnedMemberOffer {
    pub(super) fn matches(&self, offer: &HostAdvertisement) -> bool {
        offer.host_id == self.host
            && offer.boot_id == self.boot
            && offer.offer_generation == self.generation
            && offer_digest(offer) == Some(self.digest)
    }
    pub(super) fn current<'a>(
        &self,
        offers: &'a CurrentHostOffers,
        evidence: &BodyBiographyEvidence,
    ) -> Option<&'a HostAdvertisement> {
        let candidate = offers.hosts().iter().find(|offer| self.matches(offer))?;
        offers
            .current_hosts(evidence)
            .any(|current| std::ptr::eq(current, candidate))
            .then_some(candidate)
    }
    pub(super) fn generation(&self) -> conduit_core::OfferGeneration {
        self.generation
    }
}
impl Owner {
    pub(super) fn pin_admitted_member_offer(
        &self,
        offer: &HostAdvertisement,
    ) -> Result<PinnedMemberOffer, String> {
        // Actual retained observation is required; matching incoming labels
        // alone cannot create this compact reference.
        let retained = self
            .current_host_offers
            .current_hosts(self.session.evidence())
            .find(|retained| *retained == offer)
            .ok_or("member offer has no exact current admitted custody")?;
        Ok(PinnedMemberOffer {
            host: retained.host_id.clone(),
            boot: retained.boot_id.clone(),
            generation: retained.offer_generation,
            digest: offer_digest(retained).ok_or("member offer digest failed")?,
        })
    }
}
