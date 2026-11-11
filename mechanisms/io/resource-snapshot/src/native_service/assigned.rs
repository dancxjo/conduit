//! Assigned preparation checks routing/possession, never reserves resources.
use super::*;
use conduit_plan_lowering::{selected_assigned_operation_set::*, selected_operation_set::*};

pub struct PreparedAssignedRecordService<'a, S> {
    prepared: PreparedRecordService<'a, S>,
    resource: &'a c::ResourceBinding,
    resource_index: u16,
}
impl<S: NorStorage> PreparedAssignedRecordService<'_, S> {
    pub fn node(&self) -> NodeId {
        self.prepared.node()
    }
    pub fn routes(
        &self,
    ) -> [conduit_plan_lowering::selected_operation_set::SelectedServiceRoute; 2] {
        self.prepared.routes()
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.prepared.retained_backing_bytes()
    }
    /// Prepare the actual two-call Back without withdrawing the Ready owner.
    pub fn prepare_back<const PORTS: usize, const READ_BYTES: usize>(
        &self,
    ) -> Result<
        crate::native_append::service_back::RecordServiceBack<PORTS, READ_BYTES>,
        ServiceRefusal,
    > {
        self.prepared.prepare_back()
    }
    pub fn resource_binding(&self) -> &c::ResourceBinding {
        self.resource
    }
    pub fn resource_index(&self) -> u16 {
        self.resource_index
    }
    pub fn operation_backing_bytes(&self) -> Option<usize> {
        self.prepared.operation_backing_bytes()
    }
    /// Transfer only after actual live admission of this exact resource binding.
    /// # Safety
    /// Root must have completed its ordinary Start `check_resources` against the
    /// current delegated fragment, including this independently retained binding
    /// and numeric resource index, with one actual exclusive residence reservation.
    /// Assigned allow-lists/record correspondence prove NONE of that admission.
    /// The caller must preserve the original ready-owner/media-window obligations
    /// and must not run this alongside the retired/old Play or resource owner.
    pub unsafe fn bind_after_resource_admission(self) -> RecordServiceHostCall<S> {
        self.prepared.bind()
    }
}
impl<S: NorStorage> ReadyRecordService<S> {
    /// Takes no physical owner until unsafe post-admission transfer. Every refusal
    /// returns both opaque operations intact; Ready remains in its original slot.
    #[allow(clippy::result_large_err)]
    pub fn prepare_assigned_retained<'a>(
        slot: &'a mut Option<Self>,
        operations: [c::BoundBaseOperation; 2],
        resource: &'a c::ResourceBinding,
        selected: SelectedAssignedServicePlan<'_>,
    ) -> Result<PreparedAssignedRecordService<'a, S>, ([c::BoundBaseOperation; 2], ServiceRefusal)>
    {
        let resource_index = selected.resource;
        let check = || {
            let ready = slot.as_ref().ok_or(ServiceRefusal::Possession)?;
            ready.check_original_operations(&operations)?;
            let identity = &ready.identity;
            if resource.pool_id != identity.resource_pool_id
                || resource.class_id.as_str() != crate::native_append::APPEND_CLASS
                || resource.units != 1
            {
                return Err(ServiceRefusal::WrongBinding);
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
            check_selected_assigned_operation_set(
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
                AssignedServiceNativeIdentity {
                    capability: SERVICE_CAPABILITY,
                    execution_profile: SERVICE_PROFILE,
                    artifact: identity.artifact_id.as_str(),
                    base: identity.base_id.as_str(),
                    base_implementation: identity.base_implementation_id.as_str(),
                    mechanism: crate::native_append::APPEND_MECHANISM,
                    resource_class: crate::native_append::APPEND_CLASS,
                    authorities: &[crate::native_append::APPEND_AUTHORITY, READ_AUTHORITY],
                },
                selected,
            )
            .map_err(|_| ServiceRefusal::WrongBinding)
        };
        match check() {
            Ok(selected) => Ok(PreparedAssignedRecordService {
                resource,
                resource_index,
                prepared: PreparedRecordService::from_checked(slot, operations, selected),
            }),
            Err(error) => Err((operations, error)),
        }
    }
}
#[cfg(test)]
#[path = "tests/assigned.rs"]
mod tests;
