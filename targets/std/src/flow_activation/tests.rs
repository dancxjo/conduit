use super::*;
use conduit_composite::{
    BoundedActivationAdmission, BoundedActivationState, BoundedFoldAdmission, BoundedFoldState,
    BoundedScanAdmission, BoundedScanState, FlowSelectAdmission, FlowSelectCoordinator,
    FlowSelectState, KernelOperationBudget, KernelOperationFactory, KernelOperationRegistry,
};
use conduit_core::{
    kind_id, port_id, prepare_plan_on_hosts, PlannedActivationEffectMultiplicity,
    PlannedActivationEntry, PlannedActivationFront, PlannedActivationLimits, PortDirection,
    ValuePayload, BOOL_INFO_ID,
};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{HostedValueStore, PortId};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

#[path = "../../../../architecture/core/tests/common/sealed_state.rs"]
mod common;

struct BooleanFactory;

impl KernelOperationFactory for BooleanFactory {
    fn implementation_id(&self) -> &conduit_core::ImplementationId {
        static ID: std::sync::OnceLock<conduit_core::ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| conduit_core::ImplementationId::from("state@1"))
    }

    fn budget(&self, _: &conduit_core::PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 2,
            value_bytes: 2,
            maximum_value_bytes: 1,
            host_requests: 0,
            sign_items: 2,
        })
    }

    fn prepare(
        &self,
        placement: &conduit_core::PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        Ok(Box::new(BooleanBack {
            inputs: placement.inputs.len(),
        }))
    }
}

struct BooleanBack {
    inputs: usize,
}

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for BooleanBack {
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        let Some(first) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                io.consume_closed(PortId(0)).unwrap();
                StepOutcome::Complete
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if self.inputs > 1 && io.input(PortId(1)).is_none() {
            return StepOutcome::Await;
        }
        let value = first;
        io.consume(PortId(0)).unwrap();
        if self.inputs > 1 {
            io.consume(PortId(1)).unwrap();
        }
        io.send(PortId(0), value).unwrap();
        StepOutcome::Progress
    }
}

fn registry() -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry.install(BooleanFactory).unwrap();
    registry
}

fn identity() -> PreparationHostIdentity {
    PreparationHostIdentity {
        host_id: conduit_core::HostId::from("host"),
        boot_id: conduit_core::BootId::from("boot"),
        offer_generation: conduit_core::OfferGeneration(1),
    }
}

fn unary_plan(id: &str) -> Plan {
    let mut child_fragment = common::fragment();
    let boolean = kind_id(BOOL_INFO_ID);
    child_fragment.states.clear();
    child_fragment.expected_sign = vec![
        conduit_core::ExpectedSign::PlanFragmentReceived,
        conduit_core::ExpectedSign::PlanTerminal,
    ];
    child_fragment.sign_storage_budget =
        conduit_core::mandatory_sign_storage_requirement(&child_fragment.expected_sign).unwrap();
    child_fragment.placements[0].inputs[0].value_kind = boolean.clone();
    child_fragment.placements[0].outputs[0].value_kind = boolean.clone();
    child_fragment.fore_ports = vec![
        front("in", PortDirection::Input, "next", boolean.clone()),
        front("out", PortDirection::Output, "current", boolean.clone()),
    ];
    let child = common::seal(child_fragment);
    let sign_budget = child.fragments[0].sign_storage_budget;
    let outer = common::fragment();
    conduit_core::seal_plan_with_activations(
        conduit_core::PlotIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_plot_id: outer.checked_plot_id.clone(),
            expanded_plot_id: outer.expanded_plot_id.clone(),
        },
        conduit_core::PlanCompletionPolicy::Live,
        vec![],
        vec![conduit_core::PlannedActivation {
            activation_id: id.into(),
            owner_placement_id: conduit_core::PlacementId::from("placement"),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            input: activation_front("in", boolean.clone()),
            output: activation_front("out", boolean),
            limits: limits(),
            terminal_policy:
                conduit_core::PlannedActivationTerminalPolicy::DrainThenPropagateExact,
            cancellation_policy: conduit_core::PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: sign_budget,
        }],
        vec![outer],
    )
}

fn progression_plan(scan: bool) -> Plan {
    let mut child_fragment = common::fragment();
    child_fragment.states.clear();
    child_fragment.expected_sign = vec![
        conduit_core::ExpectedSign::PlanFragmentReceived,
        conduit_core::ExpectedSign::PlanTerminal,
    ];
    child_fragment.sign_storage_budget =
        conduit_core::mandatory_sign_storage_requirement(&child_fragment.expected_sign).unwrap();
    let mut item = child_fragment.placements[0].inputs[0].clone();
    item.port_id = port_id("item");
    child_fragment.placements[0].inputs.push(item);
    let byte = kind_id("fixture/byte@1");
    child_fragment.fore_ports = vec![
        front("accumulator", PortDirection::Input, "next", byte.clone()),
        front("item", PortDirection::Input, "item", byte.clone()),
        front("combined", PortDirection::Output, "current", byte.clone()),
    ];
    let child = common::seal(child_fragment);
    let sign_budget = child.fragments[0].sign_storage_budget;
    let progression_limits = PlannedActivationLimits {
        maximum_queue_bytes: 4,
        ..limits()
    };
    let activation = if scan {
        PlannedActivationEntry::Scan(conduit_core::PlannedScanActivation {
            activation_id: "scan".into(),
            owner_placement_id: conduit_core::PlacementId::from("placement"),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            accumulator_input: activation_front("accumulator", byte.clone()),
            item_input: activation_front("item", byte.clone()),
            output: activation_front("combined", byte),
            initial_accumulator: vec![0],
            retained_accumulator_bytes: 1,
            retained_item_bytes: 1,
            limits: progression_limits,
            terminal_policy:
                conduit_core::PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission,
            abnormal_policy:
                conduit_core::PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
            cancellation_policy:
                conduit_core::PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: sign_budget,
        })
    } else {
        PlannedActivationEntry::Fold(conduit_core::PlannedFoldActivation {
            activation_id: "fold".into(),
            owner_placement_id: conduit_core::PlacementId::from("placement"),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            accumulator_input: activation_front("accumulator", byte.clone()),
            item_input: activation_front("item", byte.clone()),
            output: activation_front("combined", byte),
            initial_accumulator: vec![0],
            retained_accumulator_bytes: 1,
            retained_item_bytes: 1,
            limits: progression_limits,
            terminal_policy:
                conduit_core::PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
            abnormal_policy:
                conduit_core::PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
            cancellation_policy:
                conduit_core::PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: sign_budget,
        })
    };
    let outer = common::fragment();
    conduit_core::seal_plan_with_activation_entries(
        conduit_core::PlotIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_plot_id: outer.checked_plot_id.clone(),
            expanded_plot_id: outer.expanded_plot_id.clone(),
        },
        conduit_core::PlanCompletionPolicy::Live,
        vec![],
        vec![activation],
        vec![outer],
    )
}

fn front(
    name: &str,
    direction: PortDirection,
    gear_port: &str,
    value_kind: conduit_core::KindId,
) -> conduit_core::PlannedForePort {
    conduit_core::PlannedForePort {
        front_port_id: port_id(name),
        direction,
        placement_id: conduit_core::PlacementId::from("placement"),
        gear_port_id: port_id(gear_port),
        value_kind,
        value_contract: None,
        abnormal_kind: None,
        track: conduit_core::ConnectionTrack::Payload,
        temporal: conduit_core::PortTemporal::Value,
        pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    }
}

fn activation_front(name: &str, value_kind: conduit_core::KindId) -> PlannedActivationFront {
    PlannedActivationFront {
        front_port_id: port_id(name),
        value_kind,
        abnormal_kind: None,
    }
}

fn limits() -> PlannedActivationLimits {
    PlannedActivationLimits {
        maximum_active: 1,
        maximum_queue_items: 1,
        maximum_queue_bytes: 1,
        maximum_items: 2,
    }
}

fn install(plan: &Plan, id: &str) -> PreparedPlannedActivationComposite {
    let mut host = StdActivationHost::new(identity(), registry());
    let mut prepared = prepare_plan_on_hosts(plan, &mut [&mut host]).unwrap();
    let before = prepared.subordinate_receipts().len();
    let composite = install_planned_activation(plan, &mut prepared, id, &mut host).unwrap();
    assert!(before > 0);
    assert!(prepared.subordinate_receipts().is_empty());
    assert_eq!(host.prepared_receipts(), prepared.receipts());
    composite
}

fn boolean(value: bool) -> ValuePayload {
    ValuePayload {
        value_kind: kind_id(BOOL_INFO_ID),
        encoded: conduit_core::InfoBool::new(value).encode().to_vec(),
    }
}

fn byte(value: u8) -> ValuePayload {
    ValuePayload {
        value_kind: kind_id("fixture/byte@1"),
        encoded: vec![value],
    }
}

#[test]
fn each_and_select_execute_with_pressure_terminal_and_cancellation_truth() {
    let each_plan = unary_plan("each");
    let mut each = install(&each_plan, "each").into_unary().unwrap();
    assert_eq!(
        each.activate(0, &boolean(true)).unwrap(),
        BoundedActivationAdmission::Accepted { sequence: 0 }
    );
    assert_eq!(
        each.activate(1, &boolean(false)).unwrap(),
        BoundedActivationAdmission::Full { sequence: 1 }
    );
    let mut delivered = false;
    for _ in 0..64 {
        each.step().unwrap();
        if let Some((sequence, output)) = each.output().unwrap() {
            assert_eq!(output, &boolean(true));
            each.complete_output(sequence).unwrap();
            delivered = true;
            break;
        }
    }
    assert!(delivered);
    let mut succeeded = false;
    for _ in 0..64 {
        if matches!(
            each.step().unwrap(),
            BoundedActivationState::Succeeded { sequence: 0 }
        ) {
            succeeded = true;
            break;
        }
    }
    assert!(succeeded);
    each.close_input().unwrap();
    assert_eq!(each.step().unwrap(), &BoundedActivationState::Drained);

    let select_plan = unary_plan("select");
    let unary = install(&select_plan, "select").into_unary().unwrap();
    let mut select = FlowSelectCoordinator::from_prepared_activation(unary).unwrap();
    assert_eq!(
        select.admit(0, boolean(true)).unwrap(),
        FlowSelectAdmission::Accepted { sequence: 0 }
    );
    for _ in 0..64 {
        if matches!(select.step().unwrap(), FlowSelectState::OutputReady { .. }) {
            break;
        }
    }
    assert_eq!(select.output().unwrap().1, &boolean(true));
    select.complete_output(0).unwrap();
    select.cancel().unwrap();
    assert_eq!(
        select.state(),
        &FlowSelectState::Cancelled { sequence: None }
    );
}

#[test]
fn fold_and_scan_execute_from_exact_receipt_backed_children() {
    let fold_plan = progression_plan(false);
    let mut fold = install(&fold_plan, "fold").into_fold().unwrap();
    assert_eq!(
        fold.admit(&byte(7)).unwrap(),
        BoundedFoldAdmission::Accepted
    );
    for _ in 0..64 {
        if matches!(fold.step().unwrap(), BoundedFoldState::Idle) {
            break;
        }
    }
    fold.close_input().unwrap();
    assert_eq!(fold.step().unwrap(), &BoundedFoldState::FinalReady);
    assert_eq!(fold.final_value().unwrap().unwrap(), byte(0));

    let scan_plan = progression_plan(true);
    let mut scan = install(&scan_plan, "scan").into_scan().unwrap();
    assert_eq!(
        scan.admit(&byte(9)).unwrap(),
        BoundedScanAdmission::Accepted
    );
    for _ in 0..64 {
        if matches!(scan.step().unwrap(), BoundedScanState::OutputReady) {
            break;
        }
    }
    let mut output = ValuePayload {
        value_kind: kind_id("fixture/byte@1"),
        encoded: Vec::with_capacity(1),
    };
    assert!(scan.output_into(&mut output).unwrap());
    assert_eq!(output, byte(0));
    scan.complete_output().unwrap();
    scan.cancel().unwrap();
    assert_eq!(scan.step().unwrap(), &BoundedScanState::Cancelled);
}

#[test]
fn each_allocates_nothing_after_receipt_backed_installation() {
    let plan = unary_plan("each");
    let mut each = install(&plan, "each").into_unary().unwrap();
    let input = boolean(true);

    let allocation_guard = crate::allocation_probe::begin();
    assert!(matches!(
        each.activate(0, &input).unwrap(),
        BoundedActivationAdmission::Accepted { sequence: 0 }
    ));
    for _ in 0..64 {
        each.step().unwrap();
        if let Some((sequence, _)) = each.output().unwrap() {
            each.complete_output(sequence).unwrap();
            break;
        }
    }
    for _ in 0..64 {
        if matches!(
            each.step().unwrap(),
            BoundedActivationState::Succeeded { sequence: 0 }
        ) {
            break;
        }
    }
    each.close_input().unwrap();
    assert!(matches!(
        each.step().unwrap(),
        BoundedActivationState::Drained
    ));
    assert_eq!(allocation_guard.finish(), 0);
}

#[test]
fn installation_rejects_stale_identity_and_consumed_receipts() {
    let plan = unary_plan("each");
    let mut stale_identity = identity();
    stale_identity.boot_id = conduit_core::BootId::from("replacement-boot");
    let mut stale = StdActivationHost::new(stale_identity, registry());
    assert!(matches!(
        prepare_plan_on_hosts(&plan, &mut [&mut stale]),
        Err(conduit_core::PlanPreparationError::HostRefused {
            reason: HostPreparationRefusal::StaleBoot,
            ..
        })
    ));

    let mut host = StdActivationHost::new(identity(), registry());
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    install_planned_activation(&plan, &mut prepared, "each", &mut host).unwrap();
    assert!(install_planned_activation(&plan, &mut prepared, "each", &mut host).is_err());
}

pub(crate) fn todo_scan_plan() -> Plan {
    let mut child_fragment = common::fragment();
    child_fragment.states.clear();
    child_fragment.expected_sign = vec![
        conduit_core::ExpectedSign::PlanFragmentReceived,
        conduit_core::ExpectedSign::PlanTerminal,
    ];
    child_fragment.sign_storage_budget =
        conduit_core::mandatory_sign_storage_requirement(&child_fragment.expected_sign).unwrap();
    let placement = &mut child_fragment.placements[0];
    let offer = todo_combine_offer();
    placement.kind_id = kind_id(conduit_todo_plot::TODO_COMBINE_KIND);
    placement.kind_contract_revision =
        conduit_core::KindIdentity::from(conduit_todo_plot::TODO_COMBINE_REVISION);
    placement.capability_id = offer.capability_id;
    placement.execution_profile_id = offer.implementation.execution_profile_id;
    placement.implementation_id = offer.implementation.implementation_id;
    placement.artifact_id = offer.implementation.artifact_id;
    placement.limits = offer.limits;
    placement.semantic_contract = offer.semantic_contract;
    let state_kind = kind_id(conduit_todo_plot::TODO_STATE_INFO_ID);
    let command_kind = kind_id(conduit_todo_plot::TODO_COMMAND_INFO_ID);
    placement.inputs[0].port_id = port_id("accumulator");
    placement.inputs[0].value_kind = state_kind.clone();
    let mut item = placement.inputs[0].clone();
    item.port_id = port_id("item");
    item.value_kind = command_kind.clone();
    placement.inputs.push(item);
    placement.outputs[0].port_id = port_id("combined");
    placement.outputs[0].value_kind = state_kind.clone();
    child_fragment.fore_ports = vec![
        front(
            "accumulator",
            PortDirection::Input,
            "accumulator",
            state_kind.clone(),
        ),
        front("item", PortDirection::Input, "item", command_kind.clone()),
        front(
            "combined",
            PortDirection::Output,
            "combined",
            state_kind.clone(),
        ),
    ];
    for port in &mut child_fragment.fore_ports {
        port.byte_capacity = conduit_todo_plot::STATE_MAX_BYTES as u32;
    }
    let child = common::seal(child_fragment);
    let sign_budget = child.fragments[0].sign_storage_budget;
    let initial = conduit_todo_plot::TodoState::new("List".into())
        .unwrap()
        .encode_info()
        .unwrap();
    let outer = common::fragment();
    conduit_core::seal_plan_with_activation_entries(
        conduit_core::PlotIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_plot_id: outer.checked_plot_id.clone(),
            expanded_plot_id: outer.expanded_plot_id.clone(),
        },
        conduit_core::PlanCompletionPolicy::Live,
        vec![],
        vec![PlannedActivationEntry::Scan(
            conduit_core::PlannedScanActivation {
                activation_id: "scan".into(),
                owner_placement_id: conduit_core::PlacementId::from("placement"),
                selected_plan_id: child.plan_id.clone(),
                selected_plan: Box::new(child),
                accumulator_input: activation_front("accumulator", state_kind.clone()),
                item_input: activation_front("item", command_kind),
                output: activation_front("combined", state_kind),
                initial_accumulator: initial,
                retained_accumulator_bytes: conduit_todo_plot::STATE_MAX_BYTES as u32,
                retained_item_bytes: conduit_todo_plot::COMMAND_MAX_BYTES as u32,
                limits: PlannedActivationLimits {
                    maximum_active: 1,
                    maximum_queue_items: 1,
                    maximum_queue_bytes: (2
                        * (conduit_todo_plot::STATE_MAX_BYTES
                            + conduit_todo_plot::COMMAND_MAX_BYTES))
                        as u32,
                    maximum_items: 4,
                },
                terminal_policy:
                    conduit_core::PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission,
                abnormal_policy:
                    conduit_core::PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
                cancellation_policy:
                    conduit_core::PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission,
                effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
                per_activation_sign_budget: sign_budget,
            },
        )],
        vec![outer],
    )
}

#[test]
fn todo_commands_advance_one_receipt_backed_scan() {
    let plan = todo_scan_plan();
    let mut host = StdActivationHost::new(identity(), standard_child_registry().unwrap());
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let mut scan = install_planned_activation(&plan, &mut prepared, "scan", &mut host)
        .unwrap()
        .into_scan()
        .unwrap();
    for (command, revision) in [
        (
            conduit_todo_plot::TodoCommand::Add {
                text: "Milk".into(),
            },
            1,
        ),
        (
            conduit_todo_plot::TodoCommand::SetComplete {
                id: "task-1".into(),
                complete: true,
            },
            2,
        ),
        (
            conduit_todo_plot::TodoCommand::SetComplete {
                id: "task-1".into(),
                complete: false,
            },
            3,
        ),
    ] {
        let value = ValuePayload {
            value_kind: kind_id(conduit_todo_plot::TODO_COMMAND_INFO_ID),
            encoded: command.encode_info().unwrap(),
        };
        assert_eq!(scan.admit(&value).unwrap(), BoundedScanAdmission::Accepted);
        for _ in 0..128 {
            if matches!(scan.step().unwrap(), BoundedScanState::OutputReady) {
                break;
            }
        }
        let mut output = ValuePayload {
            value_kind: kind_id(conduit_todo_plot::TODO_STATE_INFO_ID),
            encoded: Vec::with_capacity(conduit_todo_plot::STATE_MAX_BYTES),
        };
        assert!(scan.output_into(&mut output).unwrap());
        let state = conduit_todo_plot::TodoState::decode_info(&output.encoded).unwrap();
        assert_eq!(state.revision, revision);
        assert_eq!(state.items[0].text, "Milk");
        assert_eq!(state.items[0].complete, revision == 2);
        scan.complete_output().unwrap();
    }
    scan.close_input().unwrap();
    assert_eq!(scan.step().unwrap(), &BoundedScanState::Complete);
    assert!(scan
        .admit(&ValuePayload {
            value_kind: kind_id(conduit_todo_plot::TODO_COMMAND_INFO_ID),
            encoded: conduit_todo_plot::TodoCommand::Add {
                text: "Late".into(),
            }
            .encode_info()
            .unwrap(),
        })
        .is_err());
}

#[test]
fn production_todo_child_factory_refuses_substituted_plan_facts() {
    let plan = todo_scan_plan();
    let PlannedActivationEntry::Scan(entry) = &plan.activations[0] else {
        panic!("expected exact scan");
    };
    let placement = &entry.selected_plan.fragments[0].placements[0];
    let factory = TodoCombineFactory;
    assert_eq!(
        factory.budget(placement).unwrap().maximum_value_bytes,
        conduit_todo_plot::STATE_MAX_BYTES as u32
    );

    let mut altered = placement.clone();
    altered.artifact_id = conduit_core::ArtifactId::from("substitute");
    assert!(factory.budget(&altered).is_err());
    altered = placement.clone();
    altered.inputs[1].value_kind = kind_id(conduit_todo_plot::TODO_STATE_INFO_ID);
    assert!(factory.budget(&altered).is_err());
    altered = placement.clone();
    altered.limits.max_queue_items += 1;
    assert!(factory.budget(&altered).is_err());
}

#[test]
fn production_todo_scan_preserves_queue_pressure_and_cancellation() {
    let plan = todo_scan_plan();
    let mut host = StdActivationHost::new(identity(), standard_child_registry().unwrap());
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let mut scan = install_planned_activation(&plan, &mut prepared, "scan", &mut host)
        .unwrap()
        .into_scan()
        .unwrap();
    let command = ValuePayload {
        value_kind: kind_id(conduit_todo_plot::TODO_COMMAND_INFO_ID),
        encoded: conduit_todo_plot::TodoCommand::Add {
            text: "Milk".into(),
        }
        .encode_info()
        .unwrap(),
    };
    assert_eq!(
        scan.admit(&command).unwrap(),
        BoundedScanAdmission::Accepted
    );
    assert_eq!(
        scan.admit(&command).unwrap(),
        BoundedScanAdmission::Accepted
    );
    assert_eq!(scan.admit(&command).unwrap(), BoundedScanAdmission::Full);
    scan.cancel().unwrap();
    assert_eq!(scan.step().unwrap(), &BoundedScanState::Cancelled);
    assert!(scan.admit(&command).is_err());
}
