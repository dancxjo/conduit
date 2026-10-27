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

// Borrowed local scopes are minted by ordinary Source planning and checked
// against the independent compiled Source pin before Root issues any bearer.
impl RootIssuer {
    fn check_linked_scope(
        &self,
        scope: &conduit_planner::borrowed_local::SourcePlannedLocalScope<'_>,
        expected: (&str, &str, &str),
    ) -> Result<(), ()> {
        let view = scope.verified().view();
        if view.header.host_id != self.host.as_str()
            || view.header.boot_id != self.boot.as_str()
            || (
                view.header.source_document_id,
                view.header.checked_plot_id,
                view.header.expanded_plot_id,
            ) != expected
        {
            return Err(());
        }
        Ok(())
    }
    pub fn reserve_linked_network_play(
        &mut self,
        scope: &conduit_planner::borrowed_local::SourcePlannedLocalScope<'_>,
        expected: (&str, &str, &str),
    ) -> Result<c::ActivePlayIdentity, ()> {
        self.check_linked_scope(scope, expected)?;
        self.reserve_plan(&scope.verified().view().plan_id.into())
    }
    pub fn issue_linked_network(
        &mut self,
        native: &NetworkNativeIdentity,
        operation: Operation,
        scope: &conduit_planner::borrowed_local::SourcePlannedLocalScope<'_>,
        expected: (&str, &str, &str),
        active: &c::ActivePlayIdentity,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        (),
    > {
        self.check_linked_scope(scope, expected)?;
        let v = scope.verified().view();
        self.issue_network_scope(
            native,
            operation,
            v.fragment_plan_id,
            v.header.host_id,
            v.header.boot_id,
            active,
        )
    }
}
