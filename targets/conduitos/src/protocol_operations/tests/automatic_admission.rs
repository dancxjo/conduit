//! Exact fixture possession for the two retained automatic-protocol owners.
use super::*;

pub(super) fn possession(
    plan: &Plan,
    clock: bool,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
    let fragment = &plan.fragments[0];
    let (name, implementation, authority, operation, work) = if clock {
        (
            "clock",
            crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION,
            crate::monotonic_clock::installation::CLOCK_AUTHORITY,
            crate::monotonic_clock::contract::CLOCK_CALL,
            crate::monotonic_clock::owner::MAXIMUM_POLL_STEPS,
        )
    } else {
        (
            "bus",
            crate::i2c_base::installation::I2C_IMPLEMENTATION,
            crate::i2c_base::installation::I2C_AUTHORITY,
            crate::i2c_base::contract::I2C_CALL,
            1,
        )
    };
    let gear = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == implementation)
        .unwrap();
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let bytes = if clock {
        crate::monotonic_clock::contract::CLOCK_MAXIMUM_BYTES
    } else {
        crate::i2c_base::contract::I2C_MAXIMUM_BYTES
    };
    let scope = BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: alloc::format!("fixture/{name}-provider").into(),
        base_provider_generation: 1,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id,
        authority_grant_id: gear.authority[0].grant_id.clone(),
        authority_contract_id: authority.into(),
        capability_id: gear.capability_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        operation_contract_id: operation.into(),
        subject_kind: gear.kind_id.clone(),
        resource_pool_id: alloc::format!("fixture/{name}-resource").into(),
        resource_generation_id: ResourceGenerationId(alloc::format!("fixture/{name}-generation")),
        envelope_id: alloc::format!("fixture/{name}-envelope").into(),
        maximum_parameter_bytes: bytes,
        maximum_result_bytes: bytes,
        maximum_work_units: work,
        maximum_in_flight: 1,
        maximum_operations: 256,
    };
    // Test issuer only; production admission must supply opaque possession.
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
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
        maximum_in_flight: 1,
        maximum_operations: scope.maximum_operations,
    };
    let claim = BaseOperationClaim {
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
        parameter_bytes: bytes,
        work_units: work,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    (table, handle, claim)
}
