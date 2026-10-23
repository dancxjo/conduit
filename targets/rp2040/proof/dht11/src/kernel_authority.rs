//! Local bench Root policy: four pulse operations on this fresh Plan and Boot.
use conduit_core::*;
use conduit_pulse_capture::{installation::*, *};
pub fn possession(
    identity: &PulseNativeIdentity,
    plan_id: &PlanId,
    active: &ActivePlayIdentity,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
    let mut rng = embassy_rp::clocks::RoscRng;
    let mut issuer_key = [0; 32];
    for chunk in issuer_key.as_chunks_mut::<8>().0 {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    let grant = AuthorityGrant {
        grant_id: "fixture/pulse-grant".into(),
        contract_id: PULSE_AUTHORITY.into(),
        host_call_contract_id: PULSE_READ_CALL.into(),
        subject_kind: PULSE_READ_KIND.into(),
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        capability_id: PULSE_CAPABILITY.into(),
    };
    let scope = BaseCapabilityScope {
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        authority_grant_id: grant.grant_id.clone(),
        authority_contract_id: PULSE_AUTHORITY.into(),
        capability_id: PULSE_CAPABILITY.into(),
        implementation_id: identity.implementation_id.clone(),
        operation_contract_id: PULSE_READ_CALL.into(),
        subject_kind: PULSE_READ_KIND.into(),
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        maximum_parameter_bytes: 1024,
        maximum_result_bytes: 1024,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: if cfg!(feature = "http-ap") { 4096 } else { 4 },
    };
    let authority = BaseCapabilityAuthority {
        grant,
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: 1024,
        maximum_result_bytes: 1024,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: if cfg!(feature = "http-ap") { 4096 } else { 4 },
    };
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: 1024,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        identity.host_id.clone(),
        identity.boot_id.clone(),
        identity.provider_instance_id.clone(),
        1,
        issuer_key,
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    (table, handle, claim)
}
