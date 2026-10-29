//! Root-owned permission for the actually retained ordinary renderer. No effect.
use super::*;
use conduit_environment_runtime::mask_renderer as r;
impl RootIssuer {
    pub fn grant_mask(&self, native: &r::RendererIdentity) -> Result<c::AuthorityGrant, ()> {
        if native.host_id != self.host || native.boot_id != self.boot {
            return Err(());
        }
        Ok(c::AuthorityGrant {
            grant_id: "grant/esp32/seven-segment/1".into(),
            contract_id: r::AUTHORITY.into(),
            host_call_contract_id: r::CALL.into(),
            subject_kind: conduit_face::RENDERER_KIND.into(),
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            capability_id: r::CAPABILITY.into(),
        })
    }
    pub fn retire_mask_context(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<(), &'static str> {
        if self.display_issued || self.reserved_display_play.as_deref() != Some(active) {
            return Err("mask-reservation-not-dormant");
        }
        self.reserved_display_play = None;
        Ok(())
    }
    /// Caller completed full Source/current-offer admission. Reservation alone
    /// is neither possession nor a current Face/Body route/start permit.
    pub fn reserve_mask_context(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<(), &'static str> {
        if self.reserved_display_play.is_some()
            || self.display_issued
            || active.host_id != self.host
            || active.boot_id != self.boot
            || Some(active.play_sequence) != self.next_play_sequence()
            || *active
                != c::bind_active_play(
                    &active.plan_id,
                    &self.host,
                    &self.boot,
                    active.play_sequence,
                )
        {
            return Err("mask-play-reservation");
        }
        self.last_play_sequence = active.play_sequence;
        self.reserved_display_play = Some(alloc::boxed::Box::new(active.clone()));
        Ok(())
    }
}
