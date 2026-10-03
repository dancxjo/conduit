//! Cooperative admission/lifetime fixtures; no native DMA or controller proof.
use super::*;
use alloc::vec;
use conduit_core::*;
use conduit_plan_lowering::lowering::{LoweredPlanFragment, lower_plan_fragment};

pub(super) fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/usb/control/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/visible"),
        authority_contract_id: AuthorityContractId::from("authority/usb-control@1"),
        capability_id: CapabilityId::from("machine/usb/control/read-write"),
        implementation_id: ImplementationId::from("implementation/usb-control/fixture@1"),
        operation_contract_id: HostCallContractId::from("conduit.host/usb-control@1"),
        subject_kind: KindId::from("machine/usb/control"),
        resource_pool_id: ResourcePoolId::from("usb/control/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("usb/control/controller/window-8"),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 8,
        maximum_in_flight: 1,
        maximum_operations: 100000,
    }
}

fn authority() -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: AuthorityGrantId::from("grant/visible"),
            contract_id: AuthorityContractId::from("authority/usb-control@1"),
            host_call_contract_id: HostCallContractId::from("conduit.host/usb-control@1"),
            subject_kind: KindId::from("machine/usb/control"),
            host_id: HostId::from("host/one"),
            boot_id: BootId::from("boot/one"),
            capability_id: CapabilityId::from("machine/usb/control/read-write"),
        },
        base_instance_id: BaseInstanceId::from("base/usb/control/instance"),
        base_provider_generation: 4,
        resource_pool_id: ResourcePoolId::from("usb/control/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        operation_contract_id: HostCallContractId::from("conduit.host/usb-control@1"),
        envelope_id: CapabilityEnvelopeId::from("usb/control/controller/window-8"),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 32,
        maximum_in_flight: 1,
        maximum_operations: 100000,
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
        BaseInstanceId::from("base/usb/control/instance"),
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
    let scope = scope();
    let contract = ControlContract::prepare().unwrap();
    let kind = contract.kind();
    let placement = PlacementId::from("placement/usb-control");
    let gear = conduit_core::planned_gear_from_parts! {
        placement_id: placement.clone(),
        gear_id: GearId::from("gear/usb-control"),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        execution_profile_id: ExecutionProfileId::from("conduitos/cooperative-bounded-step@1"),
        configuration: vec![],
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        capability_id: scope.capability_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        artifact_id: ArtifactId::from("artifact/usb-control/fixture"),
        base: Some(BaseProviderBinding {
            base_id: HostBaseId::from("base/usb-control"),
            provider_instance_id: scope.base_instance_id.clone(),
            provider_generation: scope.base_provider_generation,
            implementation_id: BaseImplementationId::from("base/usb-control/fixture@1"),
            mechanism_family: HostBaseKindId::from("machine/usb/control-attachment"),
            enforcement_class: BaseEnforcementClass::Cooperative,
        }),
        realization_characteristics: vec![],
        limits: kind.limits.clone(),
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
        semantic_contract: kind.semantic_contract(),
        terminal_transductions: vec![],
        host_calls: vec![HostCallRequirement {
            contract_id: HostCallContractId::from(CONTROL_CALL),
            target_kind: Some(kind.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: CONTROL_MAXIMUM_BYTES,
            maximum_output_bytes: CONTROL_MAXIMUM_BYTES,
        }],
        resources: vec![ResourceBinding {
            pool_id: scope.resource_pool_id.clone(),
            class_id: ResourceClassId::from("machine/usb/control-attachment"),
            units: 1,
            protected: None,
            compute: None,
            content: None,
        }],
        authority: vec![AuthorityBinding {
            grant_id: scope.authority_grant_id.clone(),
            contract_id: scope.authority_contract_id.clone(),
            host_call_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: scope.capability_id.clone(),
        }],
        pool_references: vec![],
    };
    let mut fragment = PlanFragment {
        plan_id: scope.plan_id.clone(),
        fragment_id: FragmentId::from("fragment/usb-control"),
        source_document_id: SourceDocumentId::from("source/usb-control/fixture"),
        checked_plot_id: CheckedPlotId::from("checked/usb-control/fixture"),
        expanded_plot_id: ExpandedPlotId::from("expanded/usb-control/fixture"),
        completion_policy: Default::default(),
        realization_backs: vec![],
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        placements: vec![gear],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![],
        connections: vec![],
        fore_ports: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![placement.clone()],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![],
        expected_sign: vec![],
        sign_storage_budget: SignStorageBudget {
            item_capacity: 64,
            byte_capacity: 8192,
        },
        plan_fragments: vec![],
    };
    fragment.expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlacementPrepared(placement.clone()),
        ExpectedSign::PlacementTerminal(placement.clone()),
        ExpectedSign::PlanTerminal,
    ];
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    (fragment, lowered, active, placement)
}

pub(super) fn owner(key: u8) -> ControlCallOwner {
    let (fragment, lowered, active, placement) = selected();
    bind(
        SelectedOperationPlan {
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement_id: &placement,
        },
        key,
    )
    .unwrap()
}

pub(super) fn bind(
    selection: SelectedOperationPlan<'_>,
    key: u8,
) -> Result<ControlCallOwner, ControlOwnerRefusal> {
    let SelectedOperationPlan {
        fragment,
        lowered,
        active,
        placement_id: placement,
    } = selection;
    let mut table = table(key);
    let mut request = request();
    request.scope.plan_id = fragment.plan_id.clone();
    request.scope.active_play_id = active.active_play_id.clone();
    let handle = table.issue(request).unwrap();
    let mut claim = claim();
    claim.plan_id = fragment.plan_id.clone();
    claim.active_play_id = active.active_play_id.clone();
    unsafe {
        ControlCallOwner::bind_admitted(
            table,
            handle,
            claim,
            ControlAttachment {
                slot: 1,
                generation: 7,
                maximum_data_bytes: 256,
                resource_bytes: 4096,
            },
            &ControlContract::prepare().unwrap(),
            SelectedOperationPlan {
                fragment,
                lowered,
                active,
                placement_id: placement,
            },
        )
    }
}

pub(super) fn input() -> alloc::vec::Vec<u8> {
    let contract = ControlContract::prepare().unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("record")
    };
    let field = |name| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "setup",
                StructuredInfoValue::leaf(field("setup"), [128, 0, 0, 0, 0, 0, 8, 0].to_vec())
                    .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "output",
                StructuredInfoValue::sequence(field("output"), vec![]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
