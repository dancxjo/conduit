//! Cooperative admission/lifetime fixtures; no native transport or service proof.
use super::*;
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/network/ethernet-frame/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/visible"),
        authority_contract_id: AuthorityContractId::from("conduitos.authority/ethernet-frame@1"),
        capability_id: CapabilityId::from("network/ethernet-frame/receive"),
        implementation_id: ImplementationId::from("implementation/ethernet-frame/fixture@1"),
        operation_contract_id: HostCallContractId::from("conduit.host/ethernet-frame@1"),
        subject_kind: KindId::from("network/ethernet-frame"),
        resource_pool_id: ResourcePoolId::from("network/ethernet-frame/queues"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("network/ethernet-frame/queues/window-8"),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 8,
        maximum_in_flight: 1,
        maximum_operations: 4096,
    }
}

fn authority() -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: AuthorityGrantId::from("grant/visible"),
            contract_id: AuthorityContractId::from("conduitos.authority/ethernet-frame@1"),
            host_call_contract_id: HostCallContractId::from("conduit.host/ethernet-frame@1"),
            subject_kind: KindId::from("network/ethernet-frame"),
            host_id: HostId::from("host/one"),
            boot_id: BootId::from("boot/one"),
            capability_id: CapabilityId::from("network/ethernet-frame/receive"),
        },
        base_instance_id: BaseInstanceId::from("base/network/ethernet-frame/instance"),
        base_provider_generation: 4,
        resource_pool_id: ResourcePoolId::from("network/ethernet-frame/queues"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        operation_contract_id: HostCallContractId::from("conduit.host/ethernet-frame@1"),
        envelope_id: CapabilityEnvelopeId::from("network/ethernet-frame/queues/window-8"),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 32,
        maximum_in_flight: 1,
        maximum_operations: 4096,
    }
}

pub(super) fn request() -> CapabilityIssueRequest {
    CapabilityIssueRequest {
        scope: scope(),
        authority: authority(),
    }
}

pub(super) fn table(key: u8) -> BaseCapabilityTable {
    BaseCapabilityTable::new(
        HostId::from("host/one"),
        BootId::from("boot/one"),
        BaseInstanceId::from("base/network/ethernet-frame/instance"),
        4,
        [key; 32],
        4,
    )
    .unwrap()
}

pub(super) fn claim() -> BaseOperationClaim {
    let scope = scope();
    BaseOperationClaim {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_instance_id: scope.base_instance_id,
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id,
        active_play_id: scope.active_play_id,
        implementation_id: scope.implementation_id,
        operation_contract_id: scope.operation_contract_id,
        subject_kind: scope.subject_kind,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        parameter_bytes: 4096,
        work_units: 1,
    }
}

pub(super) fn selected() -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    crate::machine_membrane::selection_fixture::selected(
        &scope(),
        EthernetFrameContract::prepare().unwrap().kind(),
        FRAME_CALL,
        FRAME_MAXIMUM_BYTES,
        "network/ethernet-frame",
        "ethernet-frame",
    )
}
