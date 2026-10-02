use conduit_composite::{
    BoundedActivationAdmission, BoundedActivationHost, BoundedActivationState,
    BoundedFoldActivationHost, BoundedFoldState, BoundedScanActivationHost, BoundedScanAdmission,
    BoundedScanState, KernelCompositeDefinition, KernelCompositeError, KernelCompositeHost,
    KernelCompositeStatus, KernelOperationBudget, KernelOperationFactory, KernelOperationRegistry,
};
use conduit_core::{
    kind_id, process_owned_line_offer, ArtifactId, BaseImplementationId, BootId, CapabilityId,
    CapabilityLimits, FailureReason, GearId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, KindIdentity, OfferGeneration, PlacementId, PlannedActivation,
    PlannedActivationCancellationPolicy, PlannedActivationFront, PlannedActivationLimits,
    PlannedActivationTerminalPolicy, PlannedFoldAbnormalPolicy, PlannedFoldActivation,
    PlannedFoldCancellationPolicy, PlannedFoldTerminalPolicy, PlannedGear,
    PlannedScanAbnormalPolicy, PlannedScanActivation, PlannedScanCancellationPolicy,
    PlannedScanTerminalPolicy, PortDescriptor, PortDirection, SignStorageBudget, ValuePayload,
    PROTOCOL_VERSION,
};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{HostedValueStore, PortId as KernelPortId};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_plot::{parse, KindProjection, ProfileCatalog};
use std::collections::BTreeMap;

#[path = "support/allocation.rs"]
mod allocation;
use allocation::allocations_during;

const ECHO_KIND: &str = "test/kernel-composite-echo";
const VALUE_KIND: &str = "value/bytes";
const IMPLEMENTATION: &str = "test/kernel-composite-echo-v1";
const COMBINE_KIND: &str = "test/kernel-composite-combine";
const COMBINE_IMPLEMENTATION: &str = "test/kernel-composite-combine-v1";

fn descriptor(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: kind_id(VALUE_KIND),
        direction,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    }
}

fn catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindProjection {
            kind_id: kind_id(ECHO_KIND),
            kind_contract_revision: KindIdentity::from("test/kernel-composite-echo@1"),
            inputs: vec![descriptor("in", PortDirection::Input)],
            outputs: vec![descriptor("out", PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    catalog
}

fn advertisement(host: &str, boot: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/kernel-composite"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![conduit_core::capability_offer_from_parts! {
            semantic_contract: Default::default(),
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from("echo"),
            kind_id: kind_id(ECHO_KIND),
            kind_contract_revision: KindIdentity::from("test/kernel-composite-echo@1"),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: "test/kernel-composite@1".into(),
                implementation_id: IMPLEMENTATION.into(),
                artifact_id: "test/kernel-composite-artifact-v1".into(),
            },
            inputs: vec![descriptor("in", PortDirection::Input)],
            outputs: vec![descriptor("out", PortDirection::Output)],
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 2,
                max_queue_items: 2,
                max_queue_bytes: 16,
            },
        }],
    }
}

fn definition() -> KernelCompositeDefinition {
    let plot = parse(
        "plot test/two-child-echo (\n >> input: value/bytes\n output: value/bytes >>\n) {\n first: test/kernel-composite-echo\n second: test/kernel-composite-echo\n input >> first.in\n first.out >> second.in\n second.out >> output\n}\n",
        &catalog(),
    )
    .unwrap();
    let first = advertisement("first-child", "first-boot");
    let second = advertisement("second-child", "second-boot");
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("test/two-child-echo/first"),
                PlacementChoice {
                    host_id: first.host_id.clone(),
                    capability_id: CapabilityId::from("echo"),
                },
            ),
            (
                GearId::from("test/two-child-echo/second"),
                PlacementChoice {
                    host_id: second.host_id.clone(),
                    capability_id: CapabilityId::from("echo"),
                },
            ),
        ]),
    };
    let line = process_owned_line_offer(
        "line/first-second",
        "link/first-second",
        BaseImplementationId::from("conduit.proof/in-memory@1"),
        "fixture/in-memory/first-second",
        &first,
        &second,
        2,
        16,
    );
    let internal_plan = plan_with_line_offers(
        &plot,
        &[first, second],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.proof/in-memory@1"),
        ],
        2,
        16,
        &[line],
    )
    .unwrap();
    KernelCompositeDefinition::from_authored_export(
        HostId::from("kernel-composite"),
        BootId::from("kernel-composite-boot"),
        OfferGeneration(1),
        HostProfileId::from("composite/kernel"),
        ImplementationId::from("composite/kernel-two-echo-v1"),
        ArtifactId::from("composite/kernel-two-echo-artifact-v1"),
        &plot,
        &CapabilityId::from("run"),
        internal_plan,
        FailureReason::CompositeCapabilityFailed,
    )
    .unwrap()
}

struct EchoFactory {
    implementation_id: ImplementationId,
    fail: bool,
}

impl KernelOperationFactory for EchoFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn budget(&self, _placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 0,
            value_bytes: 0,
            maximum_value_bytes: 16,
            host_requests: 0,
            sign_items: 8,
        })
    }

    fn prepare(
        &self,
        _placement: &PlannedGear,
        _values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        if self.fail {
            Ok(Box::new(Fail))
        } else {
            Ok(Box::new(Echo))
        }
    }
}

struct Echo;

struct Fail;

struct Combine;

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for Fail {
    fn step(
        &mut self,
        _io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _input_bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        StepOutcome::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::InvalidLifecycle,
            detail: 17,
        })
    }
}

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for Echo {
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _input_bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        if let Some(value) = io.input(KernelPortId(0)) {
            if !io.output_ready(KernelPortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume(KernelPortId(0)).is_err() || io.send(KernelPortId(0), value).is_err() {
                return StepOutcome::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidLifecycle,
                    detail: 18,
                });
            }
            StepOutcome::Progress
        } else if io.input_closed(KernelPortId(0)) {
            if io.consume_closed(KernelPortId(0)).is_err() {
                return StepOutcome::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidLifecycle,
                    detail: 19,
                });
            }
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for Combine {
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _input_bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        match (io.input(KernelPortId(0)), io.input(KernelPortId(1))) {
            (Some(_accumulator), Some(item)) => {
                if !io.output_ready(KernelPortId(0)) {
                    return StepOutcome::Await;
                }
                if io.consume(KernelPortId(0)).is_err()
                    || io.consume(KernelPortId(1)).is_err()
                    || io.send(KernelPortId(0), item).is_err()
                {
                    return StepOutcome::Fail(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::InvalidLifecycle,
                        detail: 20,
                    });
                }
                StepOutcome::Progress
            }
            (None, None)
                if io.input_closed(KernelPortId(0)) && io.input_closed(KernelPortId(1)) =>
            {
                if io.consume_closed(KernelPortId(0)).is_err()
                    || io.consume_closed(KernelPortId(1)).is_err()
                {
                    return StepOutcome::Fail(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::InvalidLifecycle,
                        detail: 21,
                    });
                }
                StepOutcome::Complete
            }
            _ => StepOutcome::Await,
        }
    }
}

struct CombineFactory;

impl KernelOperationFactory for CombineFactory {
    fn implementation_id(&self) -> &ImplementationId {
        static ID: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| COMBINE_IMPLEMENTATION.into())
    }

    fn budget(&self, _placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 0,
            value_bytes: 0,
            maximum_value_bytes: 16,
            host_requests: 0,
            sign_items: 8,
        })
    }

    fn prepare(
        &self,
        _placement: &PlannedGear,
        _values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        Ok(Box::new(Combine))
    }
}

fn registry() -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(EchoFactory {
            implementation_id: IMPLEMENTATION.into(),
            fail: false,
        })
        .unwrap();
    registry
}

fn failing_registry() -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(EchoFactory {
            implementation_id: IMPLEMENTATION.into(),
            fail: true,
        })
        .unwrap();
    registry
}

fn value(bytes: &[u8]) -> ValuePayload {
    ValuePayload {
        value_kind: kind_id(VALUE_KIND),
        encoded: bytes.to_vec(),
    }
}

fn fold_definition() -> KernelCompositeDefinition {
    fold_definition_with_abnormal(None)
}

fn fold_definition_with_abnormal(
    abnormal: Option<conduit_core::KindId>,
) -> KernelCompositeDefinition {
    let with_abnormal = |name, direction| descriptor(name, direction);
    let accumulator = with_abnormal("accumulator", PortDirection::Input);
    let item = with_abnormal("item", PortDirection::Input);
    let combined = with_abnormal("combined", PortDirection::Output);
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindProjection {
            kind_id: kind_id(COMBINE_KIND),
            kind_contract_revision: KindIdentity::from("test/kernel-composite-combine@1"),
            inputs: vec![accumulator.clone(), item.clone()],
            outputs: vec![combined.clone()],
            configuration: vec![],
        })
        .unwrap();
    let plot = parse(
        "plot test/fold-combine (\n >> accumulator: value/bytes\n >> item: value/bytes\n combined: value/bytes >>\n) {\n combine: test/kernel-composite-combine\n accumulator >> combine.accumulator\n item >> combine.item\n combine.combined >> combined\n}\n",
        &catalog,
    )
    .unwrap();
    let mut host = advertisement("combine-child", "combine-boot");
    host.capabilities[0].capability_id = CapabilityId::from("combine");
    host.capabilities[0].kind_id = kind_id(COMBINE_KIND);
    host.capabilities[0].kind_contract_revision =
        KindIdentity::from("test/kernel-composite-combine@1");
    host.capabilities[0].implementation.implementation_id = COMBINE_IMPLEMENTATION.into();
    host.capabilities[0].inputs = vec![accumulator, item];
    host.capabilities[0].outputs = vec![combined];
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([(
            GearId::from("test/fold-combine/combine"),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: CapabilityId::from("combine"),
            },
        )]),
    };
    let internal_plan = plan_with_line_offers(
        &plot,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        2,
        16,
        &[],
    )
    .unwrap();
    let mut definition = KernelCompositeDefinition::from_authored_export(
        HostId::from("fold-composite"),
        BootId::from("fold-composite-boot"),
        OfferGeneration(1),
        HostProfileId::from("composite/kernel"),
        ImplementationId::from("composite/kernel-fold-v1"),
        ArtifactId::from("composite/kernel-fold-artifact-v1"),
        &plot,
        &CapabilityId::from("run"),
        internal_plan,
        FailureReason::CompositeCapabilityFailed,
    )
    .unwrap();
    if let Some(abnormal) = abnormal {
        for fragment in &mut definition.internal_plan.fragments {
            for front in &mut fragment.fore_ports {
                front.abnormal_kind = Some(abnormal.clone());
            }
        }
        let plan = &definition.internal_plan;
        definition.internal_plan = conduit_core::seal_plan_with_activation_entries(
            conduit_core::PlotIdentity {
                source_document_id: plan.source_document_id.clone(),
                checked_plot_id: plan.checked_plot_id.clone(),
                expanded_plot_id: plan.expanded_plot_id.clone(),
            },
            plan.completion_policy,
            plan.realization_backs.clone(),
            plan.activations.clone(),
            plan.fragments.clone(),
        );
        for front in definition
            .boundary
            .input_fronts
            .iter_mut()
            .chain(definition.boundary.output_fronts.iter_mut())
        {
            front.external_port.abnormal_kind = Some(abnormal.clone());
        }
    }
    definition
}

fn fold_registry() -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry.install(CombineFactory).unwrap();
    registry
}

fn planned_fold(definition: &KernelCompositeDefinition) -> PlannedFoldActivation {
    let sign_budget = definition.internal_plan.fragments.iter().fold(
        SignStorageBudget {
            item_capacity: 0,
            byte_capacity: 0,
        },
        |mut total, fragment| {
            total.item_capacity += fragment.sign_storage_budget.item_capacity;
            total.byte_capacity += fragment.sign_storage_budget.byte_capacity;
            total
        },
    );
    let front = |name| PlannedActivationFront {
        front_port_id: conduit_core::port_id(name),
        value_kind: kind_id(VALUE_KIND),
        abnormal_kind: None,
    };
    PlannedFoldActivation {
        activation_id: "flow/fold-combine".into(),
        owner_placement_id: PlacementId::from("flow/fold"),
        selected_plan_id: definition.internal_plan.plan_id.clone(),
        selected_plan: Box::new(definition.internal_plan.clone()),
        accumulator_input: front("accumulator"),
        item_input: front("item"),
        output: front("combined"),
        initial_accumulator: b"0".to_vec(),
        retained_accumulator_bytes: 16,
        retained_item_bytes: 16,
        limits: PlannedActivationLimits {
            maximum_items: 2,
            maximum_active: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 64,
        },
        terminal_policy: PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
        abnormal_policy: PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
        cancellation_policy: PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
        effect_multiplicity:
            conduit_core::PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
        per_activation_sign_budget: sign_budget,
    }
}

fn run_fold_to_ready(host: &mut BoundedFoldActivationHost) {
    for _ in 0..128 {
        if *host.step().unwrap() == BoundedFoldState::FinalReady {
            return;
        }
    }
    panic!("bounded fold did not drain to its final value")
}

#[test]
fn bounded_fold_empty_emits_initial_exactly_once() {
    let definition = fold_definition();
    let mut host = BoundedFoldActivationHost::prepare(
        &planned_fold(&definition),
        definition,
        &fold_registry(),
    )
    .unwrap();
    let capacities = host.allocation_capacities();
    assert_eq!(capacities, (2, 2));
    host.close_input().unwrap();
    assert_eq!(host.final_value().unwrap(), Some(value(b"0")));
    assert_eq!(host.final_value().unwrap(), None);
}

#[test]
fn bounded_fold_orders_two_items_and_bounds_one_queued_item() {
    let definition = fold_definition();
    let mut host = BoundedFoldActivationHost::prepare(
        &planned_fold(&definition),
        definition,
        &fold_registry(),
    )
    .unwrap();
    let capacities = host.allocation_capacities();
    assert_eq!(capacities, (2, 2));
    assert_eq!(
        host.admit(&value(b"1")).unwrap(),
        conduit_composite::BoundedFoldAdmission::Accepted
    );
    assert_eq!(
        host.admit(&value(b"2")).unwrap(),
        conduit_composite::BoundedFoldAdmission::Accepted
    );
    assert_eq!(
        host.admit(&value(b"3")).unwrap(),
        conduit_composite::BoundedFoldAdmission::Full
    );
    for _ in 0..128 {
        if *host.step().unwrap() == BoundedFoldState::Idle {
            break;
        }
    }
    assert_eq!(host.allocation_capacities(), capacities);
    assert_eq!(
        host.admit(&value(b"3")).unwrap(),
        conduit_composite::BoundedFoldAdmission::MaximumItemsExceeded { maximum_items: 2 }
    );
    host.close_input().unwrap();
    run_fold_to_ready(&mut host);
    assert_eq!(host.final_value().unwrap(), Some(value(b"2")));
    assert_eq!(host.final_value().unwrap(), None);
}

#[test]
fn bounded_fold_abnormal_and_cancel_discard_without_partial_value() {
    let abnormal = kind_id("failure/fold");
    let definition = fold_definition_with_abnormal(Some(abnormal.clone()));
    let mut planned = planned_fold(&definition);
    planned.accumulator_input.abnormal_kind = Some(abnormal.clone());
    planned.item_input.abnormal_kind = Some(abnormal.clone());
    planned.output.abnormal_kind = Some(abnormal.clone());

    let mut host =
        BoundedFoldActivationHost::prepare(&planned, definition.clone(), &fold_registry()).unwrap();
    let one = value(b"1");
    let late = value(b"late");
    let allocations = allocations_during(|| {
        host.admit(&one).unwrap();
        host.cancel().unwrap();
        assert_eq!(*host.step().unwrap(), BoundedFoldState::Cancelled);
        assert_eq!(host.final_value().unwrap(), None);
        assert!(host.admit(&late).is_err());
        assert!(host.next_host_request().is_none());
    });
    assert_eq!(
        allocations, 0,
        "fold cancellation allocated {allocations} times"
    );

    let mut host =
        BoundedFoldActivationHost::prepare(&planned, definition, &fold_registry()).unwrap();
    host.admit(&one).unwrap();
    let terminal = ValuePayload {
        value_kind: abnormal,
        encoded: b"exact".to_vec(),
    };
    let expected = terminal.clone();
    let allocations = allocations_during(|| {
        host.terminate_input(terminal).unwrap();
        assert!(matches!(
            host.step().unwrap(),
            BoundedFoldState::Abnormal(_)
        ));
        assert_eq!(host.final_value().unwrap(), None);
        assert!(host.admit(&late).is_err());
        assert!(host.next_host_request().is_none());
    });
    assert_eq!(
        allocations, 0,
        "fold abnormal allocated {allocations} times"
    );
    assert_eq!(*host.step().unwrap(), BoundedFoldState::Abnormal(expected));
}

fn planned_scan(
    definition: &KernelCompositeDefinition,
    maximum_items: u16,
) -> PlannedScanActivation {
    let fold = planned_fold(definition);
    PlannedScanActivation {
        activation_id: "flow/scan-combine".into(),
        owner_placement_id: PlacementId::from("flow/scan"),
        selected_plan_id: fold.selected_plan_id,
        selected_plan: fold.selected_plan,
        accumulator_input: fold.accumulator_input,
        item_input: fold.item_input,
        output: fold.output,
        initial_accumulator: fold.initial_accumulator,
        retained_accumulator_bytes: fold.retained_accumulator_bytes,
        retained_item_bytes: fold.retained_item_bytes,
        limits: PlannedActivationLimits {
            maximum_items,
            ..fold.limits
        },
        terminal_policy: PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission,
        abnormal_policy: PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
        cancellation_policy: PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission,
        effect_multiplicity: fold.effect_multiplicity,
        per_activation_sign_budget: fold.per_activation_sign_budget,
    }
}

fn drain_scan_output(host: &mut BoundedScanActivationHost) -> ValuePayload {
    for _ in 0..128 {
        if *host.step().unwrap() == BoundedScanState::OutputReady {
            break;
        }
    }
    let mut output = ValuePayload {
        value_kind: kind_id(VALUE_KIND),
        encoded: Vec::with_capacity(16),
    };
    assert!(host.output_into(&mut output).unwrap());
    host.complete_output().unwrap();
    output
}

#[test]
fn bounded_scan_repeated_n_emits_each_progression_without_growth_or_extra_values() {
    let definition = fold_definition();
    let mut host = BoundedScanActivationHost::prepare(
        &planned_scan(&definition, 2),
        definition,
        &fold_registry(),
    )
    .unwrap();
    let pools = host.allocation_capacities();
    let storage = host.storage_capacities();
    assert!(!host.output_into(&mut value(b"")).unwrap());
    assert_eq!(
        host.admit(&value(b"1")).unwrap(),
        BoundedScanAdmission::Accepted
    );
    assert_eq!(drain_scan_output(&mut host), value(b"1"));
    assert_eq!(
        host.admit(&value(b"2")).unwrap(),
        BoundedScanAdmission::Accepted
    );
    assert_eq!(drain_scan_output(&mut host), value(b"2"));
    assert_eq!(host.allocation_capacities(), pools);
    assert_eq!(host.storage_capacities(), storage);
    assert_eq!(
        host.admit(&value(b"3")).unwrap(),
        BoundedScanAdmission::MaximumItemsExceeded { maximum_items: 2 }
    );
    host.close_input().unwrap();
    assert_eq!(*host.step().unwrap(), BoundedScanState::Complete);
    assert!(!host.output_into(&mut value(b"")).unwrap());
}

#[test]
fn bounded_scan_distinguishes_pressure_from_n_plus_one() {
    let definition = fold_definition();
    let mut host = BoundedScanActivationHost::prepare(
        &planned_scan(&definition, 3),
        definition,
        &fold_registry(),
    )
    .unwrap();
    assert_eq!(
        host.admit(&value(b"1")).unwrap(),
        BoundedScanAdmission::Accepted
    );
    assert_eq!(
        host.admit(&value(b"2")).unwrap(),
        BoundedScanAdmission::Accepted
    );
    assert_eq!(
        host.admit(&value(b"3")).unwrap(),
        BoundedScanAdmission::Full
    );
    assert_eq!(drain_scan_output(&mut host), value(b"1"));
    assert_eq!(drain_scan_output(&mut host), value(b"2"));
    assert_eq!(
        host.admit(&value(b"3")).unwrap(),
        BoundedScanAdmission::Accepted
    );
    assert_eq!(drain_scan_output(&mut host), value(b"3"));
    assert_eq!(
        host.admit(&value(b"4")).unwrap(),
        BoundedScanAdmission::MaximumItemsExceeded { maximum_items: 3 }
    );
}

#[test]
fn bounded_scan_abnormal_and_cancel_discard_pending_output() {
    let abnormal = kind_id("failure/fold");
    let definition = fold_definition_with_abnormal(Some(abnormal.clone()));
    let mut planned = planned_scan(&definition, 2);
    planned.accumulator_input.abnormal_kind = Some(abnormal.clone());
    planned.item_input.abnormal_kind = Some(abnormal.clone());
    planned.output.abnormal_kind = Some(abnormal.clone());
    let mut cancelled =
        BoundedScanActivationHost::prepare(&planned, definition.clone(), &fold_registry()).unwrap();
    let one = value(b"1");
    let late = value(b"late");
    let mut empty = ValuePayload {
        value_kind: kind_id(VALUE_KIND),
        encoded: Vec::with_capacity(16),
    };
    let allocations = allocations_during(|| {
        cancelled.admit(&one).unwrap();
        cancelled.cancel().unwrap();
        assert_eq!(*cancelled.step().unwrap(), BoundedScanState::Cancelled);
        assert!(!cancelled.output_into(&mut empty).unwrap());
        assert!(cancelled.admit(&late).is_err());
        assert!(cancelled.next_host_request().is_none());
    });
    assert_eq!(
        allocations, 0,
        "scan cancellation allocated {allocations} times"
    );
    let mut failed =
        BoundedScanActivationHost::prepare(&planned, definition, &fold_registry()).unwrap();
    failed.admit(&one).unwrap();
    let terminal = ValuePayload {
        value_kind: abnormal,
        encoded: b"exact".to_vec(),
    };
    let expected = terminal.clone();
    let allocations = allocations_during(|| {
        failed.terminate_input(terminal).unwrap();
        assert!(matches!(
            failed.step().unwrap(),
            BoundedScanState::Abnormal(value) if value == &expected
        ));
        assert!(!failed.output_into(&mut empty).unwrap());
        assert!(failed.admit(&late).is_err());
        assert!(failed.next_host_request().is_none());
    });
    assert_eq!(
        allocations, 0,
        "scan abnormal allocated {allocations} times"
    );
}

#[test]
fn scan_success_pressure_terminal_cancel_and_late_refusal_are_allocation_free() {
    let definition = fold_definition();
    let mut scan = BoundedScanActivationHost::prepare(
        &planned_scan(&definition, 2),
        definition,
        &fold_registry(),
    )
    .unwrap();
    let one = value(b"1");
    let two = value(b"2");
    let full = value(b"3");
    let mut output = ValuePayload {
        value_kind: kind_id(VALUE_KIND),
        encoded: Vec::with_capacity(16),
    };
    let allocations = allocations_during(|| {
        let mut completed = 0;
        assert_eq!(scan.admit(&one).unwrap(), BoundedScanAdmission::Accepted);
        assert_eq!(scan.admit(&two).unwrap(), BoundedScanAdmission::Accepted);
        assert_eq!(scan.admit(&full).unwrap(), BoundedScanAdmission::Full);
        for _ in 0..128 {
            if *scan.step().unwrap() == BoundedScanState::OutputReady {
                assert!(scan.output_into(&mut output).unwrap());
                scan.complete_output().unwrap();
                output.encoded.clear();
                completed += 1;
            }
            if completed == 2 {
                break;
            }
        }
        scan.close_input().unwrap();
        assert_eq!(*scan.step().unwrap(), BoundedScanState::Complete);
    });
    assert_eq!(allocations, 0, "scan matrix allocated {allocations} times");

    let definition = fold_definition();
    let mut cancelled = BoundedScanActivationHost::prepare(
        &planned_scan(&definition, 2),
        definition,
        &fold_registry(),
    )
    .unwrap();
    let active = value(b"1");
    let late = value(b"late");
    let allocations = allocations_during(|| {
        cancelled.admit(&active).unwrap();
        cancelled.cancel().unwrap();
        assert_eq!(*cancelled.step().unwrap(), BoundedScanState::Cancelled);
        assert!(cancelled.admit(&late).is_err());
    });
    assert_eq!(
        allocations, 0,
        "scan cancellation matrix allocated {allocations} times"
    );
}

#[test]
fn fold_success_pressure_and_normal_completion_are_allocation_free() {
    let definition = fold_definition();
    let mut fold = BoundedFoldActivationHost::prepare(
        &planned_fold(&definition),
        definition,
        &fold_registry(),
    )
    .unwrap();
    let one = value(b"1");
    let two = value(b"2");
    let full = value(b"3");
    let mut final_value = ValuePayload {
        value_kind: kind_id(VALUE_KIND),
        encoded: Vec::with_capacity(16),
    };
    let allocations = allocations_during(|| {
        assert_eq!(
            fold.admit(&one).unwrap(),
            conduit_composite::BoundedFoldAdmission::Accepted
        );
        assert_eq!(
            fold.admit(&two).unwrap(),
            conduit_composite::BoundedFoldAdmission::Accepted
        );
        assert_eq!(
            fold.admit(&full).unwrap(),
            conduit_composite::BoundedFoldAdmission::Full
        );
        for _ in 0..128 {
            if *fold.step().unwrap() == BoundedFoldState::Idle {
                break;
            }
        }
        fold.close_input().unwrap();
        run_fold_to_ready(&mut fold);
        assert!(fold.final_value_into(&mut final_value).unwrap());
        assert!(!fold.final_value_into(&mut final_value).unwrap());
    });
    assert_eq!(allocations, 0, "fold matrix allocated {allocations} times");
}

fn run_until_output(host: &mut KernelCompositeHost) -> (u64, ValuePayload) {
    for _ in 0..64 {
        host.step().unwrap();
        if let Some(output) = host.output(&conduit_core::port_id("output")).unwrap() {
            return output;
        }
    }
    panic!("kernel composite did not produce its exact boundary output")
}

#[test]
fn success_preserves_two_child_kernel_delivery_and_terminal_propagation() {
    let definition = definition();
    let expected_plan = definition.internal_plan.plan_id.clone();
    let expected_children = definition
        .internal_plan
        .fragments
        .iter()
        .map(|fragment| (fragment.host_id.clone(), fragment.boot_id.clone()))
        .collect::<Vec<_>>();
    let expected_plays = definition
        .internal_plan
        .fragments
        .iter()
        .map(|fragment| {
            (
                fragment.host_id.clone(),
                conduit_core::bind_active_play(
                    &fragment.plan_id,
                    &fragment.host_id,
                    &fragment.boot_id,
                    0,
                )
                .active_play_id,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut host = KernelCompositeHost::prepare(definition, &registry()).unwrap();
    let transfer_capacities = host.internal_transfer_capacities();
    assert_eq!(host.definition().internal_plan.plan_id, expected_plan);
    assert_eq!(
        host.definition()
            .internal_plan
            .fragments
            .iter()
            .map(|fragment| (fragment.host_id.clone(), fragment.boot_id.clone()))
            .collect::<Vec<_>>(),
        expected_children
    );
    let plays = host.start().unwrap();
    assert_eq!(plays, &expected_plays);
    assert!(matches!(
        host.admit_input(&conduit_core::port_id("input"), 0, &value(b"exact")),
        Ok(conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 })
    ));
    let (sequence, output) = run_until_output(&mut host);
    assert_eq!(host.internal_transfer_capacities(), transfer_capacities);
    assert_eq!((sequence, output), (0, value(b"exact")));
    host.complete_output(&conduit_core::port_id("output"), sequence)
        .unwrap();
    host.close_input(&conduit_core::port_id("input")).unwrap();
    for _ in 0..64 {
        if host.step().unwrap() == KernelCompositeStatus::Complete {
            assert_eq!(host.internal_transfer_capacities(), transfer_capacities);
            assert!(host.signs().values().all(|events| !events.is_empty()));
            return;
        }
    }
    panic!("kernel composite did not propagate terminal closure")
}

#[test]
fn pressure_is_finite_and_retry_keeps_the_exact_sequence() {
    let mut host = KernelCompositeHost::prepare(definition(), &registry()).unwrap();
    host.start().unwrap();
    assert!(matches!(
        host.admit_input(&conduit_core::port_id("input"), 0, &value(b"12345678")),
        Ok(conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. })
    ));
    assert!(matches!(
        host.admit_input(&conduit_core::port_id("input"), 1, &value(b"abcdefgh")),
        Ok(conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. })
    ));
    assert!(matches!(
        host.admit_input(&conduit_core::port_id("input"), 2, &value(b"blocked")),
        Ok(conduit_kernel::scheduler::RemoteIngressOutcome::Full { sequence: 2 })
    ));
    let (sequence, _) = run_until_output(&mut host);
    host.complete_output(&conduit_core::port_id("output"), sequence)
        .unwrap();
    for _ in 0..8 {
        host.step().unwrap();
    }
    assert!(matches!(
        host.admit_input(&conduit_core::port_id("input"), 2, &value(b"blocked")),
        Ok(conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 2 })
    ));
}

#[test]
fn direct_composite_successful_play_allocates_nothing_after_preparation() {
    let mut host = KernelCompositeHost::prepare(definition(), &registry()).unwrap();
    let input_port = conduit_core::port_id("input");
    let output_port = conduit_core::port_id("output");
    let input = value(b"exact");
    let mut output = value(b"12345678");
    output.encoded.clear();

    let allocations = allocations_during(|| {
        host.start().unwrap();
        assert!(matches!(
            host.admit_input(&input_port, 0, &input).unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 }
        ));
        let sequence = loop {
            host.step().unwrap();
            if let Some(sequence) = host.output_into(&output_port, &mut output).unwrap() {
                break sequence;
            }
        };
        host.complete_output(&output_port, sequence).unwrap();
    });
    assert_eq!(allocations, 0, "play allocated {allocations} times");
    assert_eq!(output.encoded, b"exact");
}

#[test]
fn direct_composite_pressure_and_retry_allocate_nothing_during_play() {
    let mut host = KernelCompositeHost::prepare(definition(), &registry()).unwrap();
    let input_port = conduit_core::port_id("input");
    let first = value(b"12345678");
    let second = value(b"abcdefgh");
    let blocked = value(b"blocked");
    host.start().unwrap();

    let allocations = allocations_during(|| {
        assert!(matches!(
            host.admit_input(&input_port, 0, &first).unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 }
        ));
        assert!(matches!(
            host.admit_input(&input_port, 1, &second).unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 1 }
        ));
        assert!(matches!(
            host.admit_input(&input_port, 2, &blocked).unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Full { sequence: 2 }
        ));
    });
    assert_eq!(
        allocations, 0,
        "pressure path allocated {allocations} times"
    );
}

#[test]
fn child_refusal_is_distinct_and_occurs_before_play() {
    assert!(matches!(
        KernelCompositeHost::prepare(definition(), &KernelOperationRegistry::new()),
        Err(KernelCompositeError::ChildRefused { .. })
    ));
}

#[test]
fn child_failure_is_a_machine_readable_kernel_execution_terminal() {
    let mut failed = KernelCompositeHost::prepare(definition(), &failing_registry()).unwrap();
    failed.start().unwrap();
    assert!(matches!(
        failed.step(),
        Err(KernelCompositeError::Execution { .. })
    ));
}

#[test]
fn stale_child_identity_refuses_before_any_kernel_is_started() {
    let mut stale = definition();
    stale.boundary.input_fronts[0].internal_child = HostId::from("stale-child");
    assert!(matches!(
        KernelCompositeHost::prepare(stale, &registry()),
        Err(KernelCompositeError::StaleChild(_))
    ));
}

#[test]
fn malformed_boundary_binding_and_value_kind_refuse_distinctly() {
    let mut malformed = definition();
    malformed.boundary.input_fronts[0].internal_port_id = conduit_core::port_id("missing");
    assert!(matches!(
        KernelCompositeHost::prepare(malformed, &registry()),
        Err(KernelCompositeError::InvalidBoundary(_))
    ));

    let mut host = KernelCompositeHost::prepare(definition(), &registry()).unwrap();
    assert_eq!(host.step(), Err(KernelCompositeError::InvalidLifecycle));
    host.start().unwrap();
    assert!(matches!(
        host.admit_input(
            &conduit_core::port_id("input"),
            0,
            &ValuePayload {
                value_kind: kind_id("value/wrong"),
                encoded: vec![1],
            }
        ),
        Err(KernelCompositeError::MalformedBoundary(_))
    ));
}

#[test]
fn cancellation_is_terminal_and_rejects_late_kernel_work() {
    let mut host = KernelCompositeHost::prepare(definition(), &registry()).unwrap();
    let input_port = conduit_core::port_id("input");
    let late = value(b"late");
    host.start().unwrap();
    let allocations = allocations_during(|| {
        host.cancel().unwrap();
        assert_eq!(host.step().unwrap(), KernelCompositeStatus::Cancelled);
        assert!(matches!(
            host.admit_input(&input_port, 0, &late),
            Err(KernelCompositeError::InvalidLifecycle)
        ));
    });
    assert_eq!(allocations, 0, "cancellation allocated {allocations} times");
}

#[test]
fn bounded_activation_owes_one_fresh_exact_execution_per_accepted_value() {
    let mut activations = BoundedActivationHost::prepare(
        definition(),
        &registry(),
        conduit_core::port_id("input"),
        conduit_core::port_id("output"),
        2,
    )
    .unwrap();
    assert_eq!(activations.contract().maximum_active, 1);
    assert_eq!(activations.contract().maximum_queue_items, 1);
    assert_eq!(activations.contract().input_value_kind, kind_id(VALUE_KIND));
    assert_eq!(activations.contract().input_abnormal_kind, None);
    assert_eq!(
        activations.contract().output_value_kind,
        kind_id(VALUE_KIND)
    );
    assert_eq!(activations.contract().output_abnormal_kind, None);
    let capacities = activations.allocation_capacities();
    assert_eq!(capacities, (2, 2));

    assert_eq!(
        activations.activate(7, &value(b"first")).unwrap(),
        BoundedActivationAdmission::Accepted { sequence: 7 }
    );
    assert_eq!(
        activations.activate(8, &value(b"second")).unwrap(),
        BoundedActivationAdmission::Full { sequence: 8 }
    );
    for _ in 0..64 {
        activations.step().unwrap();
        if let Some((sequence, output)) = activations.output().unwrap() {
            assert_eq!((sequence, output), (7, &value(b"first")));
            activations.complete_output(sequence).unwrap();
            break;
        }
    }
    for _ in 0..64 {
        if matches!(
            activations.step().unwrap(),
            BoundedActivationState::Succeeded { sequence: 7 }
        ) {
            break;
        }
    }
    assert_eq!(
        activations.activate(8, &value(b"second")).unwrap(),
        BoundedActivationAdmission::Accepted { sequence: 8 }
    );
    for _ in 0..64 {
        activations.step().unwrap();
        if let Some((sequence, output)) = activations.output().unwrap() {
            assert_eq!((sequence, output), (8, &value(b"second")));
            activations.complete_output(sequence).unwrap();
            break;
        }
    }
    for _ in 0..64 {
        if matches!(
            activations.step().unwrap(),
            BoundedActivationState::Succeeded { sequence: 8 }
        ) {
            break;
        }
    }
    assert_eq!(activations.allocation_capacities(), capacities);
    assert_eq!(
        activations.activate(9, &value(b"overflow")).unwrap(),
        BoundedActivationAdmission::MaximumItemsExceeded {
            sequence: 9,
            maximum_items: 2,
        }
    );
    assert_eq!(activations.allocation_capacities(), capacities);
}

fn planned_activation(definition: &KernelCompositeDefinition) -> PlannedActivation {
    let sign_budget = definition.internal_plan.fragments.iter().fold(
        SignStorageBudget {
            item_capacity: 0,
            byte_capacity: 0,
        },
        |mut total, fragment| {
            total.item_capacity += fragment.sign_storage_budget.item_capacity;
            total.byte_capacity += fragment.sign_storage_budget.byte_capacity;
            total
        },
    );
    PlannedActivation {
        activation_id: "flow/each-transform".into(),
        owner_placement_id: PlacementId::from("flow/each"),
        selected_plan_id: definition.internal_plan.plan_id.clone(),
        selected_plan: Box::new(definition.internal_plan.clone()),
        input: PlannedActivationFront {
            front_port_id: conduit_core::port_id("input"),
            value_kind: kind_id(VALUE_KIND),
            abnormal_kind: None,
        },
        output: PlannedActivationFront {
            front_port_id: conduit_core::port_id("output"),
            value_kind: kind_id(VALUE_KIND),
            abnormal_kind: None,
        },
        limits: PlannedActivationLimits {
            maximum_items: 2,
            maximum_active: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 16,
        },
        terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
        cancellation_policy:
            PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
        effect_multiplicity:
            conduit_core::PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
        per_activation_sign_budget: sign_budget,
    }
}

#[test]
fn bounded_activation_prepares_only_the_exact_planned_subgraph() {
    let definition = definition();
    let planned = planned_activation(&definition);
    let prepared =
        BoundedActivationHost::prepare_planned(&planned, definition.clone(), &registry()).unwrap();
    assert_eq!(
        prepared.contract().selected_plan_id,
        planned.selected_plan_id
    );

    let mut widened = planned.clone();
    widened.limits.maximum_queue_bytes += 1;
    assert!(matches!(
        BoundedActivationHost::prepare_planned(&widened, definition, &registry()),
        Err(conduit_composite::BoundedActivationError::PlannedContractMismatch)
    ));
}

#[test]
fn bounded_activation_fault_and_cancellation_are_not_success() {
    let mut failed = BoundedActivationHost::prepare(
        definition(),
        &failing_registry(),
        conduit_core::port_id("input"),
        conduit_core::port_id("output"),
        2,
    )
    .unwrap();
    failed.activate(3, &value(b"fault")).unwrap();
    assert!(matches!(
        failed.step().unwrap(),
        BoundedActivationState::Faulted { sequence: 3, .. }
    ));

    let mut cancelled = BoundedActivationHost::prepare(
        definition(),
        &registry(),
        conduit_core::port_id("input"),
        conduit_core::port_id("output"),
        2,
    )
    .unwrap();
    let active = value(b"cancel");
    let late = value(b"late");
    let allocations = allocations_during(|| {
        cancelled.activate(4, &active).unwrap();
        cancelled.cancel().unwrap();
        assert_eq!(
            cancelled.state(),
            &BoundedActivationState::Cancelled { sequence: Some(4) }
        );
        assert!(cancelled.activate(5, &late).is_err());
    });
    assert_eq!(
        allocations, 0,
        "each cancellation allocated {allocations} times"
    );
}

#[test]
fn bounded_activation_drains_one_owed_value_before_normal_close() {
    let mut each = BoundedActivationHost::prepare(
        definition(),
        &registry(),
        conduit_core::port_id("input"),
        conduit_core::port_id("output"),
        2,
    )
    .unwrap();
    let owed = value(b"owed");
    let late = value(b"late");
    let allocations = allocations_during(|| {
        assert_eq!(
            each.activate(11, &owed).unwrap(),
            BoundedActivationAdmission::Accepted { sequence: 11 }
        );
        each.close_input().unwrap();
        assert_eq!(
            each.activate(12, &late),
            Err(conduit_composite::BoundedActivationError::InvalidLifecycle)
        );

        for _ in 0..64 {
            each.step().unwrap();
            if let Some((sequence, output)) = each.output().unwrap() {
                assert_eq!(sequence, 11);
                assert_eq!(output.encoded, b"owed");
                each.complete_output(sequence).unwrap();
                break;
            }
        }
        for _ in 0..64 {
            if matches!(
                each.step().unwrap(),
                BoundedActivationState::Succeeded { sequence: 11 }
            ) {
                break;
            }
        }
        assert_eq!(each.step().unwrap(), &BoundedActivationState::Drained);
    });
    assert_eq!(
        allocations, 0,
        "each completion allocated {allocations} times"
    );
}
