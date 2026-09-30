use conduit_composite::{
    FlowSelectAdmission, FlowSelectCoordinator, FlowSelectState, KernelCompositeDefinition,
    KernelOperationBudget, KernelOperationFactory, KernelOperationRegistry,
};
use conduit_core::{
    kind_id, ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits,
    FailureReason, GearId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    KindIdentity, OfferGeneration, PlacementId, PlannedActivation,
    PlannedActivationCancellationPolicy, PlannedActivationEffectMultiplicity,
    PlannedActivationFront, PlannedActivationLimits, PlannedActivationTerminalPolicy, PlannedGear,
    PortDescriptor, PortDirection, PortTemporal, SignStorageBudget, ValuePayload, BOOL_INFO_ID,
    PROTOCOL_VERSION,
};
use conduit_form::{parse, KindProjection, ProfileCatalog};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{HostedValueStore, PortId as KernelPortId, ValueRef, ValueStorage};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use std::collections::BTreeMap;

const VALUE_KIND: &str = "value/bytes";
const PREDICATE_KIND: &str = "test/kernel-composite-predicate";
const PREDICATE_IMPLEMENTATION: &str = "test/kernel-composite-predicate-v1";

fn port(name: &str, kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

fn definition() -> KernelCompositeDefinition {
    let input = port("in", VALUE_KIND, PortDirection::Input);
    let output = port("accepted", BOOL_INFO_ID, PortDirection::Output);
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindProjection {
            kind_id: kind_id(PREDICATE_KIND),
            kind_contract_revision: KindIdentity::from("test/kernel-composite-predicate@1"),
            inputs: vec![input.clone()],
            outputs: vec![output.clone()],
            configuration: vec![],
        })
        .unwrap();
    let form = parse(
        "form test/predicate (\n >> input: value/bytes\n output: Boolean >>\n) {\n predicate: test/kernel-composite-predicate\n input >> predicate.in\n predicate.accepted >> output\n}\n",
        &catalog,
    )
    .unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("predicate-child"),
        boot_id: BootId::from("predicate-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/predicate"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![conduit_core::capability_offer_from_parts! {
            semantic_contract: Default::default(),
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from("predicate"),
            kind_id: kind_id(PREDICATE_KIND),
            kind_contract_revision: KindIdentity::from("test/kernel-composite-predicate@1"),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: "test/predicate@1".into(),
                implementation_id: PREDICATE_IMPLEMENTATION.into(),
                artifact_id: "test/predicate-artifact@1".into(),
            },
            inputs: vec![input],
            outputs: vec![output],
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 2, max_queue_bytes: 16 },
        }],
    };
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([(
            GearId::from("test/predicate/predicate"),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: CapabilityId::from("predicate"),
            },
        )]),
    };
    let plan = plan_with_line_offers(
        &form,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        2,
        16,
        &[],
    )
    .unwrap();
    KernelCompositeDefinition::from_authored_export(
        HostId::from("predicate-composite"),
        BootId::from("predicate-composite-boot"),
        OfferGeneration(1),
        HostProfileId::from("composite/predicate"),
        ImplementationId::from("composite/predicate-v1"),
        ArtifactId::from("composite/predicate-artifact-v1"),
        &form,
        &CapabilityId::from("run"),
        plan,
        FailureReason::CompositeCapabilityFailed,
    )
    .unwrap()
}

struct Predicate {
    false_value: ValueRef,
    true_value: ValueRef,
}

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for Predicate {
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        input_bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        if io.input(KernelPortId(0)).is_some() {
            if !io.output_ready(KernelPortId(0)) {
                return StepOutcome::Await;
            }
            let accepted = input_bytes
                .input(KernelPortId(0))
                .is_some_and(|bytes| bytes.first() == Some(&b't'));
            let output = if accepted {
                self.true_value
            } else {
                self.false_value
            };
            if io.consume(KernelPortId(0)).is_err() || io.send(KernelPortId(0), output).is_err() {
                return StepOutcome::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidLifecycle,
                    detail: 1,
                });
            }
            StepOutcome::Progress
        } else if io.input_closed(KernelPortId(0)) {
            if io.consume_closed(KernelPortId(0)).is_err() {
                return StepOutcome::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidLifecycle,
                    detail: 2,
                });
            }
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}

struct PredicateFactory(ImplementationId);

impl KernelOperationFactory for PredicateFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.0
    }

    fn budget(&self, _placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 2,
            value_bytes: 2,
            maximum_value_bytes: 16,
            host_requests: 0,
            sign_items: 8,
        })
    }

    fn prepare(
        &self,
        _placement: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        Ok(Box::new(Predicate {
            false_value: values.store(&[0]).map_err(|error| format!("{error:?}"))?,
            true_value: values.store(&[1]).map_err(|error| format!("{error:?}"))?,
        }))
    }
}

fn registry() -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(PredicateFactory(PREDICATE_IMPLEMENTATION.into()))
        .unwrap();
    registry
}

fn planned(definition: &KernelCompositeDefinition) -> PlannedActivation {
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
        activation_id: "flow/select-predicate".into(),
        owner_placement_id: PlacementId::from("flow/select"),
        selected_plan_id: definition.internal_plan.plan_id.clone(),
        selected_plan: Box::new(definition.internal_plan.clone()),
        input: PlannedActivationFront {
            front_port_id: "input".into(),
            value_kind: kind_id(VALUE_KIND),
            abnormal_kind: None,
        },
        output: PlannedActivationFront {
            front_port_id: "output".into(),
            value_kind: kind_id(BOOL_INFO_ID),
            abnormal_kind: None,
        },
        limits: PlannedActivationLimits {
            maximum_items: 2,
            maximum_active: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: definition.external_capability.limits.max_queue_bytes,
        },
        terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
        cancellation_policy:
            PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
        effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
        per_activation_sign_budget: sign_budget,
    }
}

fn value(bytes: &[u8]) -> ValuePayload {
    ValuePayload {
        value_kind: kind_id(VALUE_KIND),
        encoded: bytes.to_vec(),
    }
}

fn coordinator() -> FlowSelectCoordinator {
    let definition = definition();
    FlowSelectCoordinator::prepare_planned(&planned(&definition), definition, &registry()).unwrap()
}

#[test]
fn emits_original_once_for_true_and_nothing_for_false() {
    let mut select = coordinator();
    select.admit(1, value(b"true-original")).unwrap();
    for _ in 0..64 {
        select.step().unwrap();
        if let Some((sequence, selected)) = select.output() {
            assert_eq!((sequence, selected), (1, &value(b"true-original")));
            select.complete_output(sequence).unwrap();
            break;
        }
    }
    assert_eq!(select.state(), &FlowSelectState::Idle);
    select.admit(2, value(b"false-original")).unwrap();
    for _ in 0..64 {
        select.step().unwrap();
        if select.state() == &FlowSelectState::Idle {
            break;
        }
    }
    assert_eq!(select.output(), None);
}

#[test]
fn one_active_one_queued_drain_before_close() {
    let mut select = coordinator();
    assert_eq!(
        select.admit(10, value(b"true-first")).unwrap(),
        FlowSelectAdmission::Accepted { sequence: 10 }
    );
    assert_eq!(
        select.admit(11, value(b"false-second")).unwrap(),
        FlowSelectAdmission::Accepted { sequence: 11 }
    );
    assert_eq!(
        select.admit(12, value(b"true-full")).unwrap(),
        FlowSelectAdmission::Full { sequence: 12 }
    );
    select.close_input().unwrap();
    assert!(select.admit(13, value(b"true-late")).is_err());
    for _ in 0..128 {
        select.step().unwrap();
        if let Some((sequence, selected)) = select.output() {
            assert_eq!((sequence, selected), (10, &value(b"true-first")));
            select.complete_output(sequence).unwrap();
        }
        if select.state() == &FlowSelectState::Drained {
            return;
        }
    }
    panic!("select did not drain admitted work")
}

#[test]
fn cancellation_drops_retained_work_and_rejects_late_completion() {
    let mut select = coordinator();
    select.admit(20, value(b"true-active")).unwrap();
    select.admit(21, value(b"true-queued")).unwrap();
    select.cancel().unwrap();
    assert_eq!(
        select.state(),
        &FlowSelectState::Cancelled { sequence: Some(20) }
    );
    assert!(select.complete_output(20).is_err());
    assert!(select.admit(22, value(b"true-late")).is_err());
}
