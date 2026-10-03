//! Preparation proof uses a real numeric lowering, with a fixture-only mapping.
use super::{claim, request, scope, table};
use crate::machine_membrane::{
    REGISTER_CALL, RegisterRefusal,
    register_call::{RegisterHostCall, contract},
};
use alloc::vec;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId};
use conduit_plan_lowering::lowering::{LoweredPlanFragment, lower_plan_fragment};

fn selected() -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    let scope = scope();
    let kind = contract();
    let placement = PlacementId::from("placement/registers");
    let gear = PlannedGear {
        placement_id: placement.clone(),
        gear_id: GearId::from("gear/registers"),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        source_span: None,
        execution_profile_id: ExecutionProfileId::from("conduitos/cooperative-bounded-step@1"),
        configuration: vec![],
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        capability_id: scope.capability_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        artifact_id: ArtifactId::from("artifact/registers/fixture"),
        base: Some(BaseProviderBinding {
            base_id: HostBaseId::from("base/registers"),
            provider_instance_id: scope.base_instance_id.clone(),
            provider_generation: scope.base_provider_generation,
            implementation_id: BaseImplementationId::from("base/registers/fixture@1"),
            mechanism_family: HostBaseKindId::from("machine/register-window"),
            enforcement_class: BaseEnforcementClass::Cooperative,
        }),
        realization_characteristics: vec![],
        limits: kind.limits.clone(),
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
        semantic_contract: kind.semantic_contract(),
        terminal_transductions: vec![],
        host_calls: vec![HostCallRequirement {
            contract_id: HostCallContractId::from(REGISTER_CALL),
            target_kind: Some(kind.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: 16,
            maximum_output_bytes: 4,
        }],
        resources: vec![ResourceBinding {
            pool_id: scope.resource_pool_id.clone(),
            class_id: ResourceClassId::from("machine/register-window"),
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
        fragment_id: FragmentId::from("fragment/registers"),
        source_document_id: SourceDocumentId::from("source/registers/fixture"),
        checked_plot_id: CheckedPlotId::from("checked/registers/fixture"),
        expanded_plot_id: ExpandedPlotId::from("expanded/registers/fixture"),
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

fn bound_leaf(
    registers: &mut [u32],
    fragment: &PlanFragment,
    active: &ActivePlayIdentity,
) -> crate::machine_membrane::RegisterLeaf {
    let mut table = table(7);
    let mut request = request();
    request.scope.plan_id = fragment.plan_id.clone();
    request.scope.active_play_id = active.active_play_id.clone();
    let handle = table.issue(request).unwrap();
    let mut claim = claim();
    claim.plan_id = fragment.plan_id.clone();
    claim.active_play_id = active.active_play_id.clone();
    unsafe {
        crate::machine_membrane::RegisterLeaf::from_admitted_mapping(
            table,
            handle,
            claim,
            registers.as_mut_ptr(),
            crate::machine_membrane::RegisterEnvelope {
                bytes: (registers.len() * 4) as u32,
                writable: true,
            },
        )
        .unwrap()
    }
}

#[test]
fn selected_base_binds_to_the_exact_lowered_operation() {
    let (fragment, lowered, active, placement) = selected();
    let mut registers = [42];
    let mut host = RegisterHostCall::bind_selected(
        bound_leaf(&mut registers, &fragment, &active),
        &fragment,
        &lowered,
        &active,
        &placement,
    )
    .unwrap();
    assert_eq!(
        host.invoke(NodeId(0), HostCallId(0), &[0; 16]),
        Ok(42_u32.to_le_bytes())
    );
}

#[test]
fn altered_planning_and_lowering_facts_cannot_bind_a_leaf() {
    for case in 0..16 {
        let (mut fragment, mut lowered, mut active, placement) = selected();
        let mut registers = [9];
        let provider = bound_leaf(&mut registers, &fragment, &active);
        match case {
            0 => active.active_play_id = ActivePlayId::from("play/stale"),
            1 => fragment.boot_id = BootId::from("boot/stale"),
            2 => lowered.identity.plan_id = PlanId::from("plan/other"),
            3 => {
                fragment.placements[0]
                    .base
                    .as_mut()
                    .unwrap()
                    .provider_generation += 1
            }
            4 => fragment.placements[0].authority.clear(),
            5 => fragment.placements[0].resources.clear(),
            6 => {
                fragment.placements[0].implementation_id =
                    ImplementationId::from("implementation/other")
            }
            7 => lowered.host_calls[0].binding.maximum_input_bytes += 1,
            8 => lowered.host_calls[0].contract_id = HostCallContractId::from("call/other"),
            9 => lowered.host_calls[0].call = HostCallId(1),
            10 => fragment.placements[0].outputs[0].value_kind = kind_id("value/u64"),
            11 => {
                let duplicate = lowered.host_calls[0].clone();
                lowered.host_calls.push(duplicate);
            }
            12 => lowered.nodes[0].maximum_step_fuel += 1,
            13 => lowered.cord_value_bytes += 1,
            14 => lowered.sign_items += 1,
            15 => lowered.nodes[0].outputs[0].port = conduit_kernel::PortId(1),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                RegisterHostCall::bind_selected(provider, &fragment, &lowered, &active, &placement),
                Err(RegisterRefusal::WrongBinding)
            ),
            "case {case}"
        );
        assert_eq!(registers, [9]);
    }
}

#[test]
fn register_kind_is_called_by_checked_plot_topology() {
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    };
    let source = include_str!("../../plots/machine/register32.conduit");
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (startup, profile) = crate::machine_membrane::register_call::catalogs();
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "machine-register-call", &profile)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    assert_eq!(expanded.gears[0].kind_id, contract().kind_id);
    expanded.validate_expansion().unwrap();
}
