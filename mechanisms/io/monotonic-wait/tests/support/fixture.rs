use super::*;
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/wait/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/wait"),
        authority_contract_id: AuthorityContractId::from("authority/wait@1"),
        capability_id: CapabilityId::from("machine/wait/read-write"),
        implementation_id: ImplementationId::from("implementation/wait/fixture@1"),
        operation_contract_id: HostCallContractId::from(WAIT_HOST_CALL_CONTRACT),
        subject_kind: KindId::from(conduit_time::TICK_KIND),
        resource_pool_id: ResourcePoolId::from("wait/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("wait/window"),
        maximum_parameter_bytes: PARAMETER_BYTES,
        maximum_result_bytes: 0,
        maximum_work_units: 8,
        maximum_in_flight: 1,
        maximum_operations: 100000,
    }
}

pub(super) fn selected() -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    let (mut fragment, _, _, placement) = super::selection_fixture::selected(
        &scope(),
        &conduit_time::tick_semantic_contract(),
        WAIT_HOST_CALL_CONTRACT,
        PARAMETER_BYTES,
        "wait",
        "wait",
    );
    fragment.placements[0].host_calls[0].maximum_output_bytes = 0;
    fragment.placements[0].configuration = configuration();
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    (fragment, lowered, active, placement)
}

pub(super) fn bind(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    active: &ActivePlayIdentity,
    placement: &PlacementId,
    provider: Provider,
) -> Result<MonotonicWaitHostCall<Provider>, WaitCallRefusal> {
    let (table, handle, claim) = authorization(scope(), fragment, active);
    unsafe {
        MonotonicWaitHostCall::bind_selected(
            table,
            handle,
            claim,
            WaitAttachment {
                generation: 7,
                resource_bytes: 4096,
                maximum_duration_ms: 1000,
            },
            provider,
            &configuration(),
            SelectedOperationPlan {
                fragment,
                lowered,
                active,
                placement_id: placement,
            },
        )
    }
}

pub(super) fn owner(provider: Provider) -> (MonotonicWaitHostCall<Provider>, NodeId) {
    let (fragment, lowered, active, placement) = selected();
    let node = lowered.nodes[0].node;
    (
        bind(&fragment, &lowered, &active, &placement, provider).unwrap(),
        node,
    )
}

pub(super) fn configuration() -> Vec<ConfigurationEntry> {
    vec![
        ConfigurationEntry {
            key: "count".into(),
            value: ConfigurationValue::U64(3),
        },
        ConfigurationEntry {
            key: "period-ms".into(),
            value: ConfigurationValue::U64(250),
        },
    ]
}

pub(super) fn authorization(
    mut scope: BaseCapabilityScope,
    fragment: &PlanFragment,
    active: &ActivePlayIdentity,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
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
        parameter_bytes: PARAMETER_BYTES,
        work_units: 1,
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
