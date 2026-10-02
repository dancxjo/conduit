use conduit_composite::{
    AdmittedKernelCompositeHostRequest, BoundedActivationHost, BoundedFoldActivationHost,
    BoundedScanActivationHost, FlowSelectCoordinator, KernelCompositeDefinition,
    KernelCompositeHostRequest, KernelOperationBudget, KernelOperationFactory,
    KernelOperationRegistry, PlannedActivationChildPoolHost, PreparedActivationChildPool,
    PreparedPlannedActivationComposite,
};
use conduit_core::{
    prepare_plan_on_hosts, ActivePlayId, HostPreparationRefusal, Plan, PlanFragment,
    PlanPreparationHost, PreparationHostIdentity, PreparedFragmentReceipt,
};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{BoundedValueRef, HostCallId, HostedValueStore, PortId, RequestId};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[path = "support/allocation.rs"]
mod allocation;
use allocation::allocations_during;

#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

struct Host {
    identity: PreparationHostIdentity,
    receipts: Vec<PreparedFragmentReceipt>,
    registry: KernelOperationRegistry,
    substituted_definition: Option<KernelCompositeDefinition>,
}

impl Host {
    fn new() -> Self {
        let fragment = common::fragment();
        Self {
            identity: PreparationHostIdentity {
                host_id: fragment.host_id,
                boot_id: fragment.boot_id,
                offer_generation: fragment.offer_generation,
            },
            receipts: vec![],
            registry: KernelOperationRegistry::new(),
            substituted_definition: None,
        }
    }
}

impl PlannedActivationChildPoolHost for Host {
    fn take_activation_child_pool(
        &mut self,
        receipts: &[PreparedFragmentReceipt],
        definition: &KernelCompositeDefinition,
        maximum_items: usize,
    ) -> Result<Vec<conduit_composite::KernelCompositeHost>, HostPreparationRefusal> {
        for receipt in receipts {
            if !self.receipts.contains(receipt) {
                return Err(HostPreparationRefusal::PreparedBindingMismatch);
            }
        }
        let child_definition = self.substituted_definition.as_ref().unwrap_or(definition);
        let ready = (0..maximum_items)
            .map(|_| {
                conduit_composite::KernelCompositeHost::prepare(
                    child_definition.clone(),
                    &self.registry,
                )
                .map_err(|_| HostPreparationRefusal::ImplementationUnavailable)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for receipt in receipts {
            let index = self
                .receipts
                .iter()
                .position(|candidate| candidate == receipt)
                .unwrap();
            self.receipts.remove(index);
        }
        Ok(ready)
    }
}

impl PlanPreparationHost for Host {
    fn preparation_identity(&self) -> PreparationHostIdentity {
        self.identity.clone()
    }
    fn prepare_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<PreparedFragmentReceipt, HostPreparationRefusal> {
        let receipt = PreparedFragmentReceipt::new(fragment);
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }
    fn release_fragment(
        &mut self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        let index = self
            .receipts
            .iter()
            .position(|candidate| candidate == receipt)
            .ok_or(HostPreparationRefusal::PreparedBindingMismatch)?;
        self.receipts.remove(index);
        Ok(())
    }
    fn validate_start(
        &self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        self.receipts
            .contains(receipt)
            .then_some(())
            .ok_or(HostPreparationRefusal::PreparedBindingMismatch)
    }
    fn start_fragment(&mut self, _: &PreparedFragmentReceipt) -> ActivePlayId {
        ActivePlayId::from("play")
    }
}

struct PreparedFactory(Arc<AtomicUsize>);
struct PlainFactory;

impl KernelOperationFactory for PlainFactory {
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
        _: &conduit_core::PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        Ok(Box::new(PreparedBack))
    }
}

impl KernelOperationFactory for PreparedFactory {
    fn implementation_id(&self) -> &conduit_core::ImplementationId {
        static ID: std::sync::OnceLock<conduit_core::ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| conduit_core::ImplementationId::from("state@1"))
    }
    fn budget(
        &self,
        placement: &conduit_core::PlannedGear,
    ) -> Result<KernelOperationBudget, String> {
        if placement.host_calls.len() != 1
            || placement.resources.len() != 1
            || placement.authority.len() != 1
            || placement.authority[0].host_call_contract_id != placement.host_calls[0].contract_id
        {
            return Err("exact Host Call/resource/authority obligations were not carried".into());
        }
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: 1,
            maximum_value_bytes: 1,
            host_requests: 1,
            sign_items: 1,
        })
    }
    fn prepare(
        &self,
        _: &conduit_core::PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(PreparedBack))
    }
}

struct PreparedBack;
impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for PreparedBack {
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).unwrap();
            io.send(PortId(0), value).unwrap();
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).unwrap();
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}

struct HostCallFactory;

impl KernelOperationFactory for HostCallFactory {
    fn implementation_id(&self) -> &conduit_core::ImplementationId {
        static ID: std::sync::OnceLock<conduit_core::ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| conduit_core::ImplementationId::from("state@1"))
    }

    fn budget(&self, _: &conduit_core::PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 2,
            value_bytes: 2,
            maximum_value_bytes: 1,
            host_requests: 1,
            sign_items: 8,
        })
    }

    fn prepare(
        &self,
        _: &conduit_core::PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        Ok(Box::new(HostCallBack { pending: false }))
    }
}

struct HostCallBack {
    pending: bool,
}

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for HostCallBack {
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        if self.pending {
            if let Some(completion) = io.host_completion() {
                let output = completion.1.output;
                io.consume_host_completion().expect("present completion");
                if let Some(output) = output {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.send(PortId(0), output.value).is_err() {
                        return StepOutcome::Fail(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::InvalidLifecycle,
                            detail: 3,
                        });
                    }
                }
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        let input = BoundedValueRef::new(value, 1).expect("one-byte fixture input");
        io.consume(PortId(0)).expect("present fixture input");
        if io.input(PortId(1)).is_some() {
            io.consume(PortId(1)).expect("present fixture item");
        }
        io.request_host_call(RequestId(0), HostCallId(0), input)
            .expect("planned fixture Host Call");
        self.pending = true;
        StepOutcome::Progress
    }
}

fn activation_plan() -> Plan {
    let mut child_fragment = common::fragment();
    let bool_kind = conduit_core::kind_id(conduit_core::BOOL_INFO_ID);
    child_fragment.states[0].value_kind = bool_kind.clone();
    child_fragment.placements[0].inputs[0].value_kind = bool_kind.clone();
    child_fragment.placements[0].outputs[0].value_kind = bool_kind.clone();
    child_fragment.states.clear();
    child_fragment.expected_sign = vec![
        conduit_core::ExpectedSign::PlanFragmentReceived,
        conduit_core::ExpectedSign::PlanTerminal,
    ];
    child_fragment.sign_storage_budget =
        conduit_core::mandatory_sign_storage_requirement(&child_fragment.expected_sign).unwrap();
    child_fragment.placements[0].host_calls = vec![conduit_core::HostCallRequirement {
        contract_id: conduit_core::HostCallContractId::from("fixture/call@1"),
        target_kind: Some(conduit_core::kind_id("fixture/subject")),
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: 1,
    }];
    child_fragment.placements[0].resources = vec![conduit_core::ResourceBinding {
        pool_id: conduit_core::ResourcePoolId::from("fixture/pool"),
        class_id: conduit_core::ResourceClassId::from("fixture/class"),
        units: 1,
        protected: None,
        compute: None,
        content: None,
    }];
    child_fragment.placements[0].authority = vec![conduit_core::AuthorityBinding {
        grant_id: conduit_core::AuthorityGrantId::from("fixture/grant"),
        contract_id: conduit_core::AuthorityContractId::from("fixture/authority@1"),
        host_call_contract_id: conduit_core::HostCallContractId::from("fixture/call@1"),
        subject_kind: conduit_core::kind_id("fixture/subject"),
        host_id: conduit_core::HostId::from("host"),
        boot_id: conduit_core::BootId::from("boot"),
        capability_id: conduit_core::CapabilityId::from("state"),
    }];
    child_fragment.fore_ports = vec![
        conduit_core::PlannedForePort {
            front_port_id: conduit_core::port_id("in"),
            direction: conduit_core::PortDirection::Input,
            placement_id: conduit_core::PlacementId::from("placement"),
            gear_port_id: conduit_core::port_id("next"),
            value_kind: bool_kind.clone(),
            value_contract: None,
            abnormal_kind: None,
            track: conduit_core::ConnectionTrack::Payload,
            temporal: conduit_core::PortTemporal::Value,
            pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
        },
        conduit_core::PlannedForePort {
            front_port_id: conduit_core::port_id("out"),
            direction: conduit_core::PortDirection::Output,
            placement_id: conduit_core::PlacementId::from("placement"),
            gear_port_id: conduit_core::port_id("current"),
            value_kind: bool_kind.clone(),
            value_contract: None,
            abnormal_kind: None,
            track: conduit_core::ConnectionTrack::Payload,
            temporal: conduit_core::PortTemporal::Value,
            pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
        },
    ];
    let child = common::seal(child_fragment);
    let child_sign_budget = child.fragments[0].sign_storage_budget;
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
            activation_id: "each".into(),
            owner_placement_id: conduit_core::PlacementId::from("placement"),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            input: conduit_core::PlannedActivationFront {
                front_port_id: conduit_core::port_id("in"),
                value_kind: bool_kind.clone(),
                abnormal_kind: None,
            },
            output: conduit_core::PlannedActivationFront {
                front_port_id: conduit_core::port_id("out"),
                value_kind: bool_kind,
                abnormal_kind: None,
            },
            limits: conduit_core::PlannedActivationLimits {
                maximum_active: 1,
                maximum_queue_items: 1,
                maximum_queue_bytes: 1,
                maximum_items: 2,
            },
            terminal_policy:
                conduit_core::PlannedActivationTerminalPolicy::DrainThenPropagateExact,
            cancellation_policy:
                conduit_core::PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
            effect_multiplicity:
                conduit_core::PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: child_sign_budget,
        }],
        vec![outer],
    )
}

fn fold_or_scan_plan(scan: bool) -> Plan {
    let mut child_fragment = common::fragment();
    child_fragment.states.clear();
    let mut item = child_fragment.placements[0].inputs[0].clone();
    item.port_id = conduit_core::port_id("item");
    child_fragment.placements[0].inputs.push(item);
    child_fragment.expected_sign = vec![
        conduit_core::ExpectedSign::PlanFragmentReceived,
        conduit_core::ExpectedSign::PlanTerminal,
    ];
    child_fragment.sign_storage_budget =
        conduit_core::mandatory_sign_storage_requirement(&child_fragment.expected_sign).unwrap();
    install_host_call_obligation(&mut child_fragment);
    let front = |name: &str, direction, gear_port: &str| conduit_core::PlannedForePort {
        front_port_id: conduit_core::port_id(name),
        direction,
        placement_id: conduit_core::PlacementId::from("placement"),
        gear_port_id: conduit_core::port_id(gear_port),
        value_kind: conduit_core::kind_id("fixture/byte@1"),
        value_contract: None,
        abnormal_kind: None,
        track: conduit_core::ConnectionTrack::Payload,
        temporal: conduit_core::PortTemporal::Value,
        pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
    };
    child_fragment.fore_ports = vec![
        front("accumulator", conduit_core::PortDirection::Input, "next"),
        front("item", conduit_core::PortDirection::Input, "item"),
        front("combined", conduit_core::PortDirection::Output, "current"),
    ];
    let child = common::seal(child_fragment);
    let sign_budget = child.fragments[0].sign_storage_budget;
    let activation_front = |name| conduit_core::PlannedActivationFront {
        front_port_id: conduit_core::port_id(name),
        value_kind: conduit_core::kind_id("fixture/byte@1"),
        abnormal_kind: None,
    };
    let limits = conduit_core::PlannedActivationLimits {
        maximum_active: 1,
        maximum_queue_items: 1,
        maximum_queue_bytes: 4,
        maximum_items: 2,
    };
    let entry = if scan {
        conduit_core::PlannedActivationEntry::Scan(conduit_core::PlannedScanActivation {
            activation_id: "scan".into(),
            owner_placement_id: conduit_core::PlacementId::from("placement"),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            accumulator_input: activation_front("accumulator"),
            item_input: activation_front("item"),
            output: activation_front("combined"),
            initial_accumulator: vec![0],
            retained_accumulator_bytes: 1,
            retained_item_bytes: 1,
            limits,
            terminal_policy:
                conduit_core::PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission,
            abnormal_policy:
                conduit_core::PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
            cancellation_policy:
                conduit_core::PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission,
            effect_multiplicity:
                conduit_core::PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: sign_budget,
        })
    } else {
        conduit_core::PlannedActivationEntry::Fold(conduit_core::PlannedFoldActivation {
            activation_id: "fold".into(),
            owner_placement_id: conduit_core::PlacementId::from("placement"),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            accumulator_input: activation_front("accumulator"),
            item_input: activation_front("item"),
            output: activation_front("combined"),
            initial_accumulator: vec![0],
            retained_accumulator_bytes: 1,
            retained_item_bytes: 1,
            limits,
            terminal_policy:
                conduit_core::PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
            abnormal_policy:
                conduit_core::PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
            cancellation_policy:
                conduit_core::PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
            effect_multiplicity:
                conduit_core::PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
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
        vec![entry],
        vec![outer],
    )
}

fn install_host_call_obligation(fragment: &mut PlanFragment) {
    fragment.placements[0].host_calls = vec![conduit_core::HostCallRequirement {
        contract_id: conduit_core::HostCallContractId::from("fixture/call@1"),
        target_kind: Some(conduit_core::kind_id("fixture/subject")),
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: 1,
    }];
    fragment.placements[0].resources = vec![conduit_core::ResourceBinding {
        pool_id: conduit_core::ResourcePoolId::from("fixture/pool"),
        class_id: conduit_core::ResourceClassId::from("fixture/class"),
        units: 1,
        protected: None,
        compute: None,
        content: None,
    }];
    fragment.placements[0].authority = vec![conduit_core::AuthorityBinding {
        grant_id: conduit_core::AuthorityGrantId::from("fixture/grant"),
        contract_id: conduit_core::AuthorityContractId::from("fixture/authority@1"),
        host_call_contract_id: conduit_core::HostCallContractId::from("fixture/call@1"),
        subject_kind: conduit_core::kind_id("fixture/subject"),
        host_id: conduit_core::HostId::from("host"),
        boot_id: conduit_core::BootId::from("boot"),
        capability_id: conduit_core::CapabilityId::from("state"),
    }];
}

#[test]
fn definition_is_derived_only_from_verified_plan_and_subordinate_receipt() {
    let plan = activation_plan();
    let mut host = Host::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let definition =
        KernelCompositeDefinition::from_planned_activation(&plan, &prepared, "each").unwrap();
    let conduit_core::PlannedActivationEntry::Unary(planned) = &plan.activations[0] else {
        unreachable!()
    };
    assert_eq!(
        definition.internal_plan.plan_id,
        planned.selected_plan.plan_id
    );
    assert_eq!(
        definition.boundary.input_fronts[0]
            .external_port
            .port_id
            .as_str(),
        "in"
    );
    assert_eq!(
        definition.boundary.output_fronts[0]
            .external_port
            .port_id
            .as_str(),
        "out"
    );
    assert!(KernelCompositeDefinition::from_planned_activation(&plan, &prepared, "other").is_err());
}

#[test]
fn receipted_child_pool_is_initialized_exactly_n_then_consumed_without_registry() {
    let plan = activation_plan();
    let mut host = Host::new();
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    host.registry
        .install(PreparedFactory(count.clone()))
        .unwrap();
    let pool =
        PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, "each", &mut host)
            .unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert!(
        PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, "each", &mut host)
            .is_err()
    );
    let composite = PreparedPlannedActivationComposite::prepare(&plan, "each", pool);
    assert!(composite.is_ok(), "{:?}", composite.err());
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn missing_factory_is_refused_during_receipted_host_preparation() {
    let plan = activation_plan();
    let mut host = Host::new();
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    assert!(
        PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, "each", &mut host,)
            .is_err()
    );
}

#[test]
fn child_pool_refuses_current_boot_and_offer_drift_before_consuming_receipts() {
    let plan = activation_plan();
    let mut host = Host::new();
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    host.registry
        .install(PreparedFactory(Arc::new(AtomicUsize::new(0))))
        .unwrap();
    let retained = host.receipts.len();
    host.identity.boot_id = conduit_core::BootId::from("replacement-boot");
    assert!(
        PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, "each", &mut host)
            .is_err()
    );
    assert_eq!(host.receipts.len(), retained);
    host.identity = PreparationHostIdentity {
        host_id: conduit_core::HostId::from("host"),
        boot_id: conduit_core::BootId::from("boot"),
        offer_generation: conduit_core::OfferGeneration(99),
    };
    assert!(
        PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, "each", &mut host)
            .is_err()
    );
    assert_eq!(host.receipts.len(), retained);
}

#[test]
fn child_pool_refuses_host_substituted_kernel_definitions_after_exact_receipt_consumption() {
    let plan = activation_plan();
    let mut host = Host::new();
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let exact =
        KernelCompositeDefinition::from_planned_activation(&plan, &prepared, "each").unwrap();
    let mut substituted = exact.clone();
    let mut fragment = substituted.internal_plan.fragments[0].clone();
    fragment.host_id = conduit_core::HostId::from("substituted-host");
    fragment.boot_id = conduit_core::BootId::from("substituted-boot");
    fragment.offer_generation = conduit_core::OfferGeneration(99);
    let placement = &mut fragment.placements[0];
    placement.host_calls[0].contract_id =
        conduit_core::HostCallContractId::from("substituted/call@1");
    placement.resources[0].pool_id = conduit_core::ResourcePoolId::from("substituted-pool");
    placement.authority[0].grant_id = conduit_core::AuthorityGrantId::from("substituted-grant");
    placement.authority[0].host_call_contract_id =
        conduit_core::HostCallContractId::from("substituted/call@1");
    placement.authority[0].host_id = conduit_core::HostId::from("substituted-host");
    placement.authority[0].boot_id = conduit_core::BootId::from("substituted-boot");
    substituted.internal_plan = common::seal(fragment);
    substituted.host_id = conduit_core::HostId::from("substituted-host");
    substituted.boot_id = conduit_core::BootId::from("substituted-boot");
    substituted.offer_generation = conduit_core::OfferGeneration(99);
    for front in substituted
        .boundary
        .input_fronts
        .iter_mut()
        .chain(&mut substituted.boundary.output_fronts)
    {
        front.internal_child = conduit_core::HostId::from("substituted-host");
    }
    host.registry.install(HostCallFactory).unwrap();
    host.substituted_definition = Some(substituted);

    let result =
        PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, "each", &mut host);
    match result {
        Err(conduit_composite::PlannedActivationCompositeError::SubstitutedChildDefinition {
            index: 0,
        }) => {}
        Err(error) => panic!("unexpected refusal: {error:?}"),
        Ok(_) => panic!("substituted child definition was accepted"),
    }
    assert!(prepared.take_subordinate_receipts("each").is_empty());
}

#[test]
fn child_pool_cannot_be_substituted_for_another_activation_identity() {
    let plan = activation_plan();
    let mut host = Host::new();
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    host.registry
        .install(PreparedFactory(Arc::new(AtomicUsize::new(0))))
        .unwrap();
    let pool =
        PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, "each", &mut host)
            .unwrap();
    assert!(PreparedPlannedActivationComposite::prepare(&plan, "other", pool,).is_err());
}

#[test]
fn invalid_completion_retains_dispatch_then_corrected_completion_consumes_it_once() {
    let plan = activation_plan();
    let mut preparation_host = Host::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut preparation_host]).unwrap();
    let definition =
        KernelCompositeDefinition::from_planned_activation(&plan, &prepared, "each").unwrap();
    let mut registry = KernelOperationRegistry::new();
    registry.install(HostCallFactory).unwrap();
    let mut child = conduit_composite::KernelCompositeHost::prepare(definition, &registry).unwrap();
    child.start().unwrap();
    child
        .admit_input(
            &conduit_core::port_id("in"),
            0,
            &conduit_core::ValuePayload {
                value_kind: conduit_core::kind_id(conduit_core::BOOL_INFO_ID),
                encoded: vec![7],
            },
        )
        .unwrap();
    let request = loop {
        child.step().unwrap();
        if let Some(request) = child.next_host_request() {
            break request;
        }
    };
    let admitted = {
        let obligation = child.host_request_obligation(&request).unwrap();
        child
            .admit_host_request(
                &request,
                &obligation.host,
                &obligation.resources,
                &obligation.authorities,
            )
            .unwrap()
    };

    assert!(child.complete_host_call_bytes(&admitted, &[1, 2]).is_err());
    assert_eq!(child.host_request_input(&admitted).unwrap(), &[7]);
    child.complete_host_call_bytes(&admitted, &[9]).unwrap();
    assert!(child.complete_host_call_bytes(&admitted, &[9]).is_err());
    assert!(child.host_request_input(&admitted).is_err());
}

#[test]
fn host_call_dispatch_lifecycle_allocates_nothing_after_preparation() {
    let plan = activation_plan();
    let mut preparation_host = Host::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut preparation_host]).unwrap();
    let definition =
        KernelCompositeDefinition::from_planned_activation(&plan, &prepared, "each").unwrap();
    let mut registry = KernelOperationRegistry::new();
    registry.install(HostCallFactory).unwrap();
    let mut child = conduit_composite::KernelCompositeHost::prepare(definition, &registry).unwrap();
    let input = conduit_core::ValuePayload {
        value_kind: conduit_core::kind_id(conduit_core::BOOL_INFO_ID),
        encoded: vec![7],
    };
    let input_port = conduit_core::port_id("in");
    let unknown = conduit_composite::KernelCompositeHostRequest {
        dispatch_token: u64::MAX,
    };
    let mut surfaced = None;
    let mut admitted = None;

    let allocations = allocations_during(|| {
        child.start().unwrap();
        child.admit_input(&input_port, 0, &input).unwrap();
        loop {
            child.step().unwrap();
            if let Some(request) = child.next_host_request() {
                surfaced = Some(request);
                break;
            }
        }
        let request = surfaced.unwrap();
        assert!(child.host_request_view(&request).is_ok());
        assert!(child.host_request_view(&unknown).is_err());
        let obligation = child.host_request_obligation(&request).unwrap();
        admitted = Some(
            child
                .admit_host_request(
                    &request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .unwrap(),
        );
        let admitted_request = admitted.unwrap();
        assert_eq!(child.host_request_input(&admitted_request).unwrap(), &[7]);
        assert_eq!(
            child.complete_host_call_bytes(&admitted_request, &[1, 2]),
            Err(conduit_composite::KernelCompositeError::HostCallOutputExceeded)
        );
        child
            .complete_host_call_bytes(&admitted_request, &[9])
            .unwrap();
        assert_eq!(
            child.complete_host_call_bytes(&admitted_request, &[9]),
            Err(conduit_composite::KernelCompositeError::InvalidHostCallToken)
        );
    });
    assert_eq!(
        allocations, 0,
        "Host Call lifecycle allocated {allocations} times"
    );
}

#[test]
fn select_executes_through_the_receipt_backed_unary_pool() {
    let plan = activation_plan();
    let mut host = Host::new();
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    host.registry
        .install(PreparedFactory(Arc::new(AtomicUsize::new(0))))
        .unwrap();
    let pool =
        PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, "each", &mut host)
            .unwrap();
    let unary = PreparedPlannedActivationComposite::prepare(&plan, "each", pool)
        .unwrap()
        .into_unary()
        .unwrap();
    let mut select = FlowSelectCoordinator::from_prepared_activation(unary).unwrap();
    let original = conduit_core::ValuePayload {
        value_kind: conduit_core::kind_id(conduit_core::BOOL_INFO_ID),
        encoded: vec![1],
    };
    select.admit(7, original.clone()).unwrap();
    for _ in 0..32 {
        select.step().unwrap();
        if let Some((sequence, output)) = select.output() {
            assert_eq!((sequence, output), (7, &original));
            return;
        }
    }
    panic!("receipt-backed select did not produce its selected item")
}

#[test]
fn fold_and_scan_are_reachable_only_through_their_receipt_backed_variants() {
    for (scan, id) in [(false, "fold"), (true, "scan")] {
        let plan = fold_or_scan_plan(scan);
        let mut host = Host::new();
        let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
        host.registry.install(PlainFactory).unwrap();
        let pool =
            PreparedActivationChildPool::prepare_on_host(&plan, &mut prepared, id, &mut host)
                .unwrap();
        let prepared = PreparedPlannedActivationComposite::prepare(&plan, id, pool).unwrap();
        if scan {
            assert!(prepared.into_scan().is_ok());
        } else {
            assert!(prepared.into_fold().is_ok());
        }
    }
}

trait CallForwarding {
    fn next_call(&mut self) -> Option<KernelCompositeHostRequest>;
    fn step_call(&mut self);
    fn view_call(&self, request: &KernelCompositeHostRequest) -> bool;
    fn reject_call(&self, request: &KernelCompositeHostRequest) -> bool;
    fn admit_call(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> AdmittedKernelCompositeHostRequest;
    fn input_call(&self, request: &AdmittedKernelCompositeHostRequest) -> bool;
    fn complete_bytes_call(
        &mut self,
        request: &AdmittedKernelCompositeHostRequest,
        bytes: &[u8],
    ) -> bool;
}

macro_rules! call_forwarding {
    ($type:ty) => {
        impl CallForwarding for $type {
            fn next_call(&mut self) -> Option<KernelCompositeHostRequest> {
                self.next_host_request()
            }
            fn step_call(&mut self) {
                self.step().unwrap();
            }
            fn view_call(&self, request: &KernelCompositeHostRequest) -> bool {
                self.host_request_view(request).is_ok()
            }
            fn reject_call(&self, request: &KernelCompositeHostRequest) -> bool {
                let obligation = self.host_request_obligation(request).unwrap();
                self.admit_host_request(request, &obligation.host, &[], &obligation.authorities)
                    .is_err()
            }
            fn admit_call(
                &self,
                request: &KernelCompositeHostRequest,
            ) -> AdmittedKernelCompositeHostRequest {
                let obligation = self.host_request_obligation(request).unwrap();
                self.admit_host_request(
                    request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .unwrap()
            }
            fn input_call(&self, request: &AdmittedKernelCompositeHostRequest) -> bool {
                self.host_request_input(request).is_ok()
            }
            fn complete_bytes_call(
                &mut self,
                request: &AdmittedKernelCompositeHostRequest,
                bytes: &[u8],
            ) -> bool {
                self.complete_host_call_bytes(request, bytes).is_ok()
            }
        }
    };
}

call_forwarding!(BoundedActivationHost);
call_forwarding!(FlowSelectCoordinator);
call_forwarding!(BoundedFoldActivationHost);
call_forwarding!(BoundedScanActivationHost);

fn prove_forwarded_call(host: &mut impl CallForwarding) {
    let request = loop {
        host.step_call();
        if let Some(request) = host.next_call() {
            break request;
        }
    };
    assert!(host.view_call(&request));
    assert!(host.reject_call(&request));
    assert!(host.view_call(&request));
    let admitted = host.admit_call(&request);
    assert!(host.input_call(&admitted));
    assert!(!host.complete_bytes_call(&admitted, &[1, 2]));
    assert!(host.input_call(&admitted));
    assert!(host.complete_bytes_call(&admitted, &[1]));
    assert!(!host.complete_bytes_call(&admitted, &[1]));
}

fn prepared_call_composite(plan: &Plan, id: &str) -> PreparedPlannedActivationComposite {
    let mut host = Host::new();
    let mut prepared = prepare_plan_on_hosts(plan, &mut [&mut host]).unwrap();
    host.registry.install(HostCallFactory).unwrap();
    let pool =
        PreparedActivationChildPool::prepare_on_host(plan, &mut prepared, id, &mut host).unwrap();
    PreparedPlannedActivationComposite::prepare(plan, id, pool).unwrap()
}

#[test]
fn every_coordinator_forwards_call_lifecycle_without_play_time_allocation() {
    let each_plan = activation_plan();
    let mut each = prepared_call_composite(&each_plan, "each")
        .into_unary()
        .unwrap();
    let mut select = FlowSelectCoordinator::from_prepared_activation(
        prepared_call_composite(&each_plan, "each")
            .into_unary()
            .unwrap(),
    )
    .unwrap();
    let fold_plan = fold_or_scan_plan(false);
    let mut fold = prepared_call_composite(&fold_plan, "fold")
        .into_fold()
        .unwrap();
    let scan_plan = fold_or_scan_plan(true);
    let mut scan = prepared_call_composite(&scan_plan, "scan")
        .into_scan()
        .unwrap();
    let boolean = conduit_core::ValuePayload {
        value_kind: conduit_core::kind_id(conduit_core::BOOL_INFO_ID),
        encoded: vec![7],
    };
    let byte = conduit_core::ValuePayload {
        value_kind: conduit_core::kind_id("fixture/byte@1"),
        encoded: vec![7],
    };
    let mut fold_final = conduit_core::ValuePayload {
        value_kind: conduit_core::kind_id("fixture/byte@1"),
        encoded: Vec::with_capacity(1),
    };
    let mut scan_output = conduit_core::ValuePayload {
        value_kind: conduit_core::kind_id("fixture/byte@1"),
        encoded: Vec::with_capacity(1),
    };

    let allocations = allocations_during(|| {
        each.activate(1, &boolean).unwrap();
        prove_forwarded_call(&mut each);
        for _ in 0..16 {
            each.step().unwrap();
            if let Some((sequence, output)) = each.output().unwrap() {
                assert_eq!(output.encoded, &[1]);
                each.complete_output(sequence).unwrap();
                break;
            }
        }

        select.admit(2, boolean).unwrap();
        prove_forwarded_call(&mut select);
        for _ in 0..16 {
            select.step().unwrap();
            if let Some((sequence, output)) = select.output() {
                assert_eq!(output.encoded, &[7]);
                select.complete_output(sequence).unwrap();
                break;
            }
        }

        fold.admit(&byte).unwrap();
        prove_forwarded_call(&mut fold);
        fold.close_input().unwrap();
        for _ in 0..16 {
            if matches!(
                fold.step().unwrap(),
                conduit_composite::BoundedFoldState::FinalReady
            ) {
                break;
            }
        }
        assert!(fold.final_value_into(&mut fold_final).unwrap());
        assert_eq!(fold_final.encoded, &[1]);

        scan.admit(&byte).unwrap();
        prove_forwarded_call(&mut scan);
        for _ in 0..16 {
            if matches!(
                scan.step().unwrap(),
                conduit_composite::BoundedScanState::OutputReady
            ) {
                break;
            }
        }
        assert!(scan.output_into(&mut scan_output).unwrap());
        assert_eq!(scan_output.encoded, &[1]);
        scan.complete_output().unwrap();
    });
    assert_eq!(
        allocations, 0,
        "coordinator Host Call forwarding allocated {allocations} times"
    );
}
