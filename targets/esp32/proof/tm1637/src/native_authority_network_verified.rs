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

/// Exclusive access to the actual issuing Root, minted only while ordinary
/// Source planning's opaque verified scope is live. Neither IDs nor a received
/// descriptor can construct this preparation or change its Root underneath it.
pub struct LinkedNetworkPreparation<'root> {
    issuer: &'root mut RootIssuer,
    active: Option<c::ActivePlayIdentity>,
    reserved: bool,
}
impl RootIssuer {
    pub fn prepare_linked_network<'root>(
        &'root mut self,
        scope: &conduit_planner::borrowed_local::SourcePlannedLocalScope<'_>,
        expected: (&str, &str, &str),
    ) -> Result<LinkedNetworkPreparation<'root>, ()> {
        self.check_linked_scope(scope, expected)?;
        let view = scope.verified().view();
        if view.plan_id != view.fragment_plan_id || self.issued.iter().any(|issued| *issued) {
            return Err(());
        }
        let sequence = self.last_play_sequence.checked_add(1).ok_or(())?;
        let active = c::bind_active_play(&view.plan_id.into(), &self.host, &self.boot, sequence);
        Ok(LinkedNetworkPreparation {
            issuer: self,
            active: Some(active),
            reserved: false,
        })
    }
}
impl LinkedNetworkPreparation<'_> {
    /// A complete descriptive expectation from independently retained native
    /// identity and Root policy. It is never issued or executable possession.
    pub fn expected_scope(
        &self,
        native: &NetworkNativeIdentity,
        operation: Operation,
    ) -> Result<c::BaseCapabilityScope, ()> {
        self.issuer
            .network_scope_for(native, operation, self.active.as_ref().ok_or(())?)
    }

    pub fn reserve(&mut self) -> Result<(), ()> {
        if self.reserved {
            return Err(());
        }
        // Exclusive borrowing has prevented any intervening Root mutation.
        let expected = self.active.as_ref().ok_or(())?;
        let active = self.issuer.reserve_plan(&expected.plan_id)?;
        if &active != expected {
            return Err(());
        }
        // Actual Root reservation now owns this identity; the caller copy is
        // deferred until construction and its temporary claims have retired.
        self.active = None;
        self.reserved = true;
        Ok(())
    }
    pub fn issue_prepared(
        &mut self,
        native: &NetworkNativeIdentity,
        operation: Operation,
        prepared: conduit_plan_lowering::selected_operation::PreparedFlatDirectOperation,
    ) -> Result<conduit_plan_lowering::selected_operation::IssuedFlatDirectOperation, ()> {
        if !self.reserved {
            return Err(());
        }
        let active = self.issuer.reserved_play.as_deref().ok_or(())?;
        let (table, authority, input, index) =
            self.issuer
                .network_issuer_for(native, operation, prepared.expected_scope(), active)?;
        #[cfg(test)]
        let input = if matches!(operation, Operation::Join)
            && REFUSE_NEXT_JOIN_BINDING.with(|flag| flag.replace(false))
        {
            input - 1
        } else {
            input
        };
        let issued = prepared
            .issue(table, authority, active, input, 1)
            .map_err(|_| ())?;
        self.issuer.issued[index] = true;
        Ok(issued)
    }
}

#[cfg(test)]
std::thread_local! {
    static REFUSE_NEXT_JOIN_BINDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
impl RootIssuer {
    /// Real issued possession with an allowed narrower claim, which the exact
    /// native Join call must refuse while retaining both actual bearers.
    pub(crate) fn refuse_next_prepared_join_binding_fixture() {
        REFUSE_NEXT_JOIN_BINDING.with(|flag| flag.set(true));
    }
}
