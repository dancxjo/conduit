//! Shared exact-selection fixture for cooperative Host Call contract tests.
use alloc::{format, vec};
use conduit_core::*;
use conduit_plan_lowering::lowering::{LoweredPlanFragment, lower_plan_fragment};

pub(crate) fn selected(
    scope: &BaseCapabilityScope,
    kind: &Kind,
    call: &str,
    maximum_bytes: u32,
    base: &str,
    label: &str,
) -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    let placement = PlacementId::from(format!("placement/{label}"));
    let gear = conduit_core::planned_gear_from_parts! {
        placement_id: placement.clone(),
        gear_id: GearId::from(format!("gear/{label}")),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        execution_profile_id: ExecutionProfileId::from("conduitos/cooperative-bounded-step@1"),
        configuration: vec![],
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        capability_id: scope.capability_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        artifact_id: ArtifactId::from(format!("artifact/{label}/fixture")),
        base: Some(BaseProviderBinding {
            base_id: HostBaseId::from(format!("base/{label}")),
            provider_instance_id: scope.base_instance_id.clone(),
            provider_generation: scope.base_provider_generation,
            implementation_id: BaseImplementationId::from(format!("base/{label}/fixture@1")),
            mechanism_family: HostBaseKindId::from(format!("machine/{base}-attachment")),
            enforcement_class: BaseEnforcementClass::Cooperative,
        }),
        realization_characteristics: vec![],
        limits: kind.limits.clone(),
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
        semantic_contract: kind.semantic_contract(),
        terminal_transductions: vec![],
        host_calls: vec![HostCallRequirement {
            contract_id: HostCallContractId::from(call),
            target_kind: Some(kind.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: maximum_bytes,
            maximum_output_bytes: maximum_bytes,
        }],
        resources: vec![ResourceBinding {
            pool_id: scope.resource_pool_id.clone(),
            class_id: ResourceClassId::from(format!("machine/{base}-attachment")),
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
        fragment_id: FragmentId::from(format!("fragment/{label}")),
        source_document_id: SourceDocumentId::from(format!("source/{label}/fixture")),
        checked_plot_id: CheckedPlotId::from(format!("checked/{label}/fixture")),
        expanded_plot_id: ExpandedPlotId::from(format!("expanded/{label}/fixture")),
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
