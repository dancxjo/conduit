use super::{
    HostRuntime, ImplementationFailure, ImplementationRegistry, OperationAction,
    OperationCompletion, OperationImplementation, OperationOutput, OperationState,
};
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_offer, resource_requirement,
    wait_host_operation_requirement, ArtifactId, BootId, CancellationReason, CapabilityId,
    CapabilityLimits, CapabilityOffer, ConfigurationEntry, ConfigurationValue, ConnectionProvider,
    ExecutionProfileId, FailureReason, HostAdvertisement, HostCommand, HostEvent, HostId,
    HostProfileId, ImplementationId, KindContractRevision, ObservationKind, OfferGeneration,
    PlatformEffect, PortDescriptor, PortDirection, TerminalDisposition, ValuePayload,
    PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};
use conduit_form::{parse, ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
use conduit_planner::{default_placements, plan_with_connection_limits};
use std::collections::{BTreeMap, VecDeque};

const PULSE_KIND: &str = "flow/pulse";
const SHOW_KIND: &str = "presentation/show";
const SIGNAL_VALUE_KIND: &str = "value/signal";
const SIGNAL_PRESENTATION_KIND: &str = "test/presentation";
const SIGNAL_ENCODED_LEN: u32 = 9;
const PULSE_CONTRACT: &str = "test/flow-pulse@1";
const SHOW_CONTRACT: &str = "test/presentation-show@1";
const PULSE_PROFILE: &str = "test/pulse-hosted@1";
const SHOW_PROFILE: &str = "test/show-hosted@1";

fn pulse_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("signal"),
        value_kind: kind_id(SIGNAL_VALUE_KIND),
        direction: PortDirection::Output,
    }]
}

fn show_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("signal"),
        value_kind: kind_id(SIGNAL_VALUE_KIND),
        direction: PortDirection::Input,
    }]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signal {
    sequence: u64,
    level: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestPulseConfiguration {
    count: u64,
    period_ms: u64,
    initial_level: bool,
}

fn encode_signal(signal: &Signal) -> ValuePayload {
    let mut encoded = signal.sequence.to_le_bytes().to_vec();
    encoded.push(u8::from(signal.level));
    ValuePayload {
        value_kind: kind_id(SIGNAL_VALUE_KIND),
        encoded,
    }
}

fn decode_signal(value: &ValuePayload) -> Result<Signal, String> {
    if value.value_kind.as_str() != SIGNAL_VALUE_KIND || value.encoded.len() != 9 {
        return Err("invalid test signal".to_string());
    }
    let mut sequence = [0; 8];
    sequence.copy_from_slice(&value.encoded[..8]);
    Ok(Signal {
        sequence: u64::from_le_bytes(sequence),
        level: value.encoded[8] != 0,
    })
}

fn parse_pulse_configuration(
    entries: &[ConfigurationEntry],
) -> Result<TestPulseConfiguration, String> {
    let get_u64 = |key: &str| {
        entries
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| match entry.value {
                ConfigurationValue::U64(value) => Some(value),
                ConfigurationValue::Bool(_) => None,
            })
            .ok_or_else(|| format!("missing integer '{key}'"))
    };
    let initial_level = entries
        .iter()
        .find(|entry| entry.key == "initial")
        .and_then(|entry| match entry.value {
            ConfigurationValue::Bool(value) => Some(value),
            ConfigurationValue::U64(_) => None,
        })
        .ok_or_else(|| "missing boolean 'initial'".to_string())?;
    Ok(TestPulseConfiguration {
        count: get_u64("count")?,
        period_ms: get_u64("period-ms")?,
        initial_level,
    })
}

fn signal_profile_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(PULSE_KIND),
            kind_contract_revision: KindContractRevision::from(PULSE_CONTRACT),
            inputs: Vec::new(),
            outputs: pulse_outputs(),
            configuration: vec![
                ConfigurationField {
                    key: "count".to_string(),
                    default_value: ConfigurationValue::U64(16),
                    validation: ConfigurationRule::Any,
                },
                ConfigurationField {
                    key: "period-ms".to_string(),
                    default_value: ConfigurationValue::U64(250),
                    validation: ConfigurationRule::Any,
                },
                ConfigurationField {
                    key: "initial".to_string(),
                    default_value: ConfigurationValue::Bool(false),
                    validation: ConfigurationRule::Any,
                },
            ],
        })
        .expect("test pulse kind installs");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(SHOW_KIND),
            kind_contract_revision: KindContractRevision::from(SHOW_CONTRACT),
            inputs: show_inputs(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("test show kind installs");
    catalog
}

fn advertisement(
    boot: &str,
    offer_generation: u64,
    queue_items: u16,
    queue_bytes: u32,
) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("std-host-1"),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(offer_generation),
        profile: HostProfileId::from("rust-std"),
        resources: vec![
            resource_offer("test/presentation", PRESENTATION_RESOURCE_CLASS, 8),
            resource_offer("test/timer", TIMER_RESOURCE_CLASS, 8),
        ],
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("pulse-1"),
                kind_id: kind_id(PULSE_KIND),
                kind_contract_revision: KindContractRevision::from(PULSE_CONTRACT),
                execution_profile_id: ExecutionProfileId::from(PULSE_PROFILE),
                implementation_id: ImplementationId::from("std/pulse-v1"),
                artifact_id: ArtifactId::from("test/pulse-artifact-v1"),
                inputs: vec![],
                outputs: pulse_outputs(),
                host_operations: vec![wait_host_operation_requirement()],
                resource_requirements: vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 8,
                    max_queue_items: queue_items,
                    max_queue_bytes: queue_bytes,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("stdout-show-1"),
                kind_id: kind_id(SHOW_KIND),
                kind_contract_revision: KindContractRevision::from(SHOW_CONTRACT),
                execution_profile_id: ExecutionProfileId::from(SHOW_PROFILE),
                implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                artifact_id: ArtifactId::from("test/show-artifact-v1"),
                inputs: show_inputs(),
                outputs: vec![],
                host_operations: vec![present_host_operation_requirement(
                    kind_id(SIGNAL_PRESENTATION_KIND),
                    SIGNAL_ENCODED_LEN,
                )],
                resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 8,
                    max_queue_items: queue_items,
                    max_queue_bytes: queue_bytes,
                },
            },
        ],
    }
}

fn test_runtime(advertisement: HostAdvertisement, observation_limit: usize) -> HostRuntime {
    let mut registry = ImplementationRegistry::new();
    registry
        .install(TestPulseImplementation {
            kind_id: kind_id(PULSE_KIND),
            implementation_id: ImplementationId::from("std/pulse-v1"),
            artifact_id: ArtifactId::from("test/pulse-artifact-v1"),
        })
        .expect("pulse implementation installs");
    registry
        .install(TestShowImplementation {
            kind_id: kind_id(SHOW_KIND),
            implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
            artifact_id: ArtifactId::from("test/show-artifact-v1"),
        })
        .expect("show implementation installs");
    HostRuntime::new(advertisement, registry, observation_limit)
}

struct TestPulseImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
}

impl OperationImplementation for TestPulseImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from(PULSE_CONTRACT)
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from(PULSE_PROFILE)
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        vec![wait_host_operation_requirement()]
    }

    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)]
    }

    fn prepare(
        &self,
        placement: &conduit_core::PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(TestPulseState {
            configuration: parse_pulse_configuration(&placement.configuration).map_err(
                |error| {
                    ImplementationFailure::new(
                        FailureReason::InvalidOperationConfiguration,
                        error.to_string(),
                    )
                },
            )?,
            next_sequence: 0,
        }))
    }

    fn minimum_value_size(&self, value_kind: &conduit_core::KindId) -> Option<u32> {
        (value_kind.as_str() == SIGNAL_VALUE_KIND).then_some(SIGNAL_ENCODED_LEN)
    }
}

struct TestPulseState {
    configuration: TestPulseConfiguration,
    next_sequence: u64,
}

impl TestPulseState {
    fn next(&self) -> OperationAction {
        if self.next_sequence >= self.configuration.count {
            OperationAction::Complete
        } else {
            OperationAction::Emit(vec![OperationOutput {
                port: port_id("signal"),
                value: encode_signal(&Signal {
                    sequence: self.next_sequence,
                    level: if self.next_sequence.is_multiple_of(2) {
                        self.configuration.initial_level
                    } else {
                        !self.configuration.initial_level
                    },
                }),
            }])
        }
    }
}

impl OperationState for TestPulseState {
    fn start(&mut self) -> OperationAction {
        self.next()
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Emitted => {
                self.next_sequence += 1;
                if self.next_sequence >= self.configuration.count {
                    OperationAction::Complete
                } else if self.configuration.period_ms > 0 {
                    OperationAction::Wait {
                        duration_ms: self.configuration.period_ms,
                    }
                } else {
                    self.next()
                }
            }
            OperationCompletion::TimerElapsed => self.next(),
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "unexpected pulse completion",
            )),
        }
    }
}

struct TestShowImplementation {
    kind_id: conduit_core::KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
}

impl OperationImplementation for TestShowImplementation {
    fn kind_id(&self) -> &conduit_core::KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> KindContractRevision {
        KindContractRevision::from(SHOW_CONTRACT)
    }

    fn execution_profile_id(&self) -> ExecutionProfileId {
        ExecutionProfileId::from(SHOW_PROFILE)
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        vec![present_host_operation_requirement(
            kind_id(SIGNAL_PRESENTATION_KIND),
            SIGNAL_ENCODED_LEN,
        )]
    }

    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)]
    }

    fn prepare(
        &self,
        _placement: &conduit_core::PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(TestShowState { expected: 0 }))
    }

    fn minimum_value_size(&self, value_kind: &conduit_core::KindId) -> Option<u32> {
        (value_kind.as_str() == SIGNAL_VALUE_KIND).then_some(SIGNAL_ENCODED_LEN)
    }
}

struct TestShowState {
    expected: u64,
}

impl OperationState for TestShowState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { port, value } if port.as_str() == "signal" => {
                let signal = decode_signal(&value).expect("test signal decodes");
                if signal.sequence != self.expected {
                    return OperationAction::Fail(ImplementationFailure::new(
                        FailureReason::MalformedConnectionEnvelope,
                        "out-of-order signal",
                    ));
                }
                OperationAction::Present {
                    presentation_kind: kind_id(SIGNAL_PRESENTATION_KIND),
                    value,
                }
            }
            OperationCompletion::PresentationCompleted { success: true, .. } => {
                self.expected += 1;
                OperationAction::Idle
            }
            OperationCompletion::PresentationCompleted {
                success: false,
                message,
            } => OperationAction::Fail(ImplementationFailure {
                reason: FailureReason::ManifestationFailed,
                message,
            }),
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "unexpected show completion",
            )),
        }
    }
}

fn demo_fragment(
    form_source: &str,
    queue_items: u16,
    queue_bytes: u32,
) -> conduit_core::PlanFragment {
    let form = parse(form_source, &signal_profile_catalog()).expect("form should parse");
    let advertisement = advertisement("boot-1", 1, 8, 256);
    let placements =
        default_placements(&form, std::slice::from_ref(&advertisement)).expect("placements work");
    let plan = plan_with_connection_limits(
        &form,
        std::slice::from_ref(&advertisement),
        &placements,
        &[ConnectionProvider::Local],
        queue_items,
        queue_bytes,
    )
    .expect("plan should succeed");
    plan.fragments.first().expect("fragment exists").clone()
}

fn inspect(runtime: &mut HostRuntime) -> Vec<conduit_core::Observation> {
    runtime
        .handle(HostCommand::Inspect)
        .events
        .into_iter()
        .find_map(|event| match event {
            HostEvent::Observations { items } => Some(items),
            _ => None,
        })
        .expect("observations must exist")
}

fn drive_success(runtime: &mut HostRuntime, plan_id: conduit_core::PlanId) -> Vec<Signal> {
    let output = runtime.handle(HostCommand::Activate(plan_id));
    let mut presented = Vec::new();
    let mut pending_effects = output.effects;
    while let Some(effect) = pending_effects.pop() {
        let follow_up = match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                ..
            } => runtime.handle(HostCommand::CompleteWait {
                plan_id,
                placement_id,
            }),
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                ..
            } => {
                presented.push(decode_signal(&value).expect("signal payload must decode"));
                runtime.handle(HostCommand::CompletePresentation {
                    plan_id,
                    active_play_id,
                    presentation_id,
                    placement_id,
                    value,
                    success: true,
                    message: None,
                })
            }
            PlatformEffect::TransmitConnection { .. } => {
                panic!("local test plan must not transmit remotely")
            }
        };
        pending_effects.extend(follow_up.effects.into_iter().rev());
    }
    presented
}

fn drive_with_failure(
    runtime: &mut HostRuntime,
    plan_id: conduit_core::PlanId,
    failed_placement: &conduit_core::PlacementId,
    failed_sequence: u64,
) -> Vec<HostEvent> {
    let mut all_events = Vec::new();
    let initial = runtime.handle(HostCommand::Activate(plan_id));
    all_events.extend(initial.events);
    let mut pending = VecDeque::from(initial.effects);
    while let Some(effect) = pending.pop_front() {
        let follow_up = match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                ..
            } => runtime.handle(HostCommand::CompleteWait {
                plan_id,
                placement_id,
            }),
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                ..
            } => {
                let signal = decode_signal(&value).expect("signal payload must decode");
                let fail = &placement_id == failed_placement && signal.sequence == failed_sequence;
                runtime.handle(HostCommand::CompletePresentation {
                    plan_id,
                    active_play_id,
                    presentation_id,
                    placement_id,
                    value,
                    success: !fail,
                    message: fail.then(|| "injected failure".to_string()),
                })
            }
            PlatformEffect::TransmitConnection { .. } => {
                panic!("local test plan must not transmit remotely")
            }
        };
        all_events.extend(follow_up.events);
        pending.extend(follow_up.effects);
    }
    all_events
}

#[test]
fn preparation_rejects_stale_boot() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
    let mut runtime = test_runtime(advertisement("boot-2", 1, 4, 64), 128);
    let output = runtime.handle(HostCommand::Prepare(fragment));
    assert!(matches!(
        output.events.first(),
        Some(HostEvent::PreparationRejected { .. })
    ));
}

#[test]
fn preparation_rejects_stale_offer_generation() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
    let mut runtime = test_runtime(advertisement("boot-1", 2, 4, 64), 128);
    let output = runtime.handle(HostCommand::Prepare(fragment));
    assert!(matches!(
        output.events.first(),
        Some(HostEvent::PreparationRejected { .. })
    ));
}

#[test]
fn preparation_rejects_too_small_byte_capacity() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 1\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 8);
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
    let output = runtime.handle(HostCommand::Prepare(fragment));
    assert!(matches!(
        output.events.first(),
        Some(HostEvent::PreparationRejected { .. })
    ));
}

#[test]
fn full_queue_applies_backpressure() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 3\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 1, 64);
    let mut runtime = test_runtime(advertisement("boot-1", 1, 1, 64), 128);
    runtime.handle(HostCommand::Prepare(fragment.clone()));
    let output = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
    assert!(output
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::ConnectionBlocked { .. })));
}

#[test]
fn byte_capacity_applies_backpressure() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 3\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 9);
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
    runtime.handle(HostCommand::Prepare(fragment.clone()));
    let output = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
    assert!(output
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::ConnectionBlocked { .. })));
}

#[test]
fn multiple_sources_remain_independent() {
    let fragment = demo_fragment("form 0\n\ndouble-demo {\n    pulse-a: flow/pulse\n    show-a: presentation/show\n    pulse-b: flow/pulse\n    show-b: presentation/show\n\n    pulse-a.count = 3\n    pulse-a.period-ms = 0\n    pulse-a.initial = false\n    pulse-b.count = 5\n    pulse-b.period-ms = 0\n    pulse-b.initial = true\n\n    pulse-a > show-a\n    pulse-b > show-b\n}\n", 4, 64);
    let placement_by_operation = fragment
        .placements
        .iter()
        .map(|placement| {
            (
                placement.operation_id.as_str().to_string(),
                placement.placement_id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let connection_by_source = fragment
        .connections
        .iter()
        .map(|connection| {
            (
                connection.source_placement_id.clone(),
                connection.connection_id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let plan_id = fragment.plan_id.clone();
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 256);
    runtime.handle(HostCommand::Prepare(fragment));
    let presented = drive_success(&mut runtime, plan_id.clone());
    assert_eq!(presented.len(), 8);
    let observations = inspect(&mut runtime);
    let pulse_a = placement_by_operation["pulse-a"].clone();
    let pulse_b = placement_by_operation["pulse-b"].clone();
    let show_a = placement_by_operation["show-a"].clone();
    let show_b = placement_by_operation["show-b"].clone();
    let conn_a = connection_by_source[&pulse_a].clone();
    let conn_b = connection_by_source[&pulse_b].clone();
    let produced_a = observations
        .iter()
        .filter_map(|item| match &item.kind {
            ObservationKind::ValueProduced { value }
                if item.placement_id.as_ref() == Some(&pulse_a)
                    && item.connection_id.as_ref() == Some(&conn_a) =>
            {
                Some(decode_signal(value).expect("signal payload must decode"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let produced_b = observations
        .iter()
        .filter_map(|item| match &item.kind {
            ObservationKind::ValueProduced { value }
                if item.placement_id.as_ref() == Some(&pulse_b)
                    && item.connection_id.as_ref() == Some(&conn_b) =>
            {
                Some(decode_signal(value).expect("signal payload must decode"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let shown_a = observations
        .iter()
        .filter_map(|item| match &item.kind {
            ObservationKind::ValuePresented { value }
                if item.placement_id.as_ref() == Some(&show_a) =>
            {
                Some(decode_signal(value).expect("signal payload must decode"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let shown_b = observations
        .iter()
        .filter_map(|item| match &item.kind {
            ObservationKind::ValuePresented { value }
                if item.placement_id.as_ref() == Some(&show_b) =>
            {
                Some(decode_signal(value).expect("signal payload must decode"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        produced_a
            .iter()
            .map(|value| value.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        produced_b
            .iter()
            .map(|value| value.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert_eq!(
        shown_a.iter().map(|value| value.level).collect::<Vec<_>>(),
        vec![false, true, false]
    );
    assert_eq!(
        shown_b.iter().map(|value| value.level).collect::<Vec<_>>(),
        vec![true, false, true, false, true]
    );
    assert!(observations.iter().any(|item| matches!(
        item.kind,
        ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        }
    ) && item.plan_id.as_ref() == Some(&plan_id)));
}

#[test]
fn cancellation_before_activation_is_terminal() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
    runtime.handle(HostCommand::Prepare(fragment.clone()));
    let output = runtime.handle(HostCommand::Cancel(fragment.plan_id.clone()));
    assert!(output.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Cancelled {
                reason: CancellationReason::OperatorRequested
            },
            ..
        }
    )));
}

#[test]
fn late_presentation_completion_after_cancel_is_rejected() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 1\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
    let show = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == SHOW_KIND)
        .expect("show placement exists")
        .placement_id
        .clone();
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
    runtime.handle(HostCommand::Prepare(fragment.clone()));
    let output = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
    let (active_play_id, presentation_id, value) = output
        .effects
        .into_iter()
        .find_map(|effect| match effect {
            PlatformEffect::PresentValue {
                active_play_id,
                presentation_id,
                value,
                ..
            } => Some((active_play_id, presentation_id, value)),
            _ => None,
        })
        .expect("present effect must exist");
    runtime.handle(HostCommand::Cancel(fragment.plan_id.clone()));
    let late = runtime.handle(HostCommand::CompletePresentation {
        plan_id: fragment.plan_id,
        active_play_id,
        presentation_id,
        placement_id: show,
        value,
        success: true,
        message: None,
    });
    assert!(late.events.iter().any(|event| matches!(
        event,
        HostEvent::CommandRejected {
            reason: FailureReason::LatePlatformCompletion,
            ..
        }
    )));
}

#[test]
fn repeated_release_is_rejected() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 1\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
    let plan_id = fragment.plan_id.clone();
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
    runtime.handle(HostCommand::Prepare(fragment));
    let _ = drive_success(&mut runtime, plan_id.clone());
    let first = runtime.handle(HostCommand::Release(plan_id.clone()));
    assert!(matches!(
        first.events.first(),
        Some(HostEvent::Released { .. })
    ));
    let second = runtime.handle(HostCommand::Release(plan_id));
    assert!(second.events.iter().any(|event| matches!(
        event,
        HostEvent::CommandRejected {
            reason: FailureReason::InvalidLifecycleCommand,
            ..
        }
    )));
}

#[test]
fn observation_overflow_records_gap() {
    let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 6\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
    let plan_id = fragment.plan_id.clone();
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 4);
    runtime.handle(HostCommand::Prepare(fragment));
    let _ = drive_success(&mut runtime, plan_id);
    let observations = inspect(&mut runtime);
    assert!(observations
        .iter()
        .any(|item| matches!(item.kind, ObservationKind::EvidenceGap { .. })));
}

#[test]
fn fanout_failure_before_first_manifestation_disposes_every_branch() {
    let fragment = demo_fragment("form 0\n\nfanout {\n pulse: flow/pulse\n show-a: presentation/show\n show-b: presentation/show\n show-c: presentation/show\n pulse.count = 8\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show-a\n pulse > show-b\n pulse > show-c\n}\n", 4, 64);
    let failed_sink = fragment
        .placements
        .iter()
        .find(|placement| placement.operation_id.as_str() == "show-b")
        .expect("failed sink exists")
        .placement_id
        .clone();
    let failed_connection = fragment
        .connections
        .iter()
        .find(|connection| connection.sink_placement_id == failed_sink)
        .expect("failed connection exists")
        .connection_id
        .clone();
    let plan_id = fragment.plan_id.clone();
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 512);
    runtime.handle(HostCommand::Prepare(fragment));
    let events = drive_with_failure(&mut runtime, plan_id.clone(), &failed_sink, 0);
    let terminal_connections = events
        .iter()
        .filter_map(|event| match event {
            HostEvent::ConnectionTerminated {
                connection_id,
                disposition,
                ..
            } => Some((connection_id, disposition)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal_connections.len(), 3);
    let failed = terminal_connections
        .iter()
        .find(|(connection_id, _)| **connection_id == failed_connection)
        .expect("failed branch has a disposition")
        .1;
    assert!(matches!(
        failed.disposition,
        TerminalDisposition::Failed {
            reason: FailureReason::ManifestationFailed
        }
    ));
    assert_eq!(failed.last_manifested_sequence, None);
    assert!(failed.undeliverable_items > 0);
    assert!(terminal_connections
        .iter()
        .any(|(connection_id, disposition)| {
            **connection_id != failed_connection
                && matches!(disposition.disposition, TerminalDisposition::Completed)
        }));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                HostEvent::PlanTerminated {
                    disposition: TerminalDisposition::Failed { .. },
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(!events
        .iter()
        .any(|event| matches!(event, HostEvent::PlanCompleted { .. })));
    let observations = inspect(&mut runtime);
    assert!(observations.iter().any(|item| {
        item.plan_id.as_ref() == Some(&plan_id)
            && item.connection_id.as_ref() == Some(&failed_connection)
            && matches!(item.kind, ObservationKind::ConnectionTerminal { .. })
    }));
}

#[test]
fn fanout_failure_after_sequence_seven_retains_last_manifestation() {
    let fragment = demo_fragment("form 0\n\nfanout {\n pulse: flow/pulse\n show-a: presentation/show\n show-b: presentation/show\n show-c: presentation/show\n pulse.count = 10\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show-a\n pulse > show-b\n pulse > show-c\n}\n", 4, 64);
    let failed_sink = fragment
        .placements
        .iter()
        .find(|placement| placement.operation_id.as_str() == "show-b")
        .expect("failed sink exists")
        .placement_id
        .clone();
    let failed_connection = fragment
        .connections
        .iter()
        .find(|connection| connection.sink_placement_id == failed_sink)
        .expect("failed connection exists")
        .connection_id
        .clone();
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 768);
    runtime.handle(HostCommand::Prepare(fragment.clone()));
    let events = drive_with_failure(&mut runtime, fragment.plan_id, &failed_sink, 8);
    let disposition = events
        .iter()
        .find_map(|event| match event {
            HostEvent::ConnectionTerminated {
                connection_id,
                disposition,
                ..
            } if connection_id == &failed_connection => Some(disposition),
            _ => None,
        })
        .expect("failed branch terminates");
    assert_eq!(disposition.last_accepted_sequence, Some(8));
    assert_eq!(disposition.last_manifested_sequence, Some(7));
    assert!(matches!(
        disposition.disposition,
        TerminalDisposition::Failed { .. }
    ));
}

#[test]
fn cancellation_while_waiting_rejects_late_timer_and_is_idempotently_rejected() {
    let fragment = demo_fragment("form 0\n\ndemo {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 3\n pulse.period-ms = 10\n pulse.initial = false\n pulse > show\n}\n", 4, 64);
    let pulse = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == PULSE_KIND)
        .expect("pulse exists")
        .placement_id
        .clone();
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 256);
    runtime.handle(HostCommand::Prepare(fragment.clone()));
    let activated = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
    assert!(activated
        .effects
        .iter()
        .any(|effect| matches!(effect, PlatformEffect::Wait { .. })));
    let cancelled = runtime.handle(HostCommand::Cancel(fragment.plan_id.clone()));
    assert!(cancelled.events.iter().any(|event| matches!(
        event,
        HostEvent::PlanTerminated {
            disposition: TerminalDisposition::Cancelled { .. },
            ..
        }
    )));
    let repeated = runtime.handle(HostCommand::Cancel(fragment.plan_id.clone()));
    assert!(repeated.events.iter().any(|event| matches!(
        event,
        HostEvent::CommandRejected {
            reason: FailureReason::InvalidLifecycleCommand,
            ..
        }
    )));
    let late = runtime.handle(HostCommand::CompleteWait {
        plan_id: fragment.plan_id,
        placement_id: pulse,
    });
    assert!(late.events.iter().any(|event| matches!(
        event,
        HostEvent::CommandRejected {
            reason: FailureReason::LatePlatformCompletion,
            ..
        }
    )));
}

#[test]
fn fanout_cancellation_releases_all_queued_items() {
    let fragment = demo_fragment("form 0\n\nfanout {\n pulse: flow/pulse\n show-a: presentation/show\n show-b: presentation/show\n pulse.count = 8\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show-a\n pulse > show-b\n}\n", 4, 64);
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 256);
    runtime.handle(HostCommand::Prepare(fragment.clone()));
    runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
    let cancelled = runtime.handle(HostCommand::Cancel(fragment.plan_id));
    let dispositions = cancelled
        .events
        .iter()
        .filter_map(|event| match event {
            HostEvent::ConnectionTerminated { disposition, .. } => Some(disposition),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(dispositions.len(), 2);
    assert!(dispositions.iter().all(|disposition| matches!(
        disposition.disposition,
        TerminalDisposition::Cancelled {
            reason: CancellationReason::OperatorRequested
        }
    )));
    assert!(dispositions
        .iter()
        .all(|disposition| disposition.undeliverable_items > 0));
}

#[test]
fn fanout_accounts_for_each_branches_byte_capacity_independently() {
    let fragment = demo_fragment("form 0\n\nfanout {\n pulse: flow/pulse\n show-a: presentation/show\n show-b: presentation/show\n show-c: presentation/show\n pulse.count = 3\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show-a\n pulse > show-b\n pulse > show-c\n}\n", 4, 9);
    let connection_ids = fragment
        .connections
        .iter()
        .map(|connection| connection.connection_id.clone())
        .collect::<Vec<_>>();
    let plan_id = fragment.plan_id.clone();
    let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 512);
    runtime.handle(HostCommand::Prepare(fragment));
    let presented = drive_success(&mut runtime, plan_id);
    assert_eq!(presented.len(), 9);
    let observations = inspect(&mut runtime);
    for connection_id in connection_ids {
        let sequences = observations
            .iter()
            .filter_map(|item| match &item.kind {
                ObservationKind::ValueProduced { value }
                    if item.connection_id.as_ref() == Some(&connection_id) =>
                {
                    Some(decode_signal(value).expect("signal decodes").sequence)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(sequences, vec![0, 1, 2]);
    }
}

#[test]
fn release_after_failure_and_cancellation_preserves_terminal_evidence() {
    for fail in [false, true] {
        let fragment = demo_fragment("form 0\n\ndemo {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 2\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show\n}\n", 4, 64);
        let plan_id = fragment.plan_id.clone();
        let show = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == SHOW_KIND)
            .expect("show exists")
            .placement_id
            .clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 256);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        if fail {
            let _ = drive_with_failure(&mut runtime, plan_id.clone(), &show, 0);
        } else {
            runtime.handle(HostCommand::Cancel(plan_id.clone()));
        }
        assert!(matches!(
            runtime
                .handle(HostCommand::Release(plan_id.clone()))
                .events
                .first(),
            Some(HostEvent::Released { .. })
        ));
        let observations = inspect(&mut runtime);
        assert!(observations.iter().any(|item| {
            item.plan_id.as_ref() == Some(&plan_id)
                && matches!(item.kind, ObservationKind::PlanTerminal { .. })
        }));
        assert!(observations.iter().any(|item| {
            item.plan_id.as_ref() == Some(&plan_id)
                && matches!(item.kind, ObservationKind::Released)
        }));
        let after_release = runtime.handle(HostCommand::Activate(plan_id.clone()));
        assert!(after_release.events.iter().any(|event| matches!(
            event,
            HostEvent::CommandRejected {
                reason: FailureReason::InvalidLifecycleCommand,
                ..
            }
        )));
        for rejected in [
            runtime.handle(HostCommand::Cancel(plan_id.clone())),
            runtime.handle(HostCommand::Prepare(fragment.clone())),
        ] {
            assert!(rejected.events.iter().any(|event| matches!(
                event,
                HostEvent::CommandRejected {
                    reason: FailureReason::InvalidLifecycleCommand,
                    ..
                }
            )));
        }
        let late = runtime.handle(HostCommand::CompletePresentation {
            plan_id,
            active_play_id: conduit_core::ActivePlayId::from("released-play"),
            presentation_id: conduit_core::PresentationId::from("released-presentation"),
            placement_id: show,
            value: encode_signal(&Signal {
                sequence: 0,
                level: false,
            }),
            success: true,
            message: None,
        });
        assert!(late.events.iter().any(|event| matches!(
            event,
            HostEvent::CommandRejected {
                reason: FailureReason::LatePlatformCompletion,
                ..
            }
        )));
    }
}
