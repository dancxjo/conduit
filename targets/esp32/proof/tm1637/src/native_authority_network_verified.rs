//! Network scopes may enter only from opaque Core verification, never claimed IDs.
use super::*;
impl RootIssuer {
    pub fn reserve_verified_network_play(
        &mut self,
        plan: &c::VerifiedPlanMetadata,
        id: &c::FragmentId,
    ) -> Result<c::ActivePlayIdentity, ()> {
        let fragment = plan
            .preparation_metadata()
            .fragments
            .iter()
            .find(|f| &f.fragment_id == id)
            .ok_or(())?;
        if fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || &fragment.plan_id != plan.plan_id()
        {
            return Err(());
        }
        self.reserve_plan(plan.plan_id())
    }
    pub fn issue_verified_network(
        &mut self,
        native: &NetworkNativeIdentity,
        operation: Operation,
        plan: &c::VerifiedPlanMetadata,
        id: &c::FragmentId,
        active: &c::ActivePlayIdentity,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        (),
    > {
        let fragment = plan
            .preparation_metadata()
            .fragments
            .iter()
            .find(|f| &f.fragment_id == id)
            .ok_or(())?;
        if &fragment.plan_id != plan.plan_id() {
            return Err(());
        }
        self.issue_network_checked(native, operation, fragment, active)
    }
}
