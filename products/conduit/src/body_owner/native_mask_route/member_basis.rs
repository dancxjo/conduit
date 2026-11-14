//! Recheck a member presentation claim against current authoritative custody.
use super::*;
impl Owner {
    pub(in crate::durable_host::owner::controller) fn require_current_native_member(
        &self,
        member: &MemberPresentationBasis,
    ) -> Result<(), String> {
        member.validate()?;
        let credential = &member.credential;
        let offered = &member.host_advertisement;
        let current = self.session.evidence().membership.parts.iter().any(|part| {
            part.state == conduit_body::MembershipState::Admitted
                && part.part_id == credential.part_id
                && part.current.as_ref().is_some_and(|host| {
                    host.host_id == offered.host_id
                        && host.boot_id == offered.boot_id
                        && host.offer_generation == offered.offer_generation
                })
        });
        let latest = self.admissions.as_ref().and_then(|manager| {
            manager
                .receipts
                .iter()
                .rev()
                .find(|admitted| admitted.credential.part_id == credential.part_id)
        });
        let retained_offer_mismatch = self.pending_native_mask.as_ref().is_some_and(|route| {
            route.credential == *credential && !route.host_offer.matches(offered)
        });
        if credential.body_id != self.session.evidence().body_id
            || !current
            || latest.is_none_or(|admitted| admitted.credential != *credential)
            || retained_offer_mismatch
        {
            return Err("native-mask-part-unavailable".into());
        }
        Ok(())
    }
}
