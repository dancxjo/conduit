use super::*;
use conduit_plan_lowering::{selected_operation::SelectedOperationPlan, selected_operation_set::*};
pub struct PreparedRecordService<'a, S> {
    slot: &'a mut Option<ReadyRecordService<S>>,
    operations: [c::BoundBaseOperation; 2],
    selected: SelectedOperationSet,
}
impl<S: NorStorage> PreparedRecordService<'_, S> {
    pub fn operation_backing_bytes(&self) -> Option<usize> {
        self.operations.iter().try_fold(0usize, |total, op| {
            total.checked_add(op.retained_backing_bytes()?)
        })
    }
    pub fn bind(self) -> RecordServiceHostCall<S> {
        RecordServiceHostCall {
            ready: self.slot.take().expect("exclusive prepared service owner"),
            operations: self.operations,
            routes: [
                self.selected.routes[0].expect("checked append route"),
                self.selected.routes[1].expect("checked read route"),
            ],
            node: self.selected.node,
            next_request: 0,
            cached: None,
            input: [0; 200],
            custody: None,
            append_failed: false,
            terminal: false,
        }
    }
}
impl<S: NorStorage> ReadyRecordService<S> {
    /// Rich checked Plan only. Operations and physical owner stay affine on refusal.
    #[allow(clippy::result_large_err)]
    pub fn prepare_selected_retained<'a>(
        slot: &'a mut Option<Self>,
        operations: [c::BoundBaseOperation; 2],
        resource: &c::ResourceBinding,
        selection: SelectedOperationPlan<'_>,
        owner: &c::ResourceAdmissionOwner,
    ) -> Result<PreparedRecordService<'a, S>, ([c::BoundBaseOperation; 2], ServiceRefusal)> {
        let check = || {
            let ready = slot.as_ref().ok_or(ServiceRefusal::Possession)?;
            let identity = &ready.identity;
            let gear = selection
                .fragment
                .placements
                .iter()
                .find(|g| &g.placement_id == selection.placement_id)
                .ok_or(ServiceRefusal::WrongBinding)?;
            let base = gear.base.as_ref().ok_or(ServiceRefusal::WrongBinding)?;
            if gear.host_id != identity.host_id
                || gear.boot_id != identity.boot_id
                || gear.artifact_id != identity.artifact_id
                || gear.implementation_id != identity.implementation_id
                || gear.execution_profile_id.as_str() != SERVICE_PROFILE
                || gear.capability_id.as_str() != SERVICE_CAPABILITY
                || base.base_id != identity.base_id
                || base.provider_instance_id != identity.provider_instance_id
                || base.provider_generation != identity.provider_generation
                || base.implementation_id != identity.base_implementation_id
                || base.mechanism_family.as_str() != crate::native_append::APPEND_MECHANISM
                || resource.pool_id != identity.resource_pool_id
                || resource.class_id.as_str() != crate::native_append::APPEND_CLASS
                || resource.units != 1
            {
                return Err(ServiceRefusal::WrongBinding);
            }
            for (operation, grant) in operations.iter().zip(&ready.originals) {
                let scope = &operation.inspection().scope;
                if scope.authority_grant_id != grant.grant_id
                    || scope.authority_contract_id != grant.contract_id
                    || scope.operation_contract_id != grant.host_call_contract_id
                    || scope.subject_kind != grant.subject_kind
                    || scope.capability_id != grant.capability_id
                {
                    return Err(ServiceRefusal::Possession);
                }
            }
            let kind = crate::service_contract::definition();
            let requirement = c::resource_requirement(crate::native_append::APPEND_CLASS, 1);
            let append_contract =
                c::HostCallContractId::from(crate::native_append::APPEND_OPERATION);
            let read_contract = c::HostCallContractId::from(READ_OPERATION);
            let append_kind = c::kind_id(crate::append_contract::APPEND_KIND);
            let read_kind = c::kind_id(crate::read_contract::READ_KIND);
            let append_in = c::port_id(crate::service_contract::APPEND_INPUT);
            let append_out = c::port_id(crate::service_contract::APPEND_OUTPUT);
            let read_in = c::port_id(crate::service_contract::READ_INPUT);
            let read_out = c::port_id(crate::service_contract::READ_OUTPUT);
            let calls = [
                SelectedServiceCall {
                    contract: &append_contract,
                    target_kind: &append_kind,
                    input_port: &append_in,
                    output_port: &append_out,
                    input_bytes: 200,
                    output_bytes: crate::append_result::APPEND_RESULT_BYTES,
                    work_units: u64::from(ready.provider.maximum_append_media_calls()),
                    maximum_operations: ready.limits.append_operations,
                },
                SelectedServiceCall {
                    contract: &read_contract,
                    target_kind: &read_kind,
                    input_port: &read_in,
                    output_port: &read_out,
                    input_bytes: crate::read_contract::READ_INPUT_BYTES,
                    output_bytes: crate::read_result::READ_RESULT_BYTES as u32,
                    work_units: crate::read_contract::MAXIMUM_MEDIA_READS_PER_REQUEST,
                    maximum_operations: ready.limits.read_operations,
                },
            ];
            bind_selected_operation_set(
                &[&operations[0], &operations[1]],
                SelectedServiceContract {
                    kind: &kind,
                    configuration: &[],
                    resource,
                    requirement: &requirement,
                    resource_generation: &identity.resource_generation_id,
                    envelope: &identity.envelope_id,
                    calls: &calls,
                },
                selection,
                owner,
            )
            .map_err(|_| ServiceRefusal::WrongBinding)
        };
        match check() {
            Ok(selected) => Ok(PreparedRecordService {
                slot,
                operations,
                selected,
            }),
            Err(error) => Err((operations, error)),
        }
    }
}
