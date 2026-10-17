use super::*;
use conduit_composite::{
    BoundedActivationState, BoundedFoldState, BoundedScanState, FlowSelectCoordinator,
    FlowSelectState,
};
use conduit_core::{
    kind_id, port_id, prepare_plan_on_hosts, PlannedActivationEffectMultiplicity,
    PlannedActivationEntry, PlannedActivationFront, PlannedActivationLimits, PortDirection,
    ValuePayload, BOOL_INFO_ID,
};
use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::PortId;

#[path = "../../../../../architecture/core/tests/common/sealed_state.rs"]
mod common;

struct FixtureFactory;

impl KernelOperationFactory for FixtureFactory {
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
        Ok(Box::new(FixtureBack(placement.inputs.len())))
    }
}

struct FixtureBack(usize);
impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for FixtureBack {
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        let Some(value) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                io.consume_closed(PortId(0)).unwrap();
                StepOutcome::Complete
            } else {
                StepOutcome::Await
            };
        };
        if self.0 > 1 && io.input(PortId(1)).is_none() {
            return StepOutcome::Await;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.consume(PortId(0)).unwrap();
        if self.0 > 1 {
            io.consume(PortId(1)).unwrap();
        }
        io.send(PortId(0), value).unwrap();
        StepOutcome::Progress
    }
}

fn identity() -> PreparationHostIdentity {
    PreparationHostIdentity {
        host_id: conduit_core::HostId::from("host"),
        boot_id: conduit_core::BootId::from("boot"),
        offer_generation: conduit_core::OfferGeneration(1),
    }
}

fn front(name: &str, direction: PortDirection, gear_port: &str) -> conduit_core::PlannedForePort {
    conduit_core::PlannedForePort {
        front_port_id: port_id(name),
        direction,
        placement_id: conduit_core::PlacementId::from("placement"),
        gear_port_id: port_id(gear_port),
        value_kind: kind_id(BOOL_INFO_ID),
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

fn activation_front(name: &str) -> PlannedActivationFront {
    PlannedActivationFront {
        front_port_id: port_id(name),
        value_kind: kind_id(BOOL_INFO_ID),
        abnormal_kind: None,
    }
}

fn child(binary: bool) -> Plan {
    let mut fragment = common::fragment();
    fragment.states.clear();
    fragment.expected_sign = vec![
        conduit_core::ExpectedSign::PlanFragmentReceived,
        conduit_core::ExpectedSign::PlanTerminal,
    ];
    fragment.sign_storage_budget =
        conduit_core::mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    fragment.placements[0].inputs[0].value_kind = kind_id(BOOL_INFO_ID);
    fragment.placements[0].outputs[0].value_kind = kind_id(BOOL_INFO_ID);
    if binary {
        let mut item = fragment.placements[0].inputs[0].clone();
        item.port_id = port_id("item");
        fragment.placements[0].inputs.push(item);
        fragment.fore_ports = vec![
            front("accumulator", PortDirection::Input, "next"),
            front("item", PortDirection::Input, "item"),
            front("combined", PortDirection::Output, "current"),
        ];
    } else {
        fragment.fore_ports = vec![
            front("in", PortDirection::Input, "next"),
            front("out", PortDirection::Output, "current"),
        ];
    }
    common::seal(fragment)
}

fn activation_plan(which: &str) -> Plan {
    let unary = child(false);
    let binary = child(true);
    let limits = PlannedActivationLimits {
        maximum_active: 1,
        maximum_queue_items: 1,
        maximum_queue_bytes: 4,
        maximum_items: 2,
    };
    let unary_entry = |id: &str| {
        PlannedActivationEntry::Unary(conduit_core::PlannedActivation {
            activation_id: id.into(),
            owner_placement_id: conduit_core::PlacementId::from("placement"),
            selected_plan_id: unary.plan_id.clone(),
            selected_plan: Box::new(unary.clone()),
            input: activation_front("in"),
            output: activation_front("out"),
            limits,
            terminal_policy: conduit_core::PlannedActivationTerminalPolicy::DrainThenPropagateExact,
            cancellation_policy: conduit_core::PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: unary.fragments[0].sign_storage_budget,
        })
    };
    let progression = |id: &str, scan: bool| {
        if scan {
            PlannedActivationEntry::Scan(conduit_core::PlannedScanActivation {
                activation_id: id.into(),
                owner_placement_id: conduit_core::PlacementId::from("placement"),
                selected_plan_id: binary.plan_id.clone(),
                selected_plan: Box::new(binary.clone()),
                accumulator_input: activation_front("accumulator"),
                item_input: activation_front("item"),
                output: activation_front("combined"),
                initial_accumulator: conduit_core::InfoBool::FALSE.encode().to_vec(),
                retained_accumulator_bytes: 1,
                retained_item_bytes: 1,
                limits,
                terminal_policy:
                    conduit_core::PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission,
                abnormal_policy:
                    conduit_core::PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
                cancellation_policy:
                    conduit_core::PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission,
                effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
                per_activation_sign_budget: binary.fragments[0].sign_storage_budget,
            })
        } else {
            PlannedActivationEntry::Fold(conduit_core::PlannedFoldActivation {
                activation_id: id.into(),
                owner_placement_id: conduit_core::PlacementId::from("placement"),
                selected_plan_id: binary.plan_id.clone(),
                selected_plan: Box::new(binary.clone()),
                accumulator_input: activation_front("accumulator"),
                item_input: activation_front("item"),
                output: activation_front("combined"),
                initial_accumulator: conduit_core::InfoBool::FALSE.encode().to_vec(),
                retained_accumulator_bytes: 1,
                retained_item_bytes: 1,
                limits,
                terminal_policy:
                    conduit_core::PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
                abnormal_policy:
                    conduit_core::PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
                cancellation_policy:
                    conduit_core::PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
                effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
                per_activation_sign_budget: binary.fragments[0].sign_storage_budget,
            })
        }
    };
    let outer = common::fragment();
    let activation = match which {
        "each" | "select" => unary_entry(which),
        "fold" => progression(which, false),
        "scan" => progression(which, true),
        _ => unreachable!(),
    };
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

fn value(value: bool) -> ValuePayload {
    ValuePayload {
        value_kind: kind_id(BOOL_INFO_ID),
        encoded: conduit_core::InfoBool::new(value).encode().to_vec(),
    }
}

#[test]
fn browser_host_executes_each_select_fold_and_scan_from_receipts() {
    let plan = activation_plan("each");
    let mut registry = KernelOperationRegistry::new();
    registry.install(FixtureFactory).unwrap();
    let mut host = BrowserActivationHost::with_registry(identity(), registry);
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let mut each = install_planned_activation(&plan, &mut prepared, "each", &mut host)
        .unwrap()
        .into_unary()
        .unwrap();
    each.activate(0, &value(true)).unwrap();
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
            BoundedActivationState::Succeeded { .. }
        ) {
            break;
        }
    }

    let plan = activation_plan("select");
    let mut registry = KernelOperationRegistry::new();
    registry.install(FixtureFactory).unwrap();
    let mut host = BrowserActivationHost::with_registry(identity(), registry);
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let unary = install_planned_activation(&plan, &mut prepared, "select", &mut host)
        .unwrap()
        .into_unary()
        .unwrap();
    let mut select = FlowSelectCoordinator::from_prepared_activation(unary).unwrap();
    select.admit(0, value(true)).unwrap();
    for _ in 0..64 {
        if matches!(select.step().unwrap(), FlowSelectState::OutputReady { .. }) {
            break;
        }
    }
    assert!(select.output().is_some());

    let plan = activation_plan("fold");
    let mut registry = KernelOperationRegistry::new();
    registry.install(FixtureFactory).unwrap();
    let mut host = BrowserActivationHost::with_registry(identity(), registry);
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let mut fold = install_planned_activation(&plan, &mut prepared, "fold", &mut host)
        .unwrap()
        .into_fold()
        .unwrap();
    fold.admit(&value(true)).unwrap();
    fold.close_input().unwrap();
    for _ in 0..64 {
        if matches!(fold.step().unwrap(), BoundedFoldState::FinalReady) {
            break;
        }
    }
    assert!(fold.final_value().unwrap().is_some());

    let plan = activation_plan("scan");
    let mut registry = KernelOperationRegistry::new();
    registry.install(FixtureFactory).unwrap();
    let mut host = BrowserActivationHost::with_registry(identity(), registry);
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let mut scan = install_planned_activation(&plan, &mut prepared, "scan", &mut host)
        .unwrap()
        .into_scan()
        .unwrap();
    scan.admit(&value(true)).unwrap();
    for _ in 0..64 {
        if matches!(scan.step().unwrap(), BoundedScanState::OutputReady) {
            break;
        }
    }
    let mut output = ValuePayload {
        value_kind: kind_id(BOOL_INFO_ID),
        encoded: Vec::with_capacity(1),
    };
    assert!(scan.output_into(&mut output).unwrap());
    assert_eq!(prepared.subordinate_receipts().len(), 0);
}

#[test]
fn browser_plan_refuses_an_uninstalled_selected_back() {
    let plan = activation_plan("each");
    let error = BrowserActivationHost::for_plan(identity(), &plan)
        .err()
        .unwrap();
    assert!(error.contains("state@1"));
}

mod installed_inventory;
