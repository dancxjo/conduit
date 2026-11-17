//! The original owner identity is prepared only by explicit local setup.
use super::*;
use conduit_body::FederationPeer;
use conduit_std_host::body_execution::StartupOwnerChallenge;

impl Owner {
    pub(crate) fn prepare_current_member_carrier_receiver(
        &mut self,
    ) -> Result<FederationPeer, String> {
        self.require_workset_recovered()?;
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("original owner identity setup requires an idle owner".into());
        }
        if self.member_carrier_identity.is_none() {
            let identity = self.host.prepare_startup_owner_identity()?;
            self.member_carrier_identity = Some(identity);
        }
        self.member_carrier_peer()
    }

    pub(crate) fn member_carrier_peer(&self) -> Result<FederationPeer, String> {
        let identity = self
            .member_carrier_identity
            .as_ref()
            .ok_or("original owner identity has not been explicitly prepared")?;
        let peer = identity.peer();
        let actual = self.host.advertisement();
        if peer.host_id != actual.host_id
            || peer.boot_id != actual.boot_id
            || peer.offer_generation != actual.offer_generation
        {
            return Err("original owner identity differs from current Host/Boot".into());
        }
        Ok(peer.clone())
    }

    pub(super) fn startup_member_peer(
        &self,
        selection: &StartupMemberSelection,
    ) -> Result<FederationPeer, String> {
        self.require_current_native_member(&selection.member)?;
        self.admissions
            .as_ref()
            .ok_or("original admission manager is absent")?
            .current_member_continuity(
                &self.session.evidence().membership,
                &selection.member.credential.part_id,
                &selection.member.host_advertisement,
            )
            .map(|continuity| continuity.peer().clone())
            .map_err(|_| "member startup lost original admission key custody".into())
    }

    pub(super) fn sign_startup_member_resume(
        &self,
        selection: &StartupMemberSelection,
        challenge: &StartupOwnerChallenge,
        now_millis: u64,
    ) -> Result<[u8; 64], String> {
        // No setup call here: a received challenge cannot create a signing key.
        let identity = self
            .member_carrier_identity
            .as_ref()
            .ok_or("original owner signing custody is absent")?;
        let peer = self.member_carrier_peer()?;
        let realization = self
            .session
            .realization()
            .ok_or("original Body proposal is absent")?;
        if realization.play.is_some()
            || challenge.body_id != self.session.evidence().body_id
            || challenge.wake_id != realization.wake.wake_id
            || challenge.plan_id != realization.plan.plan_id
            || challenge.global_play
                != self
                    .host
                    .prospective_body_play(&realization.wake, &realization.plan)?
            || challenge.part_id != selection.member.credential.part_id
            || challenge.credential_id != selection.member.credential.credential_id
            || challenge.federation.initiator != self.startup_member_peer(selection)?
            || challenge.federation.responder != peer
        {
            return Err("startup challenge differs from original owner proposal/member".into());
        }
        identity.sign_startup_challenge(challenge, now_millis)
    }
}
