use conduit_composite::{
    KernelCompositeDefinition, KernelOperationBudget, KernelOperationFactory,
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

#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

struct Host {
    identity: PreparationHostIdentity,
    receipts: Vec<PreparedFragmentReceipt>,
    registry: KernelOperationRegistry,
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
        let ready = (0..maximum_items)
            .map(|_| {
                conduit_composite::KernelCompositeHost::prepare(definition.clone(), &self.registry)
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
        _: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        StepOutcome::Complete
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
            if io.host_completion().is_some() {
                io.consume_host_completion().expect("present completion");
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        let input = BoundedValueRef::new(value, 1).expect("one-byte fixture input");
        io.consume(PortId(0)).expect("present fixture input");
        io.request_host_call(RequestId(0), HostCallId(0), input)
            .expect("planned fixture Host Call");
        self.pending = true;
        StepOutcome::Progress
    }
}

fn activation_plan() -> Plan {
    let mut child_fragment = common::fragment();
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
            value_kind: conduit_core::kind_id("fixture/byte@1"),
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
            value_kind: conduit_core::kind_id("fixture/byte@1"),
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
        conduit_core::FormIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_form_id: outer.checked_form_id.clone(),
            expanded_form_id: outer.expanded_form_id.clone(),
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
                value_kind: conduit_core::kind_id("fixture/byte@1"),
                abnormal_kind: None,
            },
            output: conduit_core::PlannedActivationFront {
                front_port_id: conduit_core::port_id("out"),
                value_kind: conduit_core::kind_id("fixture/byte@1"),
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
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    host.registry
        .install(PreparedFactory(count.clone()))
        .unwrap();
    let pool =
        PreparedActivationChildPool::prepare_on_host(&plan, &prepared, "each", &mut host).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert!(
        PreparedActivationChildPool::prepare_on_host(&plan, &prepared, "each", &mut host).is_err()
    );
    let composite = PreparedPlannedActivationComposite::prepare(&plan, &prepared, "each", pool);
    assert!(composite.is_ok(), "{:?}", composite.err());
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn missing_factory_is_refused_during_receipted_host_preparation() {
    let plan = activation_plan();
    let mut host = Host::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    assert!(
        PreparedActivationChildPool::prepare_on_host(&plan, &prepared, "each", &mut host,).is_err()
    );
}

#[test]
fn child_pool_refuses_current_boot_and_offer_drift_before_consuming_receipts() {
    let plan = activation_plan();
    let mut host = Host::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    host.registry
        .install(PreparedFactory(Arc::new(AtomicUsize::new(0))))
        .unwrap();
    let retained = host.receipts.len();
    host.identity.boot_id = conduit_core::BootId::from("replacement-boot");
    assert!(
        PreparedActivationChildPool::prepare_on_host(&plan, &prepared, "each", &mut host).is_err()
    );
    assert_eq!(host.receipts.len(), retained);
    host.identity = PreparationHostIdentity {
        host_id: conduit_core::HostId::from("host"),
        boot_id: conduit_core::BootId::from("boot"),
        offer_generation: conduit_core::OfferGeneration(99),
    };
    assert!(
        PreparedActivationChildPool::prepare_on_host(&plan, &prepared, "each", &mut host).is_err()
    );
    assert_eq!(host.receipts.len(), retained);
}

#[test]
fn child_pool_cannot_be_substituted_for_another_activation_identity() {
    let plan = activation_plan();
    let mut host = Host::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    host.registry
        .install(PreparedFactory(Arc::new(AtomicUsize::new(0))))
        .unwrap();
    let pool =
        PreparedActivationChildPool::prepare_on_host(&plan, &prepared, "each", &mut host).unwrap();
    assert!(PreparedPlannedActivationComposite::prepare(&plan, &prepared, "other", pool,).is_err());
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
                value_kind: conduit_core::kind_id("fixture/byte@1"),
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
    let obligation = child.host_request_obligation(&request).unwrap().clone();
    let admitted = child
        .admit_host_request(
            &request,
            &obligation.host,
            &obligation.resources,
            &obligation.authorities,
        )
        .unwrap();

    assert!(child.complete_host_call_bytes(&admitted, &[1, 2]).is_err());
    assert_eq!(child.host_request_input(&admitted).unwrap(), &[7]);
    child.complete_host_call_bytes(&admitted, &[9]).unwrap();
    assert!(child.complete_host_call_bytes(&admitted, &[9]).is_err());
    assert!(child.host_request_input(&admitted).is_err());
}
