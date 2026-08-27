use conduit_composite::{
    KernelCompositeDefinition, KernelCompositeError, KernelCompositeHost, KernelCompositeStatus,
    KernelOperationBudget, KernelOperationFactory, KernelOperationRegistry,
};
use conduit_core::{
    kind_id, process_owned_line_offer, ArtifactId, BaseImplementationId, BootId, CapabilityId,
    CapabilityLimits, CapabilityOffer, FailureReason, GearId, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, KindContractRevision, OfferGeneration, PlannedGear,
    PortDescriptor, PortDirection, ValuePayload, PROTOCOL_VERSION,
};
use conduit_form::{parse, KindDefinition, ProfileCatalog};
use conduit_kernel::{
    HostedValueStore, Operation, OperationAction, OperationInput, PortId as KernelPortId,
};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use std::collections::BTreeMap;

const ECHO_KIND: &str = "test/kernel-composite-echo";
const VALUE_KIND: &str = "value/bytes";
const IMPLEMENTATION: &str = "test/kernel-composite-echo-v1";

fn descriptor(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: kind_id(VALUE_KIND),
        direction,
        temporal: conduit_core::PortTemporal::Value,
    }
}

fn catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(ECHO_KIND),
            kind_contract_revision: KindContractRevision::from("test/kernel-composite-echo@1"),
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
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from("echo"),
            kind_id: kind_id(ECHO_KIND),
            kind_contract_revision: KindContractRevision::from("test/kernel-composite-echo@1"),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: "test/kernel-composite@1".into(),
                implementation_id: IMPLEMENTATION.into(),
                artifact_id: "test/kernel-composite-artifact-v1".into(),
            },
            inputs: vec![descriptor("in", PortDirection::Input)],
            outputs: vec![descriptor("out", PortDirection::Output)],
            host_operations: vec![],
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
    let form = parse(
        "form test/two-child-echo (\n > input: value/bytes\n output: value/bytes >\n) {\n first: test/kernel-composite-echo\n second: test/kernel-composite-echo\n input > first.in\n first.out > second.in\n second.out > output\n}\n",
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
        &form,
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
        &form,
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
    ) -> Result<Box<dyn Operation + Send>, String> {
        if self.fail {
            Ok(Box::new(Fail))
        } else {
            Ok(Box::new(Echo))
        }
    }
}

struct Echo;

struct Fail;

impl Operation for Fail {
    fn start(&mut self) -> OperationAction {
        OperationAction::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::InvalidLifecycle,
            detail: 17,
        })
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        OperationAction::Await
    }
}

impl Operation for Echo {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { value, .. } => OperationAction::Emit {
                port: KernelPortId(0),
                value,
            },
            OperationInput::Closed { .. } => OperationAction::Complete,
            _ => OperationAction::Await,
        }
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
    assert_eq!((sequence, output), (0, value(b"exact")));
    host.complete_output(&conduit_core::port_id("output"), sequence)
        .unwrap();
    host.close_input(&conduit_core::port_id("input")).unwrap();
    for _ in 0..64 {
        if host.step().unwrap() == KernelCompositeStatus::Complete {
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
    stale.boundary.input_faces[0].internal_child = HostId::from("stale-child");
    assert!(matches!(
        KernelCompositeHost::prepare(stale, &registry()),
        Err(KernelCompositeError::StaleChild(_))
    ));
}

#[test]
fn malformed_boundary_binding_and_value_kind_refuse_distinctly() {
    let mut malformed = definition();
    malformed.boundary.input_faces[0].internal_port_id = conduit_core::port_id("missing");
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
    host.cancel().unwrap();
    assert_eq!(host.step().unwrap(), KernelCompositeStatus::Cancelled);
    assert!(matches!(
        host.admit_input(&conduit_core::port_id("input"), 0, &value(b"late")),
        Err(KernelCompositeError::InvalidLifecycle)
    ));
}
