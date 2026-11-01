//! Root settlement branches keep normal completion and interruption distinct.
use super::*;

impl RootIssuer {
    /// Finish only the exact completed Mask owner in the still-current Body
    /// Play. This releases issuance bookkeeping, never a sequence or generation.
    pub fn finish_mask_play(
        &mut self,
        body: &conduit_body::SealedBodyLifecycleSession,
        initial: &c::ActivePlayIdentity,
        retired: &crate::native_display::task::RetiredMask,
    ) -> Result<(), &'static str> {
        let native = retired.renderer_identity();
        if !self.display_issued
            || self.issued_display_play.as_deref() != Some(initial)
            || native.host_id != self.host
            || native.boot_id != self.boot
            || native.provider_generation != self.display_generation
            || !retired.matches_current_body(body, initial)
        {
            return Err("mask-terminal-issuer-scope");
        }
        self.display_issued = false;
        self.issued_display_play = None;
        if self.reserved_display_play.as_deref() == Some(initial) {
            self.reserved_display_play = None;
        }
        Ok(())
    }
    /// Settle a revoked, acknowledged finite prefix without claiming its
    /// unissued children completed or resetting their reserved sequence range.
    pub fn finish_interrupted_mask_play(
        &mut self,
        body: &conduit_body::SealedBodyLifecycleSession,
        initial: &c::ActivePlayIdentity,
        retired: &crate::native_display::task::InterruptedMask,
    ) -> Result<(), &'static str> {
        let native = retired.renderer_identity();
        if !self.display_issued
            || self.issued_display_play.as_deref() != Some(initial)
            || native.host_id != self.host
            || native.boot_id != self.boot
            || native.provider_generation != self.display_generation
            || !retired.matches_current_body(body, initial)
        {
            return Err("mask-interruption-issuer-scope");
        }
        self.display_issued = false;
        self.issued_display_play = None;
        if self.reserved_display_play.as_deref() == Some(initial) {
            self.reserved_display_play = None;
        }
        Ok(())
    }
}
