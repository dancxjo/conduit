//! Permission projection for Root's actual retained display; no bearer issuance.
use super::*;
use conduit_tm1637_wire::{installation::*, WRITE_CALL, WRITE_KIND};
impl RootIssuer {
    pub fn grant_display(&self, native: &WriteNativeIdentity) -> Result<c::AuthorityGrant, ()> {
        if native.host_id != self.host || native.boot_id != self.boot {
            return Err(());
        }
        Ok(c::AuthorityGrant {
            grant_id: "grant/esp32/tm1637-write/1".into(),
            contract_id: WRITE_AUTHORITY.into(),
            host_call_contract_id: WRITE_CALL.into(),
            subject_kind: WRITE_KIND.into(),
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            capability_id: WRITE_CAPABILITY.into(),
        })
    }
}

#[cfg(any(test, feature = "display-ready-root"))]
impl RootIssuer {
    /// Reserve identity only for independently admitted output tables. Owner
    /// authentication, native possession and Body start remain separate gates.
    pub fn reserve_display_play_matching(
        &mut self,
        assignment: &conduit_environment_runtime::display_output::ValidatedAssignedDisplay,
        plan: &c::PlanId,
        fragment: &c::FragmentId,
        expected: &str,
    ) -> Result<c::ActivePlayIdentity, ()> {
        let identity = assignment.identity();
        let id = c::AssignedIdentity::from_text;
        if identity.host != id(self.host.as_str())
            || identity.boot != id(self.boot.as_str())
            || identity.plan != id(plan.as_str())
            || identity.fragment != id(fragment.as_str())
        {
            return Err(());
        }
        let next = self.next_play_sequence().ok_or(())?;
        let candidate = c::bind_active_play(plan, &self.host, &self.boot, next);
        if candidate.active_play_id.as_str() != expected {
            return Err(());
        }
        self.reserve_plan(plan)
    }
}
