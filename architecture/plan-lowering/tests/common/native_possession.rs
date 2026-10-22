//! CPU-only opaque capability issuance fixture; never an initialized device.
use conduit_core::*;
pub fn scope(subject: &str) -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: "host/network".into(),
        boot_id: "boot/current".into(),
        base_instance_id: "base/radio".into(),
        base_provider_generation: 1,
        plan_id: "plan/unsealed".into(),
        active_play_id: "play/unsealed".into(),
        authority_grant_id: "grant/configurator".into(),
        authority_contract_id: "authority/network-config@1".into(),
        capability_id: "cap/join".into(),
        implementation_id: "impl/join".into(),
        operation_contract_id: "call/join".into(),
        subject_kind: subject.into(),
        resource_pool_id: "pool/radio".into(),
        resource_generation_id: ResourceGenerationId("generation/current".into()),
        envelope_id: "envelope/radio".into(),
        maximum_parameter_bytes: 256,
        maximum_result_bytes: 256,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    }
}
pub fn possession(
    scope: BaseCapabilityScope,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
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
        parameter_bytes: scope.maximum_parameter_bytes,
        work_units: 1,
    };
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
        base_provider_generation: 1,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: scope.maximum_parameter_bytes,
        maximum_result_bytes: scope.maximum_result_bytes,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    (table, handle, claim)
}
