//! Explicit CPU Root authority fixture; the production owner services requests.
use conduit_core as c;
use conduit_tm1637_wire::{installation::*, owner::*, *};
use std::{cell::Cell, rc::Rc};
pub struct Pins(pub Rc<Cell<usize>>);
impl OpenDrain for Pins {
    type Error = core::convert::Infallible;
    fn clock(&mut self, _: bool) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
    fn data(&mut self, _: bool) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
pub fn bind(
    prepared: &conduit_environment_make::PreparedDisplayOutput,
    pins: Pins,
) -> WriteHostCall<Pins> {
    let fragment = &prepared.plan().fragments[0];
    let gear = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == WRITE_KIND)
        .unwrap();
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let scope = c::BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: "fixture/provider".into(),
        base_provider_generation: 1,
        plan_id: fragment.plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        authority_grant_id: "fixture/display".into(),
        authority_contract_id: WRITE_AUTHORITY.into(),
        capability_id: WRITE_CAPABILITY.into(),
        implementation_id: "fixture/driver".into(),
        operation_contract_id: WRITE_CALL.into(),
        subject_kind: c::kind_id(WRITE_KIND),
        resource_pool_id: "fixture/pins".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/generation".into()),
        envelope_id: "fixture/envelope".into(),
        maximum_parameter_bytes: MAXIMUM_BYTES,
        maximum_result_bytes: 0,
        maximum_work_units: 8,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let authority = c::BaseCapabilityAuthority {
        grant: c::AuthorityGrant {
            grant_id: scope.authority_grant_id.clone(),
            contract_id: scope.authority_contract_id.clone(),
            host_call_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: scope.capability_id.clone(),
        },
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: scope.maximum_parameter_bytes,
        maximum_result_bytes: scope.maximum_result_bytes,
        maximum_work_units: scope.maximum_work_units,
        maximum_in_flight: scope.maximum_in_flight,
        maximum_operations: scope.maximum_operations,
    };
    let claim = c::BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = c::BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(c::CapabilityIssueRequest { scope, authority })
        .unwrap();
    // SAFETY: the fixture exclusively owns Pins and explicitly issues current
    // Plan/Play authority above. This proves no physical pins or Root admission.
    unsafe {
        WriteHostCall::bind_selected(
            table,
            handle,
            claim,
            WriteAttachment {
                generation: 1,
                resource_bytes: 64,
            },
            pins,
            &WriteContract::prepare().unwrap(),
            conduit_plan_lowering::selected_operation::SelectedOperationPlan {
                fragment,
                lowered: prepared.lowered(),
                active: &active,
                placement_id: &gear.placement_id,
            },
        )
        .unwrap()
    }
}
