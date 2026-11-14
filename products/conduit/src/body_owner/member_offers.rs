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
