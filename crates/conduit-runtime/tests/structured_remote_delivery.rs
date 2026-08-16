use std::collections::BTreeMap;

use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, process_owned_line_offer, resource_offer,
    resource_requirement, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConnectionBase, ConnectionOutcome, ExecutionProfileId, FailureReason, GearId,
    HostAdvertisement, HostCommand, HostId, HostProfileId, ImplementationId, ImplementationOffer,
    KindContractRevision, KindId, OfferGeneration, PlatformEffect, PortDescriptor, PortDirection,
    PortTemporal, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoValue, StructuredVariantCase, ValuePayload, PRESENTATION_RESOURCE_CLASS,
    PROTOCOL_VERSION,
};
use conduit_form::{parse, KindDefinition, ProfileCatalog};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_runtime::{
    HostRuntime, ImplementationFailure, ImplementationRegistry, OperationAction,
    OperationCompletion, OperationImplementation, OperationOutput, OperationState,
};
use conduit_wire::{
    decode_envelope, encode_envelope, structured_local_envelope_from_transport,
    structured_transport_envelope_from_local,
};

const SOURCE_KIND: &str = "test/structured-source";
const SINK_KIND: &str = "test/structured-sink";
const SOURCE_CONTRACT: &str = "test/structured-source@1";
const SINK_CONTRACT: &str = "test/structured-sink@1";
const MAXIMUM_WIRE_BYTES: u32 = 2_048;

fn count_type() -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap()
}

fn text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from("value/text@1")).unwrap()
}

fn music_value() -> StructuredInfoValue {
    let note_type = StructuredInfoType::record(
        KindId::from("music/note@1"),
        vec![StructuredFieldType::new("pitch", count_type()).unwrap()],
    )
    .unwrap();
    let event_type = StructuredInfoType::variant(
        KindId::from("music/event@1"),
        vec![StructuredVariantCase::new("note_on", note_type.clone()).unwrap()],
    )
    .unwrap();
    let note = StructuredInfoValue::record(
        note_type,
        vec![StructuredFieldValue::new(
            "pitch",
            StructuredInfoValue::leaf(count_type(), 60_u64.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    StructuredInfoValue::variant(event_type, "note_on", note).unwrap()
}

fn llm_value() -> StructuredInfoValue {
    let output_type = StructuredInfoType::record(
        KindId::from("llm/extraction@1"),
        vec![StructuredFieldType::new("label", text_type()).unwrap()],
    )
    .unwrap();
    StructuredInfoValue::record(
        output_type,
        vec![StructuredFieldValue::new(
            "label",
            StructuredInfoValue::leaf(text_type(), b"ready".to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
}

fn ports(value_kind: &KindId, direction: PortDirection) -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("value"),
        value_kind: value_kind.clone(),
        direction,
        temporal: PortTemporal::Value,
    }]
}

fn catalog(value_kind: &KindId) -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(SOURCE_CONTRACT),
            inputs: Vec::new(),
            outputs: ports(value_kind, PortDirection::Output),
            configuration: Vec::new(),
        })
        .unwrap();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindContractRevision::from(SINK_CONTRACT),
            inputs: ports(value_kind, PortDirection::Input),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .unwrap();
    catalog
}

fn advertisement(host: &str, source: bool, value_kind: &KindId) -> HostAdvertisement {
    let (capability_id, kind, contract, profile, implementation, inputs, outputs, operations) =
        if source {
            (
                "structured-source",
                SOURCE_KIND,
                SOURCE_CONTRACT,
                "test/structured-source-hosted@1",
                "test/structured-source-v1",
                Vec::new(),
                ports(value_kind, PortDirection::Output),
                Vec::new(),
            )
        } else {
            (
                "structured-sink",
                SINK_KIND,
                SINK_CONTRACT,
                "test/structured-sink-hosted@1",
                "test/structured-sink-v1",
                ports(value_kind, PortDirection::Input),
                Vec::new(),
                vec![present_host_operation_requirement(
                    value_kind.clone(),
                    MAXIMUM_WIRE_BYTES,
                )],
            )
        };
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(format!("{host}/boot")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/structured-remote@1"),
        resources: (!source)
            .then(|| resource_offer("presentation/slot", PRESENTATION_RESOURCE_CLASS, 1))
            .into_iter()
            .collect(),
        planner_capabilities: Vec::new(),
        capabilities: vec![CapabilityOffer {
            startup_parameters: Vec::new(),
            shorthand: None,
            capability_id: CapabilityId::from(capability_id),
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(contract),
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(profile),
                implementation_id: ImplementationId::from(implementation),
                artifact_id: ArtifactId::from(format!("{implementation}/artifact")),
            },
            inputs,
            outputs,
            host_operations: operations,
            resource_requirements: (!source)
                .then(|| resource_requirement(PRESENTATION_RESOURCE_CLASS, 1))
                .into_iter()
                .collect(),
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 2,
                max_queue_bytes: MAXIMUM_WIRE_BYTES,
            },
        }],
    }
}

struct SourceImplementation {
    value: StructuredInfoValue,
    kind: KindId,
    implementation: ImplementationId,
    artifact: ArtifactId,
}

impl OperationImplementation for SourceImplementation {
    fn kind_id(&self) -> &KindId {
        &self.kind
    }
    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from(SOURCE_CONTRACT)
    }
    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from("test/structured-source-hosted@1")
    }
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact
    }
    fn prepare(
        &self,
        _placement: &conduit_core::PlannedGear,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(SourceState {
            value: self.value.clone(),
            emitted: false,
        }))
    }
    fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
        (value_kind == self.value.value_type().profile().unwrap().value_kind()).then_some(1)
    }
}

struct SourceState {
    value: StructuredInfoValue,
    emitted: bool,
}

impl OperationState for SourceState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Emit(vec![OperationOutput {
            port: port_id("value"),
            value: ValuePayload {
                value_kind: self
                    .value
                    .value_type()
                    .profile()
                    .unwrap()
                    .value_kind()
                    .clone(),
                encoded: self.value.canonical_bytes().unwrap(),
            },
        }])
    }
    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        if completion == OperationCompletion::Emitted && !self.emitted {
            self.emitted = true;
            OperationAction::Complete
        } else {
            OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "structured source completion drifted",
            ))
        }
    }
}

struct SinkImplementation {
    value_kind: KindId,
    implementation: ImplementationId,
    artifact: ArtifactId,
}

impl OperationImplementation for SinkImplementation {
    fn kind_id(&self) -> &KindId {
        static KIND: std::sync::OnceLock<KindId> = std::sync::OnceLock::new();
        KIND.get_or_init(|| kind_id(SINK_KIND))
    }
    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from(SINK_CONTRACT)
    }
    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from("test/structured-sink-hosted@1")
    }
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact
    }
    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        vec![present_host_operation_requirement(
            self.value_kind.clone(),
            MAXIMUM_WIRE_BYTES,
        )]
    }
    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)]
    }
    fn prepare(
        &self,
        _placement: &conduit_core::PlannedGear,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(SinkState))
    }
    fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
        (value_kind == &self.value_kind).then_some(1)
    }
}

struct SinkState;

impl OperationState for SinkState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }
    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { value, .. } => OperationAction::Present {
                presentation_kind: value.value_kind.clone(),
                value,
            },
            OperationCompletion::PresentationCompleted { success: true, .. } => {
                OperationAction::Idle
            }
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "structured sink completion drifted",
            )),
        }
    }
}

fn registry(value: &StructuredInfoValue, source: bool) -> ImplementationRegistry {
    let mut registry = ImplementationRegistry::new();
    if source {
        registry
            .install(SourceImplementation {
                value: value.clone(),
                kind: kind_id(SOURCE_KIND),
                implementation: ImplementationId::from("test/structured-source-v1"),
                artifact: ArtifactId::from("test/structured-source-v1/artifact"),
            })
            .unwrap();
    } else {
        registry
            .install(SinkImplementation {
                value_kind: value.value_type().profile().unwrap().value_kind().clone(),
                implementation: ImplementationId::from("test/structured-sink-v1"),
                artifact: ArtifactId::from("test/structured-sink-v1/artifact"),
            })
            .unwrap();
    }
    registry
}

fn prove_live_delivery(value: StructuredInfoValue) {
    let value_kind = value.value_type().profile().unwrap().value_kind().clone();
    let source_host = advertisement("structured-source-host", true, &value_kind);
    let sink_host = advertisement("structured-sink-host", false, &value_kind);
    let form = parse(
        "form 0\n\nremote {\n source: test/structured-source\n sink: test/structured-sink\n source > sink\n}\n",
        &catalog(&value_kind),
    )
    .unwrap();
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("source"),
                PlacementChoice {
                    host_id: source_host.host_id.clone(),
                    capability_id: CapabilityId::from("structured-source"),
                },
            ),
            (
                GearId::from("sink"),
                PlacementChoice {
                    host_id: sink_host.host_id.clone(),
                    capability_id: CapabilityId::from("structured-sink"),
                },
            ),
        ]),
    };
    let line = process_owned_line_offer(
        "line/structured",
        "link/structured",
        ConnectionBase::InMemory,
        "fixture/in-memory/structured",
        &source_host,
        &sink_host,
        2,
        MAXIMUM_WIRE_BYTES,
    );
    let plan = plan_with_line_offers(
        &form,
        &[source_host.clone(), sink_host.clone()],
        &placements,
        &[ConnectionBase::InMemory],
        2,
        MAXIMUM_WIRE_BYTES,
        std::slice::from_ref(&line),
    )
    .unwrap();
    let source_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == source_host.host_id)
        .unwrap()
        .clone();
    let sink_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == sink_host.host_id)
        .unwrap()
        .clone();
    let mut source = HostRuntime::new_with_external_state(
        source_host,
        registry(&value, true),
        64,
        Vec::new(),
        vec![line.clone()],
    );
    let mut sink = HostRuntime::new_with_external_state(
        sink_host,
        registry(&value, false),
        64,
        Vec::new(),
        vec![line],
    );
    source.handle(HostCommand::Prepare(source_fragment.clone()));
    sink.handle(HostCommand::Prepare(sink_fragment.clone()));
    sink.handle(HostCommand::StartPlay(sink_fragment.plan_id.clone()));
    let source_output = source.handle(HostCommand::StartPlay(source_fragment.plan_id.clone()));
    let local = source_output
        .effects
        .into_iter()
        .find_map(|effect| match effect {
            PlatformEffect::TransmitConnection { envelope } => Some(envelope),
            _ => None,
        })
        .expect("source runtime offers the planned remote value");
    assert_eq!(local.payload, value.canonical_bytes().unwrap());

    let transport = structured_transport_envelope_from_local(local, MAXIMUM_WIRE_BYTES).unwrap();
    assert_ne!(transport.payload, value.canonical_bytes().unwrap());
    let frame = encode_envelope(&transport, MAXIMUM_WIRE_BYTES).unwrap();
    let received = decode_envelope(&frame, MAXIMUM_WIRE_BYTES).unwrap();
    let local = structured_local_envelope_from_transport(received, MAXIMUM_WIRE_BYTES).unwrap();
    assert_eq!(local.payload, value.canonical_bytes().unwrap());
    let plan_id = local.plan_id.clone();
    let connection_id = local.connection_id.clone();
    let sequence = local.sequence;
    let sink_output = sink.handle(HostCommand::AcceptConnectionEnvelope(local));
    assert!(sink_output.events.iter().any(|event| matches!(
        event,
        conduit_core::HostEvent::ConnectionEnvelopeOutcome {
            outcome: ConnectionOutcome::Accepted,
            ..
        }
    )));
    let presented = sink_output
        .effects
        .into_iter()
        .find_map(|effect| match effect {
            PlatformEffect::PresentValue { value, .. } => Some(value),
            _ => None,
        })
        .expect("peer runtime consumes the restored canonical value");
    assert_eq!(presented.encoded, value.canonical_bytes().unwrap());
    let accepted = source.handle(HostCommand::CompleteConnectionDelivery {
        plan_id: plan_id.clone(),
        connection_id: connection_id.clone(),
        sequence,
        outcome: ConnectionOutcome::Accepted,
    });
    assert!(accepted.events.iter().any(|event| matches!(
        event,
        conduit_core::HostEvent::ConnectionEnvelopeOutcome {
            outcome: ConnectionOutcome::Accepted,
            ..
        }
    )));
    let delivered = source.handle(HostCommand::CompleteConnectionDelivery {
        plan_id,
        connection_id,
        sequence,
        outcome: ConnectionOutcome::Delivered,
    });
    assert!(!delivered
        .events
        .iter()
        .any(|event| matches!(event, conduit_core::HostEvent::CommandRejected { .. })));
}

#[test]
fn music_and_llm_values_cross_two_live_planned_host_runtimes() {
    prove_live_delivery(music_value());
    prove_live_delivery(llm_value());
}
