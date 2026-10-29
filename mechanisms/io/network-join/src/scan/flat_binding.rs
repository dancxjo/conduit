//! Borrowed exact binding: no owned Kind, Plan or discarded refusal bearer.
use super::*;
use conduit_plan_lowering::selected_operation::{
    bind_selected_flat_direct_operation_for_subject, SelectedFlatDirectOperationPlan,
    SelectedOperationContractRef,
};
impl<P: NetworkScanProvider> ReadyNetworkScanBase<P> {
    /// Bind complete immutable Core tables and current possession to this same
    /// initialized provider. Root separately admits backing before dispatch.
    /// Every refusal leaves the Ready slot unchanged and returns all original
    /// issued inputs; callers must retain/revoke them through normal custody.
    #[allow(
        clippy::result_large_err,
        reason = "Refusal retains affine actual bearer inputs without allocation"
    )]
    pub fn bind_flat_direct_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selected: SelectedFlatDirectOperationPlan<'_, 2, 1, 1, 2, 1, 2>,
    ) -> Result<NetworkScanHostCall<P>, ScanBindingFailure> {
        let checked = slot
            .as_ref()
            .ok_or(ScanCallRefusal::Unavailable)
            .and_then(|ready| ready.check_flat_binding(&table, &handle, &claim, selected));
        let node = match checked {
            Ok(node) => node,
            Err(e) => return Err((e, table, handle, claim)),
        };
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .unwrap_or_else(|_| unreachable!("same exclusive preflighted scan possession"));
        Ok(slot
            .take()
            .expect("same checked Ready slot")
            .into_checked_call(operation, node))
    }
    pub(super) fn check_flat_binding(
        &self,
        table: &BaseCapabilityTable,
        handle: &BaseCapabilityHandle,
        claim: &BaseOperationClaim,
        selected: SelectedFlatDirectOperationPlan<'_, 2, 1, 1, 2, 1, 2>,
    ) -> Result<NodeId, ScanCallRefusal> {
        self.check_claim(claim)?;
        Self::check_possession(table, claim)?;
        let view = selected.tables.preparation_scope().view();
        let mut gears = view
            .placements
            .iter()
            .filter(|g| g.placement_id == selected.placement_id);
        let gear = gears.next().ok_or(ScanCallRefusal::WrongBinding)?;
        if gears.next().is_some() {
            return Err(ScanCallRefusal::WrongBinding);
        }
        let canonical = PreparedCanonicalNetworkOffer::scan(
            SCAN_CAPABILITY,
            self.identity.implementation.as_str(),
            self.identity.artifact.as_str(),
        )
        .into_contract();
        self.check_native_ref(gear, canonical.kind().limits)?;
        let node = bind_selected_flat_direct_operation_for_subject(
            table,
            claim,
            SelectedOperationContractRef {
                kind: canonical.kind(),
                semantic_contract: canonical.semantic_contract(),
                call: NETWORK_SCAN_HOST_CALL,
                input_bytes: MAXIMUM_SCAN_INPUT_BYTES,
                output_bytes: MAXIMUM_SCAN_OUTPUT_BYTES,
                resource_bytes: 1,
            },
            NETWORK_SCAN_SUBJECT,
            &[],
            selected,
        )
        .map_err(ScanCallRefusal::Binding)?;
        BoundBaseOperation::check(table, handle, claim).map_err(ScanCallRefusal::Capability)?;
        Ok(node)
    }
    fn check_native_ref(
        &self,
        gear: PlannedGearRef<'_>,
        limits: &CapabilityLimits,
    ) -> Result<(), ScanCallRefusal> {
        let i = &self.identity;
        let base = gear.base.ok_or(ScanCallRefusal::WrongBinding)?;
        let mut resources = gear.resources.iter();
        let resource = resources.next().ok_or(ScanCallRefusal::WrongBinding)?;
        if resources.next().is_some()
            || resource.class_id != WIFI_STATION_RESOURCE_CLASS
            || resource.units != 1
            || gear.artifact_id != i.artifact.as_str()
            || gear.implementation_id != i.implementation.as_str()
            || gear.execution_profile_id != NETWORK_SCAN_EXECUTION_PROFILE
            || gear.capability_id != SCAN_CAPABILITY
            || gear.limits != limits
            || base.base_id != i.base.as_str()
            || base.provider_instance_id != i.instance.as_str()
            || base.provider_generation != i.provider_generation
            || base.implementation_id != i.base_implementation.as_str()
            || base.mechanism_family != SCAN_BASE_KIND
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(ScanCallRefusal::WrongBinding);
        }
        Ok(())
    }
}
