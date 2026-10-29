use super::*;
use conduit_core::*;
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/spi/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/spi"),
        authority_contract_id: AuthorityContractId::from("authority/spi@1"),
        capability_id: CapabilityId::from("machine/spi/read-write"),
        implementation_id: ImplementationId::from("implementation/spi/fixture@1"),
        operation_contract_id: HostCallContractId::from(SPI_CALL),
        subject_kind: KindId::from(super::super::super::contract::SPI_KIND),
        resource_pool_id: ResourcePoolId::from("spi/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("spi/window"),
        maximum_parameter_bytes: SPI_MAXIMUM_BYTES,
        maximum_result_bytes: SPI_MAXIMUM_BYTES,
        maximum_work_units: 8,
        maximum_in_flight: 1,
        maximum_operations: 2,
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
        SpiContract::prepare().unwrap().kind(),
        SPI_CALL,
        SPI_MAXIMUM_BYTES,
        "spi",
        "spi",
    )
}

pub(super) fn bind(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    active: &ActivePlayIdentity,
    placement: &PlacementId,
) -> Result<SpiHostCall<Provider>, SpiCallRefusal> {
    let mut scope = scope();
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
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
        parameter_bytes: SPI_MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        4,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    unsafe {
        SpiHostCall::bind_selected(
            table,
            handle,
            claim,
            SpiAttachment {
                generation: 7,
                resource_bytes: 4096,
            },
            Provider::default(),
            SpiCallSelection {
                contract: &SpiContract::prepare().unwrap(),
                fragment,
                lowered,
                active,
                placement,
            },
        )
    }
}

pub(super) fn owner() -> SpiHostCall<Provider> {
    let (fragment, lowered, active, placement) = selected();
    bind(&fragment, &lowered, &active, &placement).unwrap()
}
