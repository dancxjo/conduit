use super::*;
impl ScopedNamespace {
    pub(super) fn validate_admission(
        &self,
        request: &CapabilityIssueRequest,
        fragment: &PlanFragment,
        placement: &PlacementId,
    ) -> Result<(), NamespaceRefusal> {
        let scope = &request.scope;
        let selected = fragment
            .placements
            .iter()
            .find(|item| item.placement_id == *placement)
            .ok_or(NamespaceRefusal::WrongSelection)?;
        let call = selected
            .host_calls
            .iter()
            .find(|call| call.contract_id == scope.operation_contract_id)
            .ok_or(NamespaceRefusal::UnsupportedOperation)?;
        let granted = selected.authority.iter().any(|binding| {
            binding.grant_id == scope.authority_grant_id
                && binding.contract_id == scope.authority_contract_id
                && binding.host_call_contract_id == scope.operation_contract_id
                && binding.subject_kind == scope.subject_kind
                && binding.host_id == scope.host_id
                && binding.boot_id == scope.boot_id
                && binding.capability_id == scope.capability_id
        });
        if !verify_plan_fragment(fragment)
            || fragment.plan_id != scope.plan_id
            || fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || selected.host_id != self.host
            || selected.boot_id != self.boot
            || selected.base.as_ref() != Some(&self.binding)
            || selected.capability_id != scope.capability_id
            || selected.implementation_id != scope.implementation_id
            || !granted
            || !selected
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.root)
            || scope.maximum_parameter_bytes > call.maximum_input_bytes
            || scope.maximum_result_bytes > call.maximum_output_bytes
            || scope.maximum_in_flight > call.maximum_in_flight
            || scope.maximum_operations > self.limits.maximum_operations
            || scope.maximum_work_units
                > u64::from(
                    self.limits
                        .maximum_read_bytes
                        .max(self.limits.maximum_name_bytes),
                )
        {
            return Err(NamespaceRefusal::WrongSelection);
        }
        if scope.resource_pool_id != self.root || scope.resource_generation_id != self.generation {
            return Err(NamespaceRefusal::WrongRoot);
        }
        match scope.operation_contract_id.as_str() {
            LIST_CALL | OPEN_CALL => Ok(()),
            _ => Err(NamespaceRefusal::UnsupportedOperation),
        }
    }
}
