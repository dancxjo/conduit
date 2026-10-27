//! Retained timer transfer from the ordinary Core-verified Plan scope.
use super::*;
use conduit_plan_lowering::selected_operation::{
    bind_selected_direct_configured_operation, bind_selected_verified_configured_operation,
    SelectedCallRefusal, SelectedDirectOperationPlan, SelectedOperationContract,
    SelectedVerifiedOperationPlan,
};
impl<P: MonotonicWaitProvider> ReadyMonotonicWaitBase<P> {
    /// Check all native identities, attachment bounds and independently retained
    /// Source configuration without taking the timer or invoking a wait.
    pub fn check_verified(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedVerifiedOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, WaitCallRefusal> {
        let view = selected
            .plan
            .fragment(selected.fragment_id)
            .ok_or(WaitCallRefusal::WrongBinding)?;
        let gear = view
            .lowering_metadata()
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
            .ok_or(WaitCallRefusal::WrongBinding)?;
        self.check_native(claim, gear)?;
        let kind = crate::owner::checked_contract(table, claim, self.attachment, configuration)?;
        bind_selected_verified_configured_operation(
            table,
            claim,
            SelectedOperationContract {
                kind: &kind,
                call: WAIT_HOST_CALL_CONTRACT,
                input_bytes: PARAMETER_BYTES,
                output_bytes: 0,
                resource_bytes: self.attachment.resource_bytes,
            },
            configuration,
            selected,
        )
        .map_err(|error| match error {
            SelectedCallRefusal::WrongBinding => WaitCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => WaitCallRefusal::Possession,
        })
    }
    /// Refusals preserve the initialized slot. Authority must be locally issued;
    /// expected configuration comes from independently checked resident Source.
    pub fn bind_verified_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedVerifiedOperationPlan<'_>,
    ) -> Result<MonotonicWaitHostCall<P>, WaitCallRefusal> {
        let node = slot
            .as_ref()
            .ok_or(WaitCallRefusal::Possession)?
            .check_verified(&table, &claim, configuration, selected)?;
        BoundBaseOperation::check(&table, &handle, &claim).map_err(WaitCallRefusal::Capability)?;
        let owner = slot.take().expect("preflighted retained timer");
        let seal = identity_seal(&owner.identity, owner.identity.provider_generation);
        Ok(MonotonicWaitHostCall::from_bound(
            table,
            handle,
            claim,
            owner.attachment,
            owner.provider,
            node,
        )
        .with_native_return(seal))
    }
    /// Check all native identities, attachment bounds and independently retained
    /// Source configuration without taking the timer or invoking a wait.
    pub fn check_direct<
        const N: usize,
        const C: usize,
        const PORTS: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    >(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedDirectOperationPlan<'_, N, C, PORTS, R, T, H>,
    ) -> Result<conduit_kernel::NodeId, WaitCallRefusal> {
        let (_, fragment) = selected.tables.preparation_scope();
        let gear = fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selected.placement_id)
            .ok_or(WaitCallRefusal::WrongBinding)?;
        self.check_native(claim, gear)?;
        let kind = crate::owner::checked_contract(table, claim, self.attachment, configuration)?;
        bind_selected_direct_configured_operation(
            table,
            claim,
            SelectedOperationContract {
                kind: &kind,
                call: WAIT_HOST_CALL_CONTRACT,
                input_bytes: PARAMETER_BYTES,
                output_bytes: 0,
                resource_bytes: self.attachment.resource_bytes,
            },
            configuration,
            selected,
        )
        .map_err(|error| match error {
            SelectedCallRefusal::WrongBinding => WaitCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => WaitCallRefusal::Possession,
        })
    }
    /// Refusals preserve the initialized slot. Authority must be locally issued;
    /// expected configuration comes from independently checked resident Source.
    pub fn bind_direct_retained<
        const N: usize,
        const C: usize,
        const PORTS: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    >(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selected: SelectedDirectOperationPlan<'_, N, C, PORTS, R, T, H>,
    ) -> Result<MonotonicWaitHostCall<P>, WaitCallRefusal> {
        let node = slot
            .as_ref()
            .ok_or(WaitCallRefusal::Possession)?
            .check_direct(&table, &claim, configuration, selected)?;
        BoundBaseOperation::check(&table, &handle, &claim).map_err(WaitCallRefusal::Capability)?;
        let owner = slot.take().expect("preflighted retained timer");
        let seal = identity_seal(&owner.identity, owner.identity.provider_generation);
        Ok(MonotonicWaitHostCall::from_bound(
            table,
            handle,
            claim,
            owner.attachment,
            owner.provider,
            node,
        )
        .with_native_return(seal))
    }
}
