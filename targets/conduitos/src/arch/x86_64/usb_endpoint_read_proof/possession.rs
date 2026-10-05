//! Explicit proof-root issuance; observation never creates a grant.
use super::*;
pub(in crate::arch::x86_64::usb) fn issue(
    plan: &Plan,
) -> Result<
    (
        BaseCapabilityTable,
        BaseCapabilityHandle,
        BaseOperationClaim,
    ),
    &'static str,
> {
    let fragment = &plan.fragments[0];
    let mut selected = fragment
        .placements
        .iter()
        .filter(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION);
    let gear = selected
        .next()
        .ok_or("usb-endpoint-read-proof-possession")?;
    if selected.next().is_some() {
        return Err("usb-endpoint-read-proof-possession");
    }
    let base = gear
        .base
        .as_ref()
        .ok_or("usb-endpoint-read-proof-possession")?;
    let grant = &gear.authority[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let scope = BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: base.provider_instance_id.clone(),
        base_provider_generation: base.provider_generation,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        authority_grant_id: grant.grant_id.clone(),
        authority_contract_id: grant.contract_id.clone(),
        capability_id: gear.capability_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        operation_contract_id: ENDPOINT_READ_CALL.into(),
        subject_kind: gear.kind_id.clone(),
        resource_pool_id: gear.resources[0].pool_id.clone(),
        resource_generation_id: ResourceGenerationId("conduitos.proof/usb-dma-generation@1".into()),
        envelope_id: "conduitos.proof/usb-dma-envelope@1".into(),
        maximum_parameter_bytes: ENDPOINT_READ_MAXIMUM_BYTES,
        maximum_result_bytes: ENDPOINT_READ_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: u32::from(planning::ENDPOINT_READ_PROOF_TRANSFERS),
    };
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: grant.grant_id.clone(),
            contract_id: grant.contract_id.clone(),
            host_call_contract_id: grant.host_call_contract_id.clone(),
            subject_kind: grant.subject_kind.clone(),
            host_id: grant.host_id.clone(),
            boot_id: grant.boot_id.clone(),
            capability_id: grant.capability_id.clone(),
        },
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: ENDPOINT_READ_MAXIMUM_BYTES,
        maximum_result_bytes: ENDPOINT_READ_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: u32::from(planning::ENDPOINT_READ_PROOF_TRANSFERS),
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
        parameter_bytes: ENDPOINT_READ_MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [71; 32],
        1,
    )
    .map_err(|_| "usb-endpoint-read-proof-possession")?;
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .map_err(|_| "usb-endpoint-read-proof-possession")?;
    Ok((table, handle, claim))
}
