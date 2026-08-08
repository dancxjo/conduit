use super::{
    ChildHostBinding, CompositeBoundary, CompositeDefinition, CompositeFaceBinding, CompositeHost,
    DeliveryMode,
};
use conduit_core::{
    kind_id, process_owned_link_binding, ArtifactId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, CheckedFormId, ConnectionEnvelope, ConnectionOutcome, ConnectionProvider,
    ExecutionProfileId, FailureReason, HostAdvertisement, HostCommand, HostEvent, HostId,
    HostProfileId, ImplementationId, KindContractRevision, KindId, ObservationKind,
    OfferGeneration, OperationId, PlannedOperation, PlatformEffect, PortDescriptor, PortDirection,
    TerminalDisposition, PROTOCOL_VERSION,
};
use conduit_form::{
    parse, CheckedForm, CheckedOperation, CompositeFaceTerminal, KindDefinition, ProfileCatalog,
};
use conduit_planner::{plan, plan_with_link_bindings, PlacementChoice, PlacementChoices};
use conduit_runtime::{
    providers::in_memory::InMemoryConnectionProvider, HostRuntime, ImplementationFailure,
    ImplementationRegistry, OperationAction, OperationCompletion, OperationImplementation,
    OperationOutput, OperationState,
};
use conduit_signal::{
    pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
    pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_inputs, show_resource_requirements,
    signal_profile_catalog, signal_registry, signal_resource_offers, PULSE_KIND, SHOW_KIND,
};
use std::collections::BTreeMap;

const COMPOSITE_DEMONSTRATION_KIND: &str = "demonstration/run-signal";
const NUMBER_KIND: &str = "value/number";
const BYTES_KIND: &str = "value/bytes";

fn descriptor(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: conduit_core::PortTemporal::Value,
    }
}

fn definition(
    kind: &str,
    revision: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        inputs,
        outputs,
        configuration: Vec::new(),
    }
}

fn multi_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    for item in [
        definition(
            "test/number-echo",
            "test/number-echo@1",
            vec![descriptor("in", NUMBER_KIND, PortDirection::Input)],
            vec![descriptor("out", NUMBER_KIND, PortDirection::Output)],
        ),
        definition(
            "test/bytes-echo",
            "test/bytes-echo@1",
            vec![descriptor("in", BYTES_KIND, PortDirection::Input)],
            vec![descriptor("out", BYTES_KIND, PortDirection::Output)],
        ),
        definition(
            "test/number-source",
            "test/number-source@1",
            Vec::new(),
            vec![descriptor("out", NUMBER_KIND, PortDirection::Output)],
        ),
        definition(
            "test/bytes-source",
            "test/bytes-source@1",
            Vec::new(),
            vec![descriptor("out", BYTES_KIND, PortDirection::Output)],
        ),
        definition(
            "test/number-sink",
            "test/number-sink@1",
            vec![descriptor("in", NUMBER_KIND, PortDirection::Input)],
            Vec::new(),
        ),
        definition(
            "test/bytes-sink",
            "test/bytes-sink@1",
            vec![descriptor("in", BYTES_KIND, PortDirection::Input)],
            Vec::new(),
        ),
    ] {
        catalog.insert(item).expect("multi kind installs");
    }
    catalog
}

fn multi_internal_form() -> CheckedForm {
    parse(
            "form 0\nmulti {\n number: test/number-echo\n bytes: test/bytes-echo\n export run: demonstration/multi-echo {\n  input control-in: value/number = number.in terminal independent\n  input data-in: value/bytes = bytes.in terminal independent\n  output control-out: value/number = number.out terminal independent\n  output data-out: value/bytes = bytes.out terminal independent\n }\n}\n",
            &multi_catalog(),
        )
        .expect("multi-face internal form checks")
}

struct EchoImplementation {
    kind_id: KindId,
    revision: KindContractRevision,
    profile: ExecutionProfileId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
}

impl OperationImplementation for EchoImplementation {
    fn kind_id(&self) -> &KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        self.revision.clone()
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        self.profile.clone()
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn prepare(
        &self,
        placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        if placement.inputs.len() != 1 || placement.outputs.len() != 1 {
            return Err(ImplementationFailure::new(
                FailureReason::InvalidOperationConfiguration,
                "echo requires one exact input and output",
            ));
        }
        Ok(Box::new(EchoState {
            input: placement.inputs[0].port_id.clone(),
            output: placement.outputs[0].port_id.clone(),
        }))
    }

    fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
        (value_kind == &kind_id(NUMBER_KIND) || value_kind == &kind_id(BYTES_KIND)).then_some(1)
    }
}

struct EchoState {
    input: conduit_core::PortId,
    output: conduit_core::PortId,
}

impl OperationState for EchoState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { port, value } if port == self.input => {
                OperationAction::Emit(vec![OperationOutput {
                    port: self.output.clone(),
                    value,
                }])
            }
            OperationCompletion::Emitted => OperationAction::Idle,
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "echo received a wrong port or completion",
            )),
        }
    }
}

fn hosted_offer(
    capability: &str,
    kind: &str,
    revision: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(format!("{kind}/hosted@1")),
            implementation_id: ImplementationId::from(format!("test/{capability}-v1")),
            artifact_id: ArtifactId::from(format!("test/{capability}-artifact-v1")),
        },
        inputs,
        outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

fn multi_child_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("multi-child"),
        boot_id: BootId::from("multi-child-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/multi-child"),
        resources: Vec::new(),
        planner_capabilities: vec![],
        capabilities: vec![
            hosted_offer(
                "number-echo",
                "test/number-echo",
                "test/number-echo@1",
                vec![descriptor("in", NUMBER_KIND, PortDirection::Input)],
                vec![descriptor("out", NUMBER_KIND, PortDirection::Output)],
            ),
            hosted_offer(
                "bytes-echo",
                "test/bytes-echo",
                "test/bytes-echo@1",
                vec![descriptor("in", BYTES_KIND, PortDirection::Input)],
                vec![descriptor("out", BYTES_KIND, PortDirection::Output)],
            ),
        ],
    }
}

fn multi_internal_plan() -> conduit_core::Plan {
    let child = multi_child_advertisement();
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("number"),
                PlacementChoice {
                    host_id: child.host_id.clone(),
                    capability_id: CapabilityId::from("number-echo"),
                },
            ),
            (
                OperationId::from("bytes"),
                PlacementChoice {
                    host_id: child.host_id.clone(),
                    capability_id: CapabilityId::from("bytes-echo"),
                },
            ),
        ]),
    };
    plan(
        &multi_internal_form(),
        &[child],
        &placements,
        &[ConnectionProvider::Local],
    )
    .expect("multi internal plan succeeds")
}

fn multi_definition() -> CompositeDefinition {
    let internal_form = multi_internal_form();
    CompositeDefinition::from_authored_export(
        HostId::from("multi-composite"),
        BootId::from("multi-composite-boot"),
        OfferGeneration(1),
        HostProfileId::from("composite/multi-test"),
        ImplementationId::from("composite/multi-echo-v1"),
        ArtifactId::from("composite/multi-echo-artifact-v1"),
        &internal_form,
        &CapabilityId::from("run"),
        multi_internal_plan(),
        FailureReason::CompositeCapabilityFailed,
    )
    .expect("multi composite definition derives from faces without a cord")
}

fn multi_child_runtime() -> HostRuntime {
    let child_ad = multi_child_advertisement();
    let mut registry = ImplementationRegistry::new();
    for offer in &child_ad.capabilities {
        registry
            .install(EchoImplementation {
                kind_id: offer.kind_id.clone(),
                revision: offer.kind_contract_revision.clone(),
                profile: offer.implementation.execution_profile_id.clone(),
                implementation_id: offer.implementation.implementation_id.clone(),
                artifact_id: offer.implementation.artifact_id.clone(),
            })
            .expect("echo implementation installs");
    }
    HostRuntime::new(child_ad, registry, 128)
}

fn multi_composite() -> CompositeHost {
    CompositeHost::from_definition(multi_definition(), vec![multi_child_runtime()], 128)
        .expect("multi composite host builds")
}

fn parent_endpoint_advertisement(host: &str, source: bool) -> HostAdvertisement {
    let (boot, capabilities) = if source {
        (
            "parent-source-boot",
            vec![
                hosted_offer(
                    "number-source",
                    "test/number-source",
                    "test/number-source@1",
                    Vec::new(),
                    vec![descriptor("out", NUMBER_KIND, PortDirection::Output)],
                ),
                hosted_offer(
                    "bytes-source",
                    "test/bytes-source",
                    "test/bytes-source@1",
                    Vec::new(),
                    vec![descriptor("out", BYTES_KIND, PortDirection::Output)],
                ),
            ],
        )
    } else {
        (
            "parent-sink-boot",
            vec![
                hosted_offer(
                    "number-sink",
                    "test/number-sink",
                    "test/number-sink@1",
                    vec![descriptor("in", NUMBER_KIND, PortDirection::Input)],
                    Vec::new(),
                ),
                hosted_offer(
                    "bytes-sink",
                    "test/bytes-sink",
                    "test/bytes-sink@1",
                    vec![descriptor("in", BYTES_KIND, PortDirection::Input)],
                    Vec::new(),
                ),
            ],
        )
    };
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/parent-endpoint"),
        resources: Vec::new(),
        planner_capabilities: vec![],
        capabilities,
    }
}

fn multi_parent_fragment(composite: &CompositeHost) -> conduit_core::PlanFragment {
    let mut catalog = multi_catalog();
    catalog
        .insert_export(&multi_internal_form(), &CapabilityId::from("run"))
        .expect("multi export installs in parent catalog");
    let form = parse(
            "form 0\nparent {\n number-source: test/number-source\n bytes-source: test/bytes-source\n child: demonstration/multi-echo\n number-sink: test/number-sink\n bytes-sink: test/bytes-sink\n number-source.out -> child.control-in\n bytes-source.out -> child.data-in\n child.control-out -> number-sink.in\n child.data-out -> bytes-sink.in\n}\n",
            &catalog,
        )
        .expect("multi parent checks ordinary faces");
    let source = parent_endpoint_advertisement("parent-source", true);
    let sink = parent_endpoint_advertisement("parent-sink", false);
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("number-source"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("number-source"),
                },
            ),
            (
                OperationId::from("bytes-source"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("bytes-source"),
                },
            ),
            (
                OperationId::from("child"),
                PlacementChoice {
                    host_id: composite.advertisement().host_id.clone(),
                    capability_id: CapabilityId::from("run"),
                },
            ),
            (
                OperationId::from("number-sink"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("number-sink"),
                },
            ),
            (
                OperationId::from("bytes-sink"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("bytes-sink"),
                },
            ),
        ]),
    };
    let links = [
        process_owned_link_binding(
            "link/source-composite",
            ConnectionProvider::InMemory,
            "fixture/in-memory/source-composite",
            &source,
            composite.advertisement(),
            2,
            32,
        ),
        process_owned_link_binding(
            "link/composite-sink",
            ConnectionProvider::InMemory,
            "fixture/in-memory/composite-sink",
            composite.advertisement(),
            &sink,
            2,
            32,
        ),
    ];
    plan_with_link_bindings(
        &form,
        &[source, composite.advertisement().clone(), sink],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::InMemory],
        2,
        32,
        &links,
    )
    .expect("multi parent plans ordinary remote faces")
    .fragments
    .into_iter()
    .find(|fragment| fragment.host_id == composite.advertisement().host_id)
    .expect("multi composite fragment exists")
}

fn authored_internal_form() -> conduit_form::CheckedForm {
    parse(
        include_str!("../../../examples/signal-composite.form"),
        &signal_profile_catalog(),
    )
    .expect("authored composite form parses")
}

fn parent_catalog() -> ProfileCatalog {
    let mut catalog = signal_profile_catalog();
    catalog
        .insert_export(&authored_internal_form(), &CapabilityId::from("run-signal"))
        .expect("authored export installs into the parent catalog");
    catalog
}

fn child_advertisement(host: &str, boot: &str, source: bool) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std"),
        resources: signal_resource_offers(
            &format!("{host}/timer"),
            &format!("{host}/presentation"),
            2,
        ),
        planner_capabilities: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: if source {
                conduit_signal::pulse_face_startup_parameters()
            } else {
                vec![]
            },
            shorthand: None,
            capability_id: CapabilityId::from(if source { "pulse" } else { "show" }),
            kind_id: kind_id(if source { PULSE_KIND } else { SHOW_KIND }),
            kind_contract_revision: if source {
                pulse_contract_revision()
            } else {
                show_contract_revision()
            },
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: if source {
                    pulse_execution_profile()
                } else {
                    show_execution_profile()
                },
                implementation_id: ImplementationId::from(if source {
                    "test/pulse-v1"
                } else {
                    "test/show-v1"
                }),
                artifact_id: ArtifactId::from(if source {
                    "conduit-signal/pulse-artifact-v1"
                } else {
                    "conduit-signal/show-artifact-v1"
                }),
            },
            inputs: if source { vec![] } else { show_inputs() },
            outputs: if source { pulse_outputs() } else { vec![] },
            host_operations: if source {
                pulse_host_operation_requirements()
            } else {
                show_host_operation_requirements()
            },
            resource_requirements: if source {
                pulse_resource_requirements()
            } else {
                show_resource_requirements()
            },
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 2,
                max_queue_items: 8,
                max_queue_bytes: 128,
            },
        }],
    }
}

fn internal_plan(item_capacity: u16, byte_capacity: u32) -> conduit_core::Plan {
    let form = authored_internal_form();
    let source = child_advertisement("child-source", "source-boot", true);
    let sink = child_advertisement("child-sink", "sink-boot", false);
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("pulse"),
                },
            ),
            (
                OperationId::from("show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("show"),
                },
            ),
        ]),
    };
    let links = [process_owned_link_binding(
        "link/composite-children",
        ConnectionProvider::InMemory,
        "fixture/in-memory/composite-children",
        &source,
        &sink,
        8,
        128,
    )];
    plan_with_link_bindings(
        &form,
        &[source, sink],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::InMemory],
        item_capacity,
        byte_capacity,
        &links,
    )
    .expect("cross-host plan succeeds")
}

fn three_child_internal_plan() -> conduit_core::Plan {
    let form = parse(
            "form 0\n\ninternal {\n pulse: flow/pulse\n show: presentation/show\n auxiliary: flow/pulse\n pulse.count = 1\n pulse.period-ms = 0\n pulse.initial = false\n auxiliary.count = 0\n auxiliary.period-ms = 0\n auxiliary.initial = false\n pulse > show\n export run-signal: demonstration/run-signal {\n  input signal-in: value/signal = show.signal terminal independent\n  output signal: value/signal = pulse.signal terminal independent\n }\n}\n",
            &signal_profile_catalog(),
        )
        .expect("three-child internal form parses");
    let source = child_advertisement("child-source", "source-boot", true);
    let sink = child_advertisement("child-sink", "sink-boot", false);
    let auxiliary = child_advertisement("child-auxiliary", "auxiliary-boot", true);
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("pulse"),
                },
            ),
            (
                OperationId::from("show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("show"),
                },
            ),
            (
                OperationId::from("auxiliary"),
                PlacementChoice {
                    host_id: auxiliary.host_id.clone(),
                    capability_id: CapabilityId::from("pulse"),
                },
            ),
        ]),
    };
    let links = [process_owned_link_binding(
        "link/composite-children",
        ConnectionProvider::InMemory,
        "fixture/in-memory/composite-children",
        &source,
        &sink,
        8,
        128,
    )];
    plan_with_link_bindings(
        &form,
        &[source, sink, auxiliary],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::InMemory],
        conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
        conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
        &links,
    )
    .expect("three-child plan succeeds")
}

fn composite(item_capacity: u16, byte_capacity: u32) -> CompositeHost {
    let definition = composite_definition(item_capacity, byte_capacity);
    CompositeHost::from_definition(definition, child_runtimes(), 64).expect("composite is valid")
}

fn child_runtimes() -> Vec<HostRuntime> {
    let source_ad = child_advertisement("child-source", "source-boot", true);
    let sink_ad = child_advertisement("child-sink", "sink-boot", false);
    child_runtimes_with_advertisements(source_ad, sink_ad)
}

fn child_runtimes_with_advertisements(
    source_ad: HostAdvertisement,
    sink_ad: HostAdvertisement,
) -> Vec<HostRuntime> {
    let link = process_owned_link_binding(
        "link/composite-children",
        ConnectionProvider::InMemory,
        "fixture/in-memory/composite-children",
        &source_ad,
        &sink_ad,
        8,
        128,
    );
    vec![
        HostRuntime::new_with_external_state(
            source_ad,
            signal_registry(
                ImplementationId::from("test/pulse-v1"),
                ImplementationId::from("unused/show-v1"),
            )
            .expect("source registry installs"),
            128,
            vec![],
            vec![link.clone()],
        ),
        HostRuntime::new_with_external_state(
            sink_ad,
            signal_registry(
                ImplementationId::from("unused/pulse-v1"),
                ImplementationId::from("test/show-v1"),
            )
            .expect("sink registry installs"),
            128,
            vec![],
            vec![link],
        ),
    ]
}

fn composite_definition(item_capacity: u16, byte_capacity: u32) -> CompositeDefinition {
    let form = authored_internal_form();
    let plan = internal_plan(item_capacity, byte_capacity);
    CompositeDefinition::from_authored_export(
        HostId::from("composite-host"),
        BootId::from("composite-boot"),
        OfferGeneration(7),
        HostProfileId::from("composite/in-memory-v1"),
        ImplementationId::from("composite/pulse-show-v1"),
        ArtifactId::from("composite/pulse-show-artifact-v1"),
        &form,
        &CapabilityId::from("run-signal"),
        plan,
        conduit_core::FailureReason::CompositeCapabilityFailed,
    )
    .expect("authored export derives the composite definition")
}

fn parent_fragment(composite: &CompositeHost) -> conduit_core::PlanFragment {
    let form = parse(
        include_str!("../../../examples/composite-parent.form"),
        &parent_catalog(),
    )
    .expect("authored parent form parses");
    let ordinary = child_advertisement("ordinary-host", "ordinary-boot", true);
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([(
            OperationId::from("run"),
            PlacementChoice {
                host_id: composite.advertisement().host_id.clone(),
                capability_id: CapabilityId::from("run-signal"),
            },
        )]),
    };
    plan(
        &form,
        &[composite.advertisement().clone(), ordinary],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::InMemory],
    )
    .expect("parent planner treats composite as one host")
    .fragments
    .into_iter()
    .find(|fragment| fragment.host_id == composite.advertisement().host_id)
    .expect("composite fragment exists")
}

#[test]
fn authored_parent_consumes_derived_export_through_an_ordinary_planned_cord() {
    let internal = authored_internal_form();
    let boundary = internal
        .export_boundary(&CapabilityId::from("run-signal"))
        .expect("authored export checks");
    let composite = composite(4, 64);
    let sink = child_advertisement("parent-sink", "parent-sink-boot", false);
    let parent = parse(
            "form 0\nparent {\n child: demonstration/run-signal\n sink: presentation/show\n child.signal -> sink.signal\n}\n",
            &parent_catalog(),
        )
        .expect("parent consumes the derived output as an ordinary port");
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("child"),
                PlacementChoice {
                    host_id: composite.advertisement().host_id.clone(),
                    capability_id: CapabilityId::from("run-signal"),
                },
            ),
            (
                OperationId::from("sink"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("show"),
                },
            ),
        ]),
    };
    let links = [process_owned_link_binding(
        "link/parent-child",
        ConnectionProvider::InMemory,
        "fixture/in-memory/parent-child",
        composite.advertisement(),
        &sink,
        8,
        128,
    )];
    let plan = plan_with_link_bindings(
        &parent,
        &[composite.advertisement().clone(), sink],
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::InMemory],
        4,
        64,
        &links,
    )
    .expect("ordinary parent cord plans against the composite offer");
    let connection = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .next()
        .expect("parent plan has the ordinary cord");

    assert_eq!(
        parent.operations[0].kind_contract_revision,
        boundary.kind_contract_revision
    );
    assert_eq!(connection.source_port_id.as_str(), "signal");
    assert_eq!(connection.sink_port_id.as_str(), "signal");
    assert_eq!(connection.value_kind, boundary.outputs[0].value_kind);
}

#[test]
fn two_input_two_output_multi_kind_faces_execute_with_exact_pressure_and_closure() {
    let mut composite = multi_composite();
    let fragment = multi_parent_fragment(&composite);
    let plan_id = fragment.plan_id.clone();
    let placement_id = fragment.placements[0].placement_id.clone();
    let connection = |input: bool, port: &str| {
        fragment
            .connections
            .iter()
            .find(|connection| {
                if input {
                    connection.sink_placement_id == placement_id
                        && connection.sink_port_id.as_str() == port
                } else {
                    connection.source_placement_id == placement_id
                        && connection.source_port_id.as_str() == port
                }
            })
            .expect("named parent connection exists")
            .connection_id
            .clone()
    };
    let number_in = connection(true, "control-in");
    let bytes_in = connection(true, "data-in");
    let number_out = connection(false, "control-out");
    let bytes_out = connection(false, "data-out");

    let prepared = composite.handle(HostCommand::Prepare(fragment));
    assert!(matches!(
        prepared.events.first(),
        Some(HostEvent::Prepared { .. })
    ));
    let activated = composite.handle(HostCommand::Activate(plan_id.clone()));
    assert!(activated
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Activated { .. })));

    let malformed = composite.handle(HostCommand::AcceptConnectionEnvelope(ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id: plan_id.clone(),
        connection_id: number_in.clone(),
        sequence: 0,
        value_kind: kind_id(BYTES_KIND),
        payload: vec![9],
    }));
    assert!(malformed.events.iter().any(|event| matches!(
        event,
        HostEvent::ConnectionEnvelopeOutcome {
            outcome: ConnectionOutcome::Malformed,
            ..
        }
    )));
    assert!(malformed.effects.is_empty());

    let number = composite.handle(HostCommand::AcceptConnectionEnvelope(ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id: plan_id.clone(),
        connection_id: number_in.clone(),
        sequence: 0,
        value_kind: kind_id(NUMBER_KIND),
        payload: 42u64.to_le_bytes().to_vec(),
    }));
    let number_envelope = number
        .effects
        .iter()
        .find_map(|effect| match effect {
            PlatformEffect::TransmitConnection { envelope }
                if envelope.connection_id == number_out =>
            {
                Some(envelope.clone())
            }
            _ => None,
        })
        .expect("control value exits only through control-out");
    assert_eq!(number_envelope.value_kind.as_str(), NUMBER_KIND);
    assert_eq!(number_envelope.payload, 42u64.to_le_bytes());
    assert!(!format!("{number:?}").contains("multi-child"));

    let retry = composite.handle(HostCommand::CompleteConnectionDelivery {
        plan_id: plan_id.clone(),
        connection_id: number_out.clone(),
        sequence: number_envelope.sequence,
        outcome: ConnectionOutcome::Full,
    });
    assert!(retry.effects.iter().any(|effect| matches!(
        effect,
        PlatformEffect::TransmitConnection { envelope } if envelope == &number_envelope
    )));
    composite.handle(HostCommand::CompleteConnectionDelivery {
        plan_id: plan_id.clone(),
        connection_id: number_out.clone(),
        sequence: number_envelope.sequence,
        outcome: ConnectionOutcome::Delivered,
    });

    let bytes = composite.handle(HostCommand::AcceptConnectionEnvelope(ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id: plan_id.clone(),
        connection_id: bytes_in.clone(),
        sequence: 0,
        value_kind: kind_id(BYTES_KIND),
        payload: vec![1, 2, 3, 4],
    }));
    let bytes_envelope = bytes
        .effects
        .iter()
        .find_map(|effect| match effect {
            PlatformEffect::TransmitConnection { envelope }
                if envelope.connection_id == bytes_out =>
            {
                Some(envelope.clone())
            }
            _ => None,
        })
        .expect("data value exits only through data-out");
    assert_eq!(bytes_envelope.value_kind.as_str(), BYTES_KIND);
    assert_eq!(bytes_envelope.payload, vec![1, 2, 3, 4]);
    composite.handle(HostCommand::CompleteConnectionDelivery {
        plan_id: plan_id.clone(),
        connection_id: bytes_out.clone(),
        sequence: bytes_envelope.sequence,
        outcome: ConnectionOutcome::Delivered,
    });

    let first_closed = composite.handle(HostCommand::CloseConnection {
        plan_id: plan_id.clone(),
        connection_id: number_in,
    });
    assert!(first_closed.events.iter().any(|event| matches!(
        event,
        HostEvent::ConnectionTerminated { connection_id, .. }
            if connection_id == &number_out
    )));
    assert!(!first_closed
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::PlanTerminated { .. })));

    let completed = composite.handle(HostCommand::CloseConnection {
        plan_id: plan_id.clone(),
        connection_id: bytes_in,
    });
    assert!(completed.events.iter().any(|event| matches!(
        event,
        HostEvent::ConnectionTerminated { connection_id, .. }
            if connection_id == &bytes_out
    )));
    assert!(completed.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            plan_id: terminal_plan,
            disposition: TerminalDisposition::Completed,
        } if terminal_plan == &plan_id
    )));
    assert!(!format!("{completed:?}").contains("multi-child"));
}

#[test]
fn input_only_and_output_only_exports_plan_as_ordinary_operations() {
    let catalog = multi_catalog();
    let input_only = parse(
            "form 0\ninput-only {\n echo: test/number-echo\n export ingest: demonstration/input-only {\n  input value: value/number = echo.in terminal independent\n }\n}\n",
            &catalog,
        )
        .expect("input-only checks");
    let output_only = parse(
            "form 0\noutput-only {\n echo: test/number-echo\n export produce: demonstration/output-only {\n  output value: value/number = echo.out terminal independent\n }\n}\n",
            &catalog,
        )
        .expect("output-only checks");
    let input_boundary = input_only
        .export_boundary(&CapabilityId::from("ingest"))
        .expect("input-only boundary derives");
    let output_boundary = output_only
        .export_boundary(&CapabilityId::from("produce"))
        .expect("output-only boundary derives");
    let mut parent_catalog = multi_catalog();
    parent_catalog
        .insert_export(&input_only, &CapabilityId::from("ingest"))
        .expect("input-only installs");
    parent_catalog
        .insert_export(&output_only, &CapabilityId::from("produce"))
        .expect("output-only installs");
    let parent = parse(
            "form 0\nparent {\n source: test/number-source\n input-only: demonstration/input-only\n output-only: demonstration/output-only\n sink: test/number-sink\n source.out -> input-only.value\n output-only.value -> sink.in\n}\n",
            &parent_catalog,
        )
        .expect("zero-sided parent checks");
    let mut advertisement = parent_endpoint_advertisement("zero-sided-host", true);
    advertisement.boot_id = BootId::from("zero-sided-boot");
    advertisement.capabilities.push(hosted_offer(
        "number-sink",
        "test/number-sink",
        "test/number-sink@1",
        vec![descriptor("in", NUMBER_KIND, PortDirection::Input)],
        Vec::new(),
    ));
    for (capability, boundary) in [
        ("input-only", input_boundary),
        ("output-only", output_boundary),
    ] {
        advertisement.capabilities.push(CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from(capability),
            kind_id: boundary.kind_id,
            kind_contract_revision: boundary.kind_contract_revision,
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(format!(
                    "test/{capability}-hosted@1"
                )),
                implementation_id: ImplementationId::from(format!("test/{capability}-v1")),
                artifact_id: ArtifactId::from(format!("test/{capability}-artifact-v1")),
            },
            inputs: boundary.inputs,
            outputs: boundary.outputs,
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 2,
                max_queue_items: 4,
                max_queue_bytes: 64,
            },
        });
    }
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("source"),
                PlacementChoice {
                    host_id: advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("number-source"),
                },
            ),
            (
                OperationId::from("input-only"),
                PlacementChoice {
                    host_id: advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("input-only"),
                },
            ),
            (
                OperationId::from("output-only"),
                PlacementChoice {
                    host_id: advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("output-only"),
                },
            ),
            (
                OperationId::from("sink"),
                PlacementChoice {
                    host_id: advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("number-sink"),
                },
            ),
        ]),
    };
    let planned = plan(
        &parent,
        &[advertisement],
        &placements,
        &[ConnectionProvider::Local],
    )
    .expect("input-only and output-only parent plans normally");
    assert_eq!(planned.fragments.len(), 1);
    assert_eq!(planned.fragments[0].connections.len(), 2);
    assert!(planned.fragments[0].connections.iter().any(|connection| {
        connection.sink_port_id.as_str() == "value" && connection.source_port_id.as_str() == "out"
    }));
    assert!(planned.fragments[0].connections.iter().any(|connection| {
        connection.source_port_id.as_str() == "value" && connection.sink_port_id.as_str() == "in"
    }));
}

#[test]
fn composite_definition_rejects_every_face_mapping_mutation() {
    let rejects = |definition: CompositeDefinition| {
        CompositeHost::from_definition(definition, vec![multi_child_runtime()], 32).is_err()
    };

    let mut name = multi_definition();
    name.boundary.input_faces[0].external_port.port_id = conduit_core::port_id("renamed");
    assert!(rejects(name));

    let mut direction = multi_definition();
    direction.boundary.input_faces[0].external_port.direction = PortDirection::Output;
    assert!(rejects(direction));

    let mut kind = multi_definition();
    kind.boundary.output_faces[0].external_port.value_kind = kind_id(BYTES_KIND);
    assert!(rejects(kind));

    let mut endpoint = multi_definition();
    endpoint.boundary.output_faces[0].internal_port_id = conduit_core::port_id("missing");
    assert!(rejects(endpoint));

    let mut terminal = multi_definition();
    terminal.boundary.output_faces[0].terminal = CompositeFaceTerminal::Coupled;
    assert!(rejects(terminal));

    let mut hidden_child = multi_definition();
    hidden_child.boundary.input_faces[0].internal_child = HostId::from("hidden-child");
    assert!(rejects(hidden_child));
}

#[test]
fn named_face_delivery_failure_and_cancellation_are_parent_terminal_without_topology_leaks() {
    let mut failed_composite = multi_composite();
    let failed_fragment = multi_parent_fragment(&failed_composite);
    let failed_plan_id = failed_fragment.plan_id.clone();
    let placement = failed_fragment.placements[0].placement_id.clone();
    let number_in = failed_fragment
        .connections
        .iter()
        .find(|connection| {
            connection.sink_placement_id == placement
                && connection.sink_port_id.as_str() == "control-in"
        })
        .expect("number input exists")
        .connection_id
        .clone();
    failed_composite.handle(HostCommand::Prepare(failed_fragment));
    failed_composite.handle(HostCommand::Activate(failed_plan_id.clone()));
    let emitted =
        failed_composite.handle(HostCommand::AcceptConnectionEnvelope(ConnectionEnvelope {
            protocol_version: PROTOCOL_VERSION,
            plan_id: failed_plan_id.clone(),
            connection_id: number_in,
            sequence: 0,
            value_kind: kind_id(NUMBER_KIND),
            payload: vec![7],
        }));
    let envelope = emitted
        .effects
        .iter()
        .find_map(|effect| match effect {
            PlatformEffect::TransmitConnection { envelope } => Some(envelope.clone()),
            _ => None,
        })
        .expect("named output is offered");
    let failed = failed_composite.handle(HostCommand::CompleteConnectionDelivery {
        plan_id: failed_plan_id.clone(),
        connection_id: envelope.connection_id,
        sequence: envelope.sequence,
        outcome: ConnectionOutcome::Malformed,
    });
    assert!(failed.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Failed {
                reason: FailureReason::CompositeCapabilityFailed,
            },
            ..
        }
    )));
    assert_eq!(
        failed
            .events
            .iter()
            .filter(|event| matches!(event, HostEvent::ConnectionTerminated { .. }))
            .count(),
        4
    );
    assert!(!format!("{failed:?}").contains("multi-child"));
    let evidence = failed_composite.handle(HostCommand::Inspect);
    assert!(evidence.events.iter().any(|event| match event {
        HostEvent::Observations { items } => items.iter().any(|observation| matches!(
            observation.kind,
            ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Failed {
                    reason: FailureReason::CompositeCapabilityFailed,
                }
            }
        )),
        _ => false,
    }));

    let mut cancelled_composite = multi_composite();
    let cancelled_fragment = multi_parent_fragment(&cancelled_composite);
    let cancelled_plan_id = cancelled_fragment.plan_id.clone();
    cancelled_composite.handle(HostCommand::Prepare(cancelled_fragment));
    cancelled_composite.handle(HostCommand::Activate(cancelled_plan_id.clone()));
    let cancelled = cancelled_composite.handle(HostCommand::Cancel(cancelled_plan_id));
    assert!(cancelled.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Cancelled { .. },
            ..
        }
    )));
    assert_eq!(
        cancelled
            .events
            .iter()
            .filter(|event| matches!(event, HostEvent::ConnectionTerminated { .. }))
            .count(),
        4
    );
    assert!(!format!("{cancelled:?}").contains("multi-child"));
}

#[test]
fn provider_enforces_identity_order_and_bounds() {
    let plan = internal_plan(1, 9);
    let connection = &plan.fragments[0].connections[0];
    let mut provider = InMemoryConnectionProvider::new(plan.plan_id.clone(), connection);
    let envelope = ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id: plan.plan_id.clone(),
        connection_id: connection.connection_id.clone(),
        sequence: 0,
        value_kind: connection.value_kind.clone(),
        payload: vec![0; 9],
    };
    assert_eq!(provider.status(), ConnectionOutcome::Ready);
    assert_eq!(
        provider.accept(envelope.clone()),
        ConnectionOutcome::Accepted
    );
    assert_eq!(provider.queued_items(), 1);
    assert_eq!(provider.queued_bytes(), 9);
    assert_eq!(provider.status(), ConnectionOutcome::Full);
    let mut next = envelope.clone();
    next.sequence = 1;
    assert_eq!(provider.accept(next), ConnectionOutcome::Full);
    let mut stale = envelope.clone();
    stale.plan_id = conduit_core::PlanId::from("stale");
    stale.sequence = 1;
    assert_eq!(provider.accept(stale), ConnectionOutcome::Malformed);
    assert!(matches!(
        provider.deliver(),
        Some((ConnectionOutcome::Delivered, _))
    ));
    assert_eq!(provider.queued_bytes(), 0);
    assert_eq!(
        provider.accept(envelope.clone()),
        ConnectionOutcome::Malformed
    );
    let mut out_of_order = envelope.clone();
    out_of_order.sequence = 2;
    assert_eq!(provider.accept(out_of_order), ConnectionOutcome::Malformed);
    let oversized = ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id: plan.plan_id.clone(),
        connection_id: connection.connection_id.clone(),
        sequence: 1,
        value_kind: connection.value_kind.clone(),
        payload: vec![0; 10],
    };
    assert_eq!(provider.accept(oversized), ConnectionOutcome::Malformed);
    assert_eq!(provider.queued_items(), 0);
    assert_eq!(provider.disconnect(), ConnectionOutcome::Disconnected);
    assert_eq!(provider.status(), ConnectionOutcome::Terminal);
    assert_eq!(provider.accept(envelope), ConnectionOutcome::Terminal);
}

#[test]
fn definition_rejects_missing_extra_and_mismatched_children() {
    let definition = composite_definition(2, 18);

    let mut duplicate_child = definition.clone();
    duplicate_child
        .children
        .push(duplicate_child.children[0].clone());
    assert!(CompositeHost::from_definition(duplicate_child, child_runtimes(), 16).is_err());

    let mut runtimes = child_runtimes();
    runtimes.pop();
    assert!(CompositeHost::from_definition(definition.clone(), runtimes, 16).is_err());

    let mut missing_declared_child = definition.clone();
    missing_declared_child.children.pop();
    assert!(CompositeHost::from_definition(missing_declared_child, child_runtimes(), 16).is_err());

    let mut extra_declared_child = definition.clone();
    extra_declared_child.children.push(ChildHostBinding {
        host_id: HostId::from("unused-child"),
    });
    let mut runtimes = child_runtimes();
    runtimes.push(HostRuntime::new(
        child_advertisement("unused-child", "unused-boot", true),
        signal_registry(
            ImplementationId::from("test/pulse-v1"),
            ImplementationId::from("unused/show-v1"),
        )
        .expect("unused child registry installs"),
        16,
    ));
    assert!(CompositeHost::from_definition(extra_declared_child, runtimes, 16).is_err());

    let mut mismatched_boundary = definition;
    mismatched_boundary.boundary.output_faces[0].internal_child = HostId::from("child-sink");
    assert!(CompositeHost::from_definition(mismatched_boundary, child_runtimes(), 16).is_err());

    let mut mismatched_fragment = composite_definition(2, 18);
    mismatched_fragment
        .internal_plan
        .fragments
        .iter_mut()
        .find(|fragment| fragment.host_id.as_str() == "child-source")
        .expect("source fragment exists")
        .boot_id = BootId::from("wrong-child-boot");
    assert!(CompositeHost::from_definition(mismatched_fragment, child_runtimes(), 16).is_err());
}

#[test]
fn definition_runs_three_plan_used_children_without_new_role_fields() {
    let plan = three_child_internal_plan();
    let connection = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .next()
        .expect("three-child plan has its exposed boundary")
        .clone();
    let mut definition = composite_definition(4, 64);
    definition.internal_plan = plan;
    definition.children.push(ChildHostBinding {
        host_id: HostId::from("child-auxiliary"),
    });
    let output_placement = definition
        .internal_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.operation_id.as_str() == "pulse")
        .expect("pulse placement exists");
    definition.boundary.output_faces[0].internal_child = output_placement.host_id.clone();
    definition.boundary.output_faces[0].internal_placement_id =
        output_placement.placement_id.clone();
    let input_placement = definition
        .internal_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.operation_id.as_str() == "show")
        .expect("show placement exists");
    definition.boundary.input_faces[0].internal_child = input_placement.host_id.clone();
    definition.boundary.input_faces[0].internal_placement_id = input_placement.placement_id.clone();
    let _ = connection;

    let mut runtimes = child_runtimes();
    runtimes.push(HostRuntime::new(
        child_advertisement("child-auxiliary", "auxiliary-boot", true),
        signal_registry(
            ImplementationId::from("test/pulse-v1"),
            ImplementationId::from("unused/show-v1"),
        )
        .expect("auxiliary registry installs"),
        32,
    ));
    let mut composite = CompositeHost::from_definition(definition, runtimes, 64)
        .expect("three-child composite is valid");
    let fragment = parent_fragment(&composite);
    composite.handle(HostCommand::Prepare(fragment.clone()));
    let output = composite.handle(HostCommand::Activate(fragment.plan_id));
    assert!(output.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Completed,
            ..
        }
    )));
    assert!(composite
        .internal_observations()
        .contains_key(&HostId::from("child-auxiliary")));
}

#[test]
fn two_child_hosts_compose_and_parent_sees_one_host() {
    let mut composite = composite(4, 64);
    let fragment = parent_fragment(&composite);
    let plan_id = fragment.plan_id.clone();
    assert!(matches!(
        composite
            .handle(HostCommand::Prepare(fragment))
            .events
            .first(),
        Some(HostEvent::Prepared { .. })
    ));
    let output = composite.handle(HostCommand::Activate(plan_id));
    assert!(output.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Completed,
            ..
        }
    )));
    assert!(!format!("{output:?}").contains("child-source"));
    assert!(!format!("{output:?}").contains("child-sink"));
    let observations = composite.internal_observations();
    let source = &observations[&HostId::from("child-source")];
    let sink = &observations[&HostId::from("child-sink")];
    assert!(source
        .iter()
        .all(|item| item.host_id.as_str() == "child-source"));
    assert!(sink
        .iter()
        .all(|item| item.host_id.as_str() == "child-sink"));
    assert_eq!(
        sink.iter()
            .filter(|item| matches!(
                item.kind,
                conduit_core::ObservationKind::ValuePresented { .. }
            ))
            .count(),
        3
    );
}

#[test]
fn parent_planning_cannot_address_an_internal_child_identity() {
    let composite = composite(4, 64);
    let form = CheckedForm {
        source_document_id: conduit_core::SourceDocumentId::from("parent-child-leak-source"),
        checked_form_id: CheckedFormId::from("parent-child-leak-form"),
        expanded_form_id: conduit_core::ExpandedFormId::from("parent-child-leak-expanded"),
        name: "parent-child-leak".into(),
        operations: vec![CheckedOperation {
            startup_parameters: vec![],
            shorthand: None,
            operation_id: OperationId::from("run"),
            kind_id: KindId::from(COMPOSITE_DEMONSTRATION_KIND),
            kind_contract_revision: KindContractRevision::from(format!(
                "{COMPOSITE_DEMONSTRATION_KIND}@1"
            )),
            inputs: Vec::new(),
            outputs: Vec::new(),
            configuration: Vec::new(),
            pool_references: Vec::new(),
        }],
        connections: Vec::new(),
        exports: Vec::new(),
        nested_forms: Vec::new(),
    };
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([(
            OperationId::from("run"),
            PlacementChoice {
                host_id: HostId::from("child-source"),
                capability_id: CapabilityId::from("pulse"),
            },
        )]),
    };
    assert!(plan(
        &form,
        std::slice::from_ref(composite.advertisement()),
        &placements,
        &[ConnectionProvider::Local, ConnectionProvider::InMemory],
    )
    .is_err());
}

#[test]
fn child_failure_is_translated_without_topology_leakage() {
    let mut composite = composite(4, 64);
    composite.fail_next_presentation();
    let fragment = parent_fragment(&composite);
    let output = composite.handle(HostCommand::Prepare(fragment.clone()));
    assert!(matches!(
        output.events.first(),
        Some(HostEvent::Prepared { .. })
    ));
    assert!(!format!("{output:?}").contains("child-source"));
    assert!(!format!("{output:?}").contains("child-sink"));
    let output = composite.handle(HostCommand::Activate(fragment.plan_id));
    let failures = output
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                HostEvent::PlanTerminated {
                    disposition: TerminalDisposition::Failed {
                        reason: conduit_core::FailureReason::CompositeCapabilityFailed
                    },
                    ..
                }
            )
        })
        .count();
    assert_eq!(failures, 1);
    assert!(!format!("{output:?}").contains("child-sink"));
    let observations = composite.internal_observations();
    let sink = &observations[&HostId::from("child-sink")];
    assert!(sink
        .iter()
        .any(|item| matches!(item.kind, conduit_core::ObservationKind::Failure { .. })));
}

#[test]
fn external_limits_are_conservatively_derived() {
    let narrow = composite(2, 18);
    let wide = composite(4, 64);
    let narrow_limits = &narrow.advertisement().capabilities[0].limits;
    let wide_limits = &wide.advertisement().capabilities[0].limits;
    assert_eq!(narrow_limits.max_active_instances, 1);
    assert_eq!(narrow_limits.max_queue_items, 2);
    assert_eq!(narrow_limits.max_queue_bytes, 18);
    assert_eq!(wide_limits.max_queue_items, 4);
    assert_eq!(wide_limits.max_queue_bytes, 64);

    let definition = composite_definition(4, 64);
    let mut source_ad = child_advertisement("child-source", "source-boot", true);
    source_ad.capabilities.push(CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("unrelated-narrow-capability"),
        kind_id: kind_id("unrelated/kind"),
        kind_contract_revision: KindContractRevision::from("unrelated/kind@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("unrelated/profile@1"),
            implementation_id: ImplementationId::from("unrelated/implementation"),
            artifact_id: ArtifactId::from("unrelated/artifact"),
        },
        inputs: vec![],
        outputs: vec![],
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 0,
            max_queue_items: 0,
            max_queue_bytes: 0,
        },
    });
    let sink_ad = child_advertisement("child-sink", "sink-boot", false);
    let relevant_only = CompositeHost::from_definition(
        definition,
        child_runtimes_with_advertisements(source_ad, sink_ad),
        16,
    )
    .expect("unrelated child capability does not narrow exposed limits");
    assert_eq!(
        relevant_only.advertisement().capabilities[0]
            .limits
            .max_active_instances,
        1
    );
}

#[test]
fn controlled_delivery_fills_then_drains_in_real_composite_flow() {
    let mut composite = composite(2, 18);
    composite.set_delivery_mode(DeliveryMode::Controlled);
    let fragment = parent_fragment(&composite);
    let plan_id = fragment.plan_id.clone();
    composite.handle(HostCommand::Prepare(fragment));
    let activated = composite.handle(HostCommand::Activate(plan_id.clone()));
    assert!(!activated
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::PlanTerminated { .. })));
    assert_eq!(composite.provider_status(), ConnectionOutcome::Full);
    assert_eq!(composite.provider_queued_items(), 2);
    assert_eq!(composite.provider_queued_bytes(), 18);
    let before_delivery = composite.internal_observations();
    assert_eq!(
        before_delivery[&HostId::from("child-sink")]
            .iter()
            .filter(|item| matches!(
                item.kind,
                conduit_core::ObservationKind::ValuePresented { .. }
            ))
            .count(),
        0
    );
    assert!(composite.internal_events().iter().any(|(host_id, event)| {
        host_id.as_str() == "child-source" && matches!(event, HostEvent::ConnectionBlocked { .. })
    }));

    let first_delivery = composite.deliver_next(&plan_id);
    assert!(!first_delivery
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::PlanTerminated { .. })));
    assert_eq!(composite.provider_queued_items(), 2);
    assert_eq!(composite.provider_queued_bytes(), 18);

    let mut terminal = false;
    for _ in 0..3 {
        let output = composite.deliver_next(&plan_id);
        terminal |= output.events.iter().any(|event| {
            matches!(
                event,
                HostEvent::PlanTerminated {
                    disposition: TerminalDisposition::Completed,
                    ..
                }
            )
        });
        if terminal {
            break;
        }
    }
    assert!(terminal);
    assert_eq!(composite.provider_queued_items(), 0);
    assert_eq!(composite.provider_queued_bytes(), 0);
    let after_delivery = composite.internal_observations();
    assert_eq!(
        after_delivery[&HostId::from("child-sink")]
            .iter()
            .filter(|item| matches!(
                item.kind,
                conduit_core::ObservationKind::ValuePresented { .. }
            ))
            .count(),
        3
    );
}

#[test]
fn configured_failure_translation_is_the_only_parent_failure_contract() {
    let mut definition = composite_definition(4, 64);
    definition.failure_translation = conduit_core::FailureReason::RequiredBranchFailed;
    let mut composite = CompositeHost::from_definition(definition, child_runtimes(), 64)
        .expect("configured composite is valid");
    composite.fail_next_presentation();
    let fragment = parent_fragment(&composite);
    composite.handle(HostCommand::Prepare(fragment.clone()));
    let output = composite.handle(HostCommand::Activate(fragment.plan_id));
    assert!(output.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Failed {
                reason: conduit_core::FailureReason::RequiredBranchFailed
            },
            ..
        }
    )));
    let rendered = format!("{output:?}");
    assert!(!rendered.contains("child-source"));
    assert!(!rendered.contains("child-sink"));
}

#[test]
fn malformed_sink_delivery_becomes_source_failure_without_topology_leakage() {
    let mut composite = composite(2, 18);
    composite.set_delivery_mode(DeliveryMode::Controlled);
    let fragment = parent_fragment(&composite);
    let plan_id = fragment.plan_id.clone();
    composite.handle(HostCommand::Prepare(fragment));
    composite.handle(HostCommand::Activate(plan_id.clone()));

    let failed = composite.deliver_next_malformed(&plan_id);
    assert!(
        failed.events.iter().any(|event| matches!(
            event,
            HostEvent::PlanTerminated {
                disposition: TerminalDisposition::Failed {
                    reason: conduit_core::FailureReason::CompositeCapabilityFailed
                },
                ..
            }
        )),
        "{failed:?}"
    );
    assert!(!format!("{failed:?}").contains("child-source"));
    assert!(!format!("{failed:?}").contains("child-sink"));
    let observations = composite.internal_observations();
    assert!(observations[&HostId::from("child-source")]
        .iter()
        .any(|item| matches!(
            &item.kind,
            conduit_core::ObservationKind::ConnectionTerminal { disposition }
                if matches!(
                    disposition.disposition,
                    TerminalDisposition::Failed {
                        reason: conduit_core::FailureReason::MalformedConnectionEnvelope
                    }
                )
        )));
}

#[test]
fn disconnect_with_empty_provider_fails_composite_and_rejects_future_delivery() {
    let mut composite = composite(2, 18);
    composite.set_delivery_mode(DeliveryMode::Controlled);
    let fragment = parent_fragment(&composite);
    let plan_id = fragment.plan_id.clone();
    composite.handle(HostCommand::Prepare(fragment));
    let disconnected = composite.disconnect_provider(&plan_id);
    assert_eq!(composite.provider_status(), ConnectionOutcome::Terminal);
    assert!(disconnected
        .events
        .iter()
        .all(|event| !matches!(event, HostEvent::PlanTerminated { .. })));
    let activated = composite.handle(HostCommand::Activate(plan_id));
    assert!(
        activated.events.iter().any(|event| matches!(
            event,
            HostEvent::PlanTerminated {
                disposition: TerminalDisposition::Failed {
                    reason: conduit_core::FailureReason::CompositeCapabilityFailed
                },
                ..
            }
        )),
        "{activated:?}"
    );
}

#[test]
fn controlled_delivery_releases_bytes_only_on_delivery_or_disconnect() {
    let mut composite = composite(4, 64);
    composite.set_delivery_mode(DeliveryMode::Controlled);
    let fragment = parent_fragment(&composite);
    let plan_id = fragment.plan_id.clone();
    composite.handle(HostCommand::Prepare(fragment));
    composite.handle(HostCommand::Activate(plan_id.clone()));
    assert_eq!(composite.provider_queued_bytes(), 27);
    composite.deliver_next(&plan_id);
    assert_eq!(composite.provider_queued_bytes(), 18);
    let failed = composite.disconnect_provider(&plan_id);
    assert_eq!(composite.provider_queued_bytes(), 0);
    assert_eq!(composite.provider_status(), ConnectionOutcome::Terminal);
    assert!(failed.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Failed {
                reason: conduit_core::FailureReason::CompositeCapabilityFailed
            },
            ..
        }
    )));
    let observations = composite.internal_observations();
    assert!(observations[&HostId::from("child-source")]
        .iter()
        .any(|item| matches!(
            &item.kind,
            conduit_core::ObservationKind::ConnectionTerminal { disposition }
                if disposition.undeliverable_items == 2
                    && matches!(
                        disposition.disposition,
                        TerminalDisposition::Failed {
                            reason: conduit_core::FailureReason::ConnectionDisconnected
                        }
                    )
        )));
}

#[test]
fn definition_data_can_expose_a_different_composite_capability() {
    let plan = internal_plan(2, 18);
    let connection = plan.fragments[0].connections[0].clone();
    let source_ad = child_advertisement("child-source", "source-boot", true);
    let sink_ad = child_advertisement("child-sink", "sink-boot", false);
    let mut alternate_input = show_inputs()[0].clone();
    alternate_input.port_id = conduit_core::port_id("signal-in");
    let definition = CompositeDefinition {
        host_id: HostId::from("alternate-composite"),
        boot_id: BootId::from("alternate-boot"),
        offer_generation: OfferGeneration(3),
        profile: HostProfileId::from("composite/test-alternate"),
        external_capability: CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from("alternate-capability"),
            kind_id: kind_id("demonstration/alternate"),
            kind_contract_revision: KindContractRevision::from("demonstration/alternate@1"),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from("composite/alternate-hosted@1"),
                implementation_id: ImplementationId::from("composite/alternate-v1"),
                artifact_id: ArtifactId::from("composite/alternate-artifact-v1"),
            },
            inputs: vec![alternate_input.clone()],
            outputs: pulse_outputs(),
            host_operations: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 5,
                max_queue_items: 9,
                max_queue_bytes: 99,
            },
        },
        children: vec![
            ChildHostBinding {
                host_id: source_ad.host_id.clone(),
            },
            ChildHostBinding {
                host_id: sink_ad.host_id.clone(),
            },
        ],
        internal_plan: plan,
        boundary: CompositeBoundary {
            input_faces: vec![CompositeFaceBinding {
                external_port: alternate_input,
                internal_child: sink_ad.host_id.clone(),
                internal_placement_id: connection.sink_placement_id.clone(),
                internal_port_id: connection.sink_port_id.clone(),
                terminal: CompositeFaceTerminal::Independent,
            }],
            output_faces: vec![CompositeFaceBinding {
                external_port: pulse_outputs()[0].clone(),
                internal_child: source_ad.host_id.clone(),
                internal_placement_id: connection.source_placement_id.clone(),
                internal_port_id: connection.source_port_id.clone(),
                terminal: CompositeFaceTerminal::Independent,
            }],
        },
        failure_translation: conduit_core::FailureReason::RequiredBranchFailed,
    };
    let host = CompositeHost::from_definition(
        definition,
        vec![
            HostRuntime::new(
                source_ad,
                signal_registry(
                    ImplementationId::from("test/pulse-v1"),
                    ImplementationId::from("unused/show-v1"),
                )
                .expect("source registry installs"),
                64,
            ),
            HostRuntime::new(
                sink_ad,
                signal_registry(
                    ImplementationId::from("unused/pulse-v1"),
                    ImplementationId::from("test/show-v1"),
                )
                .expect("sink registry installs"),
                64,
            ),
        ],
        64,
    )
    .expect("data-driven composite builds");
    let capability = &host.advertisement().capabilities[0];
    assert_eq!(capability.kind_id.as_str(), "demonstration/alternate");
    assert_eq!(capability.limits.max_active_instances, 1);
    assert_eq!(capability.limits.max_queue_items, 8);
    assert_eq!(capability.limits.max_queue_bytes, 99);
}
