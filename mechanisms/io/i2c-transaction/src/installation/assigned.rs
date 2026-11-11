//! Compact assignment admission without replacing Root's controller on refusal.
use super::*;
use crate::owner::{check_possession, PreparedI2cBinding};
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_fore_operation, bind_selected_assigned_operation,
        AssignedNativeIdentity, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
    selected_operation::{SelectedCallRefusal, SelectedOperationContract},
};
/// Exclusive preparation custody, not a reservation or permission to Start.
/// Dropping this ticket releases codecs and capability storage while leaving
/// the same ready controller in its original slot. Binding cannot allocate.
/// The ticket owns the move-only table and bearer: no API can revoke, exhaust,
/// replace or mutate that grant between successful preparation and transfer.
pub struct PreparedAssignedI2c<'a, P: I2cProvider> {
    prepared: PreparedI2cBinding,
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    slot: &'a mut Option<ReadyI2cBase<P>>,
    node: conduit_kernel::NodeId,
}
impl<P: I2cProvider> PreparedAssignedI2c<'_, P> {
    /// Owned codec/claim/capability backing only. The borrowed ready installation,
    /// inline frames and allocator overhead must be funded independently.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.prepared
            .retained_backing_bytes()?
            .checked_add(self.table.retained_backing_bytes()?)?
            .checked_add(self.claim.retained_backing_bytes()?)
    }

    pub fn node(&self) -> conduit_kernel::NodeId {
        self.node
    }
    /// Root must independently complete whole-workset resource admission and
    /// arrange lawful Start/retirement. This transfers exactly the checked owner.
    pub fn bind(self) -> I2cHostCall<P> {
        let owner = self.slot.take().expect("exclusive prepared I2C owner");
        I2cHostCall::from_prepared(
            self.table,
            self.handle,
            self.claim,
            owner.attachment,
            owner.provider,
            self.prepared,
        )
    }
}

impl<P: I2cProvider> ReadyI2cBase<P> {
    /// Check the exact assignment against retained native possession; no bus IO.
    pub fn check_assigned(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &I2cContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, I2cCallRefusal> {
        self.prepare_assigned(table, claim, contract, selected, None)
            .map(|(node, _)| node)
    }
    /// Independently pinned local Fore bindings; no network Line is inferred.
    pub fn check_assigned_fore(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &I2cContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<conduit_kernel::NodeId, I2cCallRefusal> {
        self.prepare_assigned(
            table,
            claim,
            contract,
            selected.operation,
            Some(selected.fore),
        )
        .map(|(node, _)| node)
    }
    fn prepare_assigned(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        contract: &I2cContract,
        selected: SelectedAssignedOperationPlan<'_>,
        fore: Option<&[AssignedForeRequirement]>,
    ) -> Result<(conduit_kernel::NodeId, PreparedI2cBinding), I2cCallRefusal> {
        let id = &self.identity;
        if claim.host_id != id.host_id
            || claim.boot_id != id.boot_id
            || claim.base_instance_id != id.provider_instance_id
            || claim.base_provider_generation != id.provider_generation
            || claim.resource_pool_id != id.resource_pool_id
            || claim.resource_generation_id != id.resource_generation_id
            || claim.envelope_id != id.envelope_id
            || claim.implementation_id.as_str() != I2C_IMPLEMENTATION
        {
            return Err(I2cCallRefusal::WrongBinding);
        }
        check_possession(table, claim, self.attachment)?;
        let definition = SelectedOperationContract {
            kind: contract.kind(),
            call: I2C_CALL,
            input_bytes: I2C_MAXIMUM_BYTES,
            output_bytes: I2C_MAXIMUM_BYTES,
            resource_bytes: self.attachment.resource_bytes,
        };
        let native = AssignedNativeIdentity {
            capability: I2C_IMPLEMENTATION,
            execution_profile: I2C_EXECUTION_PROFILE,
            artifact: id.artifact_id.as_str(),
            base: id.base_id.as_str(),
            base_implementation: "conduitos.base/i2c-controller@1",
            mechanism: I2C_ATTACHMENT_CLASS,
            authority: I2C_AUTHORITY,
            resource_class: I2C_ATTACHMENT_CLASS,
        };
        let node = match fore {
            Some(fore) => bind_selected_assigned_fore_operation(
                table,
                claim,
                definition,
                native,
                SelectedAssignedForeOperationPlan {
                    operation: selected,
                    fore,
                },
            ),
            None => bind_selected_assigned_operation(table, claim, definition, native, selected),
        }
        .map_err(|e| match e {
            SelectedCallRefusal::WrongBinding => I2cCallRefusal::WrongBinding,
            SelectedCallRefusal::Possession => I2cCallRefusal::Possession,
        })?;
        Ok((node, PreparedI2cBinding::new(contract, node)?))
    }
    /// Refusal retains the same initialized controller. All fallible identity,
    /// assignment and codec preparation precedes its one-time transfer to Play.
    /// The opaque capability table/handle must be issued locally by Root.
    pub fn bind_assigned_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &I2cContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<I2cHostCall<P>, I2cCallRefusal> {
        Ok(Self::prepare_assigned_retained(slot, table, handle, claim, contract, selected)?.bind())
    }

    /// All Fore, identity, possession and codec checks precede controller transfer.
    /// Refusal retains Root's same provider and starts no electrical operation.
    pub fn bind_assigned_fore_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &I2cContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<I2cHostCall<P>, I2cCallRefusal> {
        Ok(
            Self::prepare_assigned_fore_retained(slot, table, handle, claim, contract, selected)?
                .bind(),
        )
    }

    /// Prepare exact possession, assignment and codecs before Root reserves the
    /// complete workset. No controller withdrawal, bus operation or Start.
    pub fn prepare_assigned_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &I2cContract,
        selected: SelectedAssignedOperationPlan<'_>,
    ) -> Result<PreparedAssignedI2c<'a, P>, I2cCallRefusal> {
        Self::prepare_retained(slot, table, handle, claim, contract, selected, None)
    }

    /// Local Fore pins pass the same binder before retained native transfer.
    pub fn prepare_assigned_fore_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &I2cContract,
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<PreparedAssignedI2c<'a, P>, I2cCallRefusal> {
        Self::prepare_retained(
            slot,
            table,
            handle,
            claim,
            contract,
            selected.operation,
            Some(selected.fore),
        )
    }

    fn prepare_retained<'a>(
        slot: &'a mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &I2cContract,
        selected: SelectedAssignedOperationPlan<'_>,
        fore: Option<&[AssignedForeRequirement]>,
    ) -> Result<PreparedAssignedI2c<'a, P>, I2cCallRefusal> {
        let (node, prepared) = slot
            .as_ref()
            .ok_or(I2cCallRefusal::Possession)?
            .prepare_assigned(&table, &claim, contract, selected, fore)?;
        // Assignment identity cannot substitute for the original opaque bearer.
        // This non-effectful check happens before provider-slot custody moves.
        BoundBaseOperation::check(&table, &handle, &claim).map_err(I2cCallRefusal::Capability)?;
        Ok(PreparedAssignedI2c {
            prepared,
            table,
            handle,
            claim,
            slot,
            node,
        })
    }
}
