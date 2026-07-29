//! Hosted projection of bounded executor observations into ExecutionEvent v1.

use std::fmt;
use std::io::{self, Write};

use conduit_core::{
    EventCorrelation, EventTimeKind, EvidencePolicy, ExecutionEventKind, ExecutionPlan,
    FlowEventKind, Id, InstancePath, MAX_EVENT_DERIVATIONS, RuntimeEvidenceBudget,
    RuntimeEvidenceMode, RuntimeEvidenceReason, TelemetryAdmission, TerminalClass,
    validate_execution_event, validate_runtime_evidence_policy,
};
use serde::Serialize;

use crate::{
    OwnedEventCorrelation, OwnedEventPayload, OwnedEventRelations, OwnedEventTerminality,
    OwnedEventTime, OwnedExecutionEvent, OwnedPayloadShape, OwnedTypeRef, SchedulerEvent,
    SchedulerEventKind, SchedulerSubject,
};

const RUNTIME_OBSERVATION_TYPE_HASH: &str =
    "sha256:2323232323232323232323232323232323232323232323232323232323232323";
const RUNTIME_BINDING_TYPE_HASH: &str =
    "sha256:2424242424242424242424242424242424242424242424242424242424242424";
const RUNTIME_AUTHORITY_BINDING_TYPE_HASH: &str =
    "sha256:2525252525252525252525252525252525252525252525252525252525252525";
const RUNTIME_OBSERVATION_PAYLOAD_BYTES: usize = 52;
const EVENT_ID_BYTES: usize = 23;

#[derive(Clone, Copy, Debug)]
pub struct RuntimeEvidenceContext<'a> {
    pub run: Id<'a>,
    pub recorder: Id<'a>,
    pub observer: Id<'a>,
    pub monotonic_basis: Id<'a>,
    pub correlation: EventCorrelation<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEvidenceError {
    Policy(RuntimeEvidenceReason),
    InvalidObservation,
    InvalidPlanSubject,
    InvalidEvent,
    AllocationFailed,
    RecordTooLarge,
    HandleIndexFull,
}

impl RuntimeEvidenceError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Policy(reason) => reason.code(),
            Self::InvalidObservation => "CND-RTE-009",
            Self::InvalidPlanSubject => "CND-RTE-010",
            Self::InvalidEvent => "CND-RTE-011",
            Self::AllocationFailed | Self::RecordTooLarge | Self::HandleIndexFull => "CND-RTE-012",
        }
    }
}

impl fmt::Display for RuntimeEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RuntimeEvidenceError {}

impl From<RuntimeEvidenceReason> for RuntimeEvidenceError {
    fn from(reason: RuntimeEvidenceReason) -> Self {
        Self::Policy(reason)
    }
}

#[derive(Clone, Copy)]
struct EventDescription {
    kind: ExecutionEventKind,
    detail: &'static str,
    required: bool,
    terminal: Option<TerminalClass>,
}

/// Project one executor's fixed observation log. No channel bytes or log prose
/// are accepted by this API.
pub fn record_scheduler_evidence(
    plan: &ExecutionPlan<'_>,
    context: RuntimeEvidenceContext<'_>,
    observations: &[SchedulerEvent],
) -> Result<Vec<OwnedExecutionEvent>, RuntimeEvidenceError> {
    let Some(policy) = plan.runtime_evidence else {
        return Ok(Vec::new());
    };
    let stream = policy.stream.and_then(|stream_id| {
        plan.event_streams
            .iter()
            .find(|stream| stream.contract.id == stream_id)
            .map(|stream| (stream.contract, stream.provider_capabilities))
    });
    validate_runtime_evidence_policy(policy, stream)?;
    let mut budget = RuntimeEvidenceBudget::new(policy);
    if policy.mode == RuntimeEvidenceMode::Disabled {
        budget.finish()?;
        return Ok(Vec::new());
    }

    let capacity = usize::try_from(policy.maximum_events)
        .map_err(|_| RuntimeEvidenceError::AllocationFailed)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| RuntimeEvidenceError::AllocationFailed)?;
    let mut handles: Vec<(u64, String)> = Vec::new();
    handles
        .try_reserve_exact(capacity)
        .map_err(|_| RuntimeEvidenceError::AllocationFailed)?;
    let mut last_event_id: Option<String> = None;

    for observation in observations {
        let description = describe(observation.kind);
        if description.terminal.is_some() {
            if let Some(skipped) = budget.flush_sampling_summary()? {
                push_summary(
                    plan,
                    context,
                    &mut output,
                    &mut last_event_id,
                    policy.gap_summary_bytes,
                    skipped,
                    observation.tick,
                )?;
            }
        }

        let derived_from = derivations(observation, &handles);
        let mut event = build_event(
            plan,
            context,
            output.len(),
            observation,
            description,
            last_event_id.as_deref(),
            &derived_from,
        )?;
        let accounted_bytes = serialized_bytes(&event)?;

        if description.required {
            budget.record_required(accounted_bytes, description.terminal.is_some())?;
        } else {
            match budget.admit_telemetry(accounted_bytes)? {
                TelemetryAdmission::Disabled | TelemetryAdmission::Sampled => continue,
                TelemetryAdmission::Record => {}
                TelemetryAdmission::RecordAfterSummary { skipped } => {
                    push_summary(
                        plan,
                        context,
                        &mut output,
                        &mut last_event_id,
                        policy.gap_summary_bytes,
                        skipped,
                        observation.tick,
                    )?;
                    event.sequence = u64::try_from(output.len())
                        .map_err(|_| RuntimeEvidenceError::InvalidEvent)?;
                    event.observer_sequence = event.sequence;
                    event.event_id = event_id(event.sequence);
                    event.relations.caused_by = last_event_id.clone();
                    finish_identity(&mut event)?;
                }
            }
        }

        let event_id = event.event_id.clone();
        if let Some(handle) = observation.value_handle {
            update_handle(&mut handles, handle, &event_id, capacity)?;
        }
        last_event_id = Some(event_id);
        output.push(event);

        if matches!(observation.kind, SchedulerEventKind::NodePrepared) {
            let SchedulerSubject::Node(node_index) = observation.subject else {
                return Err(RuntimeEvidenceError::InvalidPlanSubject);
            };
            let node = plan
                .nodes
                .get(usize::from(node_index))
                .ok_or(RuntimeEvidenceError::InvalidPlanSubject)?;
            for resource in node.required_resources {
                push_plan_fact(
                    plan,
                    context,
                    &mut output,
                    &mut last_event_id,
                    &mut budget,
                    node.instance,
                    "runtime/resource-bound",
                    ExecutionEventKind::Resource,
                    observation.tick,
                    binding_payload(resource.as_str()),
                )?;
            }
            for (authority_index, _) in plan
                .authorities
                .iter()
                .enumerate()
                .filter(|(_, authority)| authority.node == node.instance)
            {
                let detail = format!("runtime/authority-bound.a{authority_index}");
                push_plan_fact(
                    plan,
                    context,
                    &mut output,
                    &mut last_event_id,
                    &mut budget,
                    node.instance,
                    &detail,
                    ExecutionEventKind::Authority,
                    observation.tick,
                    authority_binding_payload(),
                )?;
            }
        }
    }

    if let Some(skipped) = budget.flush_sampling_summary()? {
        let tick = observations.last().map_or(0, |event| event.tick);
        push_summary(
            plan,
            context,
            &mut output,
            &mut last_event_id,
            policy.gap_summary_bytes,
            skipped,
            tick,
        )?;
    }
    budget.finish()?;
    validate_complete_stream(&output)?;
    Ok(output)
}

fn describe(kind: SchedulerEventKind) -> EventDescription {
    let (kind, detail, required, terminal) = match kind {
        SchedulerEventKind::AllocationPrepared => (
            ExecutionEventKind::Lifecycle,
            "runtime/allocation-prepared",
            true,
            None,
        ),
        SchedulerEventKind::NodePrepared => (
            ExecutionEventKind::Lifecycle,
            "runtime/node-prepared",
            true,
            None,
        ),
        SchedulerEventKind::RunStarted => (
            ExecutionEventKind::Lifecycle,
            "runtime/run-started",
            true,
            None,
        ),
        SchedulerEventKind::Decision { .. } => (
            ExecutionEventKind::Progress,
            "runtime/scheduler-decision",
            false,
            None,
        ),
        SchedulerEventKind::NodeOutcome { .. } => (
            ExecutionEventKind::Progress,
            "runtime/node-outcome",
            false,
            None,
        ),
        SchedulerEventKind::NodeWoken { .. } => (
            ExecutionEventKind::Progress,
            "runtime/node-woken",
            false,
            None,
        ),
        SchedulerEventKind::ValueAccepted => (
            ExecutionEventKind::ValueAccepted,
            "runtime/value-accepted",
            true,
            None,
        ),
        SchedulerEventKind::ValueConsumed => (
            ExecutionEventKind::Derivation,
            "runtime/value-consumed",
            true,
            None,
        ),
        SchedulerEventKind::DerivationCommitted => (
            ExecutionEventKind::Derivation,
            "runtime/derivation-committed",
            true,
            None,
        ),
        SchedulerEventKind::CancellationRequested { .. } => (
            ExecutionEventKind::Cancellation,
            "runtime/cancellation-requested",
            true,
            None,
        ),
        SchedulerEventKind::Terminal(class) => (
            ExecutionEventKind::Terminal,
            terminal_detail(class),
            true,
            Some(class),
        ),
        SchedulerEventKind::Cord(flow) => return describe_flow(flow),
    };
    EventDescription {
        kind,
        detail,
        required,
        terminal,
    }
}

fn describe_flow(kind: FlowEventKind) -> EventDescription {
    let (event_kind, detail, required) = match kind {
        FlowEventKind::PressureEntered => (
            ExecutionEventKind::Pressure,
            "runtime/pressure-entered",
            true,
        ),
        FlowEventKind::PressureCleared => (
            ExecutionEventKind::Pressure,
            "runtime/pressure-cleared",
            true,
        ),
        FlowEventKind::ValueRejected => (
            ExecutionEventKind::ValueRejected,
            "runtime/value-rejected",
            true,
        ),
        FlowEventKind::ValueCoalesced { .. } => (
            ExecutionEventKind::ValueCoalesced,
            "runtime/value-coalesced",
            true,
        ),
        FlowEventKind::ValueSampledOut => (
            ExecutionEventKind::ValueDropped,
            "runtime/value-sampled-out",
            true,
        ),
        FlowEventKind::ValueDroppedDisposable => (
            ExecutionEventKind::ValueDropped,
            "runtime/value-dropped-disposable",
            true,
        ),
        FlowEventKind::ConsumerReady => (
            ExecutionEventKind::CordOccupancy,
            "runtime/consumer-ready",
            false,
        ),
        FlowEventKind::ProducerReady => (
            ExecutionEventKind::CordOccupancy,
            "runtime/producer-ready",
            false,
        ),
        FlowEventKind::Disconnected => (
            ExecutionEventKind::Lifecycle,
            "runtime/cord-disconnected",
            true,
        ),
        FlowEventKind::Failed => (ExecutionEventKind::Lifecycle, "runtime/cord-failed", true),
        FlowEventKind::Cancelled { .. } => (
            ExecutionEventKind::Cancellation,
            "runtime/cord-cancelled",
            true,
        ),
        FlowEventKind::DrainStarted { .. } => {
            (ExecutionEventKind::Lifecycle, "runtime/cord-draining", true)
        }
        FlowEventKind::ValuesDiscardedOnAbort { .. } => (
            ExecutionEventKind::ValueDropped,
            "runtime/accepted-values-aborted",
            true,
        ),
        FlowEventKind::Completed => (
            ExecutionEventKind::Lifecycle,
            "runtime/cord-completed",
            true,
        ),
    };
    EventDescription {
        kind: event_kind,
        detail,
        required,
        terminal: None,
    }
}

fn terminal_detail(class: TerminalClass) -> &'static str {
    match class {
        TerminalClass::Succeeded => "runtime/terminal-succeeded",
        TerminalClass::Cancelled => "runtime/terminal-cancelled",
        TerminalClass::Disconnected => "runtime/terminal-disconnected",
        TerminalClass::Failed => "runtime/terminal-failed",
    }
}

fn terminal_cause(class: TerminalClass) -> String {
    match class {
        TerminalClass::Succeeded => "cause/succeeded",
        TerminalClass::Cancelled => "cause/cancelled",
        TerminalClass::Disconnected => "cause/disconnected",
        TerminalClass::Failed => "cause/failed",
    }
    .to_owned()
}

#[allow(clippy::too_many_arguments)]
fn build_event(
    plan: &ExecutionPlan<'_>,
    context: RuntimeEvidenceContext<'_>,
    sequence: usize,
    observation: &SchedulerEvent,
    description: EventDescription,
    caused_by: Option<&str>,
    derived_from: &[String],
) -> Result<OwnedExecutionEvent, RuntimeEvidenceError> {
    let sequence = u64::try_from(sequence).map_err(|_| RuntimeEvidenceError::InvalidEvent)?;
    let subject = subject(plan, observation.subject)?;
    let logical_template = logical_template(plan, &subject);
    let tick =
        i64::try_from(observation.tick).map_err(|_| RuntimeEvidenceError::InvalidObservation)?;
    let terminality = description
        .terminal
        .map_or(OwnedEventTerminality::NonTerminal, |class| {
            OwnedEventTerminality::Terminal {
                class: terminal_class(class).to_owned(),
                cause: terminal_cause(class),
            }
        });
    let mut event = OwnedExecutionEvent {
        schema_version: 1,
        identity: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_owned(),
        event_id: event_id(sequence),
        run_id: context.run.as_str().to_owned(),
        plan_identity: plan.identity.to_string(),
        sequence,
        recorder: context.recorder.as_str().to_owned(),
        observer: context.observer.as_str().to_owned(),
        observer_sequence: sequence,
        logical_template,
        subject,
        kind: description.kind.as_str().to_owned(),
        detail: description.detail.to_owned(),
        observed_time: OwnedEventTime {
            kind: EventTimeKind::Monotonic.as_str().to_owned(),
            basis: context.monotonic_basis.as_str().to_owned(),
            tick,
        },
        domain_time: None,
        correlation: owned_correlation(context.correlation),
        relations: OwnedEventRelations {
            caused_by: caused_by.map(str::to_owned),
            derived_from: derived_from.to_vec(),
            supersedes: None,
            retracts: None,
        },
        terminality,
        payload: runtime_payload(observation),
    };
    finish_identity(&mut event)?;
    Ok(event)
}

fn finish_identity(event: &mut OwnedExecutionEvent) -> Result<(), RuntimeEvidenceError> {
    event.identity = computed_identity(event)?;
    validate_owned_event(event)
}

fn computed_identity(event: &OwnedExecutionEvent) -> Result<String, RuntimeEvidenceError> {
    let mut scratch = [Id(""); MAX_EVENT_DERIVATIONS];
    let borrowed = event
        .as_event(&mut scratch)
        .map_err(|_| RuntimeEvidenceError::InvalidEvent)?;
    let identity = borrowed
        .semantic_hash()
        .map_err(|_| RuntimeEvidenceError::InvalidEvent)?;
    Ok(identity.to_string())
}

fn validate_owned_event(event: &OwnedExecutionEvent) -> Result<(), RuntimeEvidenceError> {
    let mut scratch = [Id(""); MAX_EVENT_DERIVATIONS];
    let borrowed = event
        .as_event(&mut scratch)
        .map_err(|_| RuntimeEvidenceError::InvalidEvent)?;
    validate_execution_event(
        &borrowed,
        EvidencePolicy {
            max_inline_payload_bytes: u32::MAX,
            reveal_redacted_byte_length: false,
            reveal_redacted_item_count: false,
        },
    )
    .map_err(|_| RuntimeEvidenceError::InvalidEvent)
}

fn runtime_payload(observation: &SchedulerEvent) -> OwnedEventPayload {
    let mut bytes = Vec::with_capacity(RUNTIME_OBSERVATION_PAYLOAD_BYTES);
    bytes.push(1);
    let mut flags = 0_u8;
    flags |= u8::from(observation.value_handle.is_some());
    flags |= u8::from(observation.related_value_handle.is_some()) << 1;
    bytes.push(flags);
    bytes.extend_from_slice(&observation.occupancy_items.to_be_bytes());
    bytes.extend_from_slice(&observation.occupancy_bytes.to_be_bytes());
    bytes.extend_from_slice(&observation.value_handle.unwrap_or_default().to_be_bytes());
    bytes.extend_from_slice(
        &observation
            .related_value_handle
            .unwrap_or_default()
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&observation.scheduling_latency_ticks.to_be_bytes());
    bytes.extend_from_slice(&observation.processing_latency_ticks.to_be_bytes());
    bytes.extend_from_slice(&observation.sequence.to_be_bytes());
    debug_assert_eq!(bytes.len(), RUNTIME_OBSERVATION_PAYLOAD_BYTES);
    OwnedEventPayload::InlinePublic {
        value_type: OwnedTypeRef {
            id: "conduit/runtime-observation".to_owned(),
            schema_version: 1,
            semantic_hash: RUNTIME_OBSERVATION_TYPE_HASH.to_owned(),
        },
        bytes,
    }
}

fn summary_payload(skipped: u64) -> OwnedEventPayload {
    let observation = SchedulerEvent {
        sequence: skipped,
        tick: 0,
        subject: SchedulerSubject::Run,
        kind: SchedulerEventKind::RunStarted,
        occupancy_items: 0,
        occupancy_bytes: 0,
        value_handle: Some(skipped),
        related_value_handle: None,
        scheduling_latency_ticks: 0,
        processing_latency_ticks: 0,
    };
    runtime_payload(&observation)
}

#[allow(clippy::too_many_arguments)]
fn push_summary(
    plan: &ExecutionPlan<'_>,
    context: RuntimeEvidenceContext<'_>,
    output: &mut Vec<OwnedExecutionEvent>,
    last_event_id: &mut Option<String>,
    maximum_bytes: u64,
    skipped: u64,
    tick: u64,
) -> Result<(), RuntimeEvidenceError> {
    let sequence = u64::try_from(output.len()).map_err(|_| RuntimeEvidenceError::InvalidEvent)?;
    let tick = i64::try_from(tick).map_err(|_| RuntimeEvidenceError::InvalidObservation)?;
    let mut event = OwnedExecutionEvent {
        schema_version: 1,
        identity: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_owned(),
        event_id: event_id(sequence),
        run_id: context.run.as_str().to_owned(),
        plan_identity: plan.identity.to_string(),
        sequence,
        recorder: context.recorder.as_str().to_owned(),
        observer: context.observer.as_str().to_owned(),
        observer_sequence: sequence,
        logical_template: None,
        subject: "run".to_owned(),
        kind: ExecutionEventKind::Progress.as_str().to_owned(),
        detail: "runtime/telemetry-summary".to_owned(),
        observed_time: OwnedEventTime {
            kind: EventTimeKind::Monotonic.as_str().to_owned(),
            basis: context.monotonic_basis.as_str().to_owned(),
            tick,
        },
        domain_time: None,
        correlation: owned_correlation(context.correlation),
        relations: OwnedEventRelations {
            caused_by: last_event_id.clone(),
            derived_from: vec![],
            supersedes: None,
            retracts: None,
        },
        terminality: OwnedEventTerminality::NonTerminal,
        payload: summary_payload(skipped),
    };
    finish_identity(&mut event)?;
    if serialized_bytes(&event)? > maximum_bytes {
        return Err(RuntimeEvidenceError::RecordTooLarge);
    }
    *last_event_id = Some(event.event_id.clone());
    output.push(event);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_plan_fact(
    plan: &ExecutionPlan<'_>,
    context: RuntimeEvidenceContext<'_>,
    output: &mut Vec<OwnedExecutionEvent>,
    last_event_id: &mut Option<String>,
    budget: &mut RuntimeEvidenceBudget<'_>,
    subject: InstancePath<'_>,
    detail: &str,
    kind: ExecutionEventKind,
    tick: u64,
    payload: OwnedEventPayload,
) -> Result<(), RuntimeEvidenceError> {
    let observation = SchedulerEvent {
        sequence: 0,
        tick,
        subject: SchedulerSubject::Run,
        kind: SchedulerEventKind::RunStarted,
        occupancy_items: 0,
        occupancy_bytes: 0,
        value_handle: None,
        related_value_handle: None,
        scheduling_latency_ticks: 0,
        processing_latency_ticks: 0,
    };
    let description = EventDescription {
        kind,
        detail: "runtime/plan-fact",
        required: true,
        terminal: None,
    };
    let mut event = build_event(
        plan,
        context,
        output.len(),
        &observation,
        description,
        last_event_id.as_deref(),
        &[],
    )?;
    event.subject = subject.as_str().to_owned();
    event.logical_template = logical_template(plan, &event.subject);
    event.detail = detail.to_owned();
    event.payload = payload;
    finish_identity(&mut event)?;
    budget.record_required(serialized_bytes(&event)?, false)?;
    *last_event_id = Some(event.event_id.clone());
    output.push(event);
    Ok(())
}

fn binding_payload(binding: &str) -> OwnedEventPayload {
    OwnedEventPayload::InlinePublic {
        value_type: OwnedTypeRef {
            id: "conduit/runtime-binding".to_owned(),
            schema_version: 1,
            semantic_hash: RUNTIME_BINDING_TYPE_HASH.to_owned(),
        },
        bytes: binding.as_bytes().to_vec(),
    }
}

fn authority_binding_payload() -> OwnedEventPayload {
    OwnedEventPayload::Redacted {
        value_type: OwnedTypeRef {
            id: "conduit/runtime-authority-binding".to_owned(),
            schema_version: 1,
            semantic_hash: RUNTIME_AUTHORITY_BINDING_TYPE_HASH.to_owned(),
        },
        sensitivity: "secret".to_owned(),
        shape: OwnedPayloadShape {
            byte_length: None,
            item_count: None,
        },
        reason: "authority/redacted".to_owned(),
    }
}

fn serialized_bytes(value: &impl Serialize) -> Result<u64, RuntimeEvidenceError> {
    let mut counter = CountingWriter { bytes: 0 };
    serde_json::to_writer(&mut counter, value).map_err(|_| RuntimeEvidenceError::RecordTooLarge)?;
    counter
        .bytes
        .checked_add(1)
        .ok_or(RuntimeEvidenceError::RecordTooLarge)
}

struct CountingWriter {
    bytes: u64,
}

impl Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("runtime-evidence-size-overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn event_id(sequence: u64) -> String {
    let value = format!("event/e{sequence:016x}");
    debug_assert_eq!(value.len(), EVENT_ID_BYTES);
    value
}

fn subject(
    plan: &ExecutionPlan<'_>,
    subject: SchedulerSubject,
) -> Result<String, RuntimeEvidenceError> {
    match subject {
        SchedulerSubject::Run => Ok("run".to_owned()),
        SchedulerSubject::Node(index) => plan
            .nodes
            .get(usize::from(index))
            .map(|node| node.instance.as_str().to_owned())
            .ok_or(RuntimeEvidenceError::InvalidPlanSubject),
        SchedulerSubject::Cord(index) => {
            let cord = plan
                .cords
                .get(usize::from(index))
                .ok_or(RuntimeEvidenceError::InvalidPlanSubject)?;
            let value = format!("{}/cord.{}", cord.from.node.as_str(), cord.id.as_str());
            InstancePath::new(&value).map_err(|_| RuntimeEvidenceError::InvalidPlanSubject)?;
            Ok(value)
        }
    }
}

fn logical_template(plan: &ExecutionPlan<'_>, subject: &str) -> Option<String> {
    plan.composites
        .iter()
        .filter(|composite| {
            subject == composite.instance.as_str()
                || subject
                    .strip_prefix(composite.instance.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
        .max_by_key(|composite| composite.instance.as_str().len())
        .map(|composite| composite.instance.as_str().to_owned())
}

fn derivations(observation: &SchedulerEvent, handles: &[(u64, String)]) -> Vec<String> {
    if !matches!(
        observation.kind,
        SchedulerEventKind::ValueConsumed | SchedulerEventKind::DerivationCommitted
    ) {
        return Vec::new();
    }
    let mut values = Vec::with_capacity(2);
    let handles_to_find = if observation.kind == SchedulerEventKind::ValueConsumed {
        [observation.value_handle, None]
    } else {
        [observation.related_value_handle, observation.value_handle]
    };
    for handle in handles_to_find.into_iter().flatten() {
        if let Some((_, event)) = handles.iter().find(|(candidate, _)| *candidate == handle) {
            if !values.contains(event) {
                values.push(event.clone());
            }
        }
    }
    values
}

fn update_handle(
    handles: &mut Vec<(u64, String)>,
    handle: u64,
    event: &str,
    maximum: usize,
) -> Result<(), RuntimeEvidenceError> {
    if let Some((_, prior)) = handles
        .iter_mut()
        .find(|(candidate, _)| *candidate == handle)
    {
        *prior = event.to_owned();
        return Ok(());
    }
    if handles.len() == maximum {
        return Err(RuntimeEvidenceError::HandleIndexFull);
    }
    handles.push((handle, event.to_owned()));
    Ok(())
}

fn owned_correlation(value: EventCorrelation<'_>) -> OwnedEventCorrelation {
    OwnedEventCorrelation {
        request: value.request.map(|id| id.as_str().to_owned()),
        exchange: value.exchange.map(|id| id.as_str().to_owned()),
        session: value.session.map(|id| id.as_str().to_owned()),
        epoch: value.epoch,
        work_unit: value.work_unit.map(|id| id.as_str().to_owned()),
        attempt: value.attempt.map(|id| id.as_str().to_owned()),
        correlation: value.correlation.map(|id| id.as_str().to_owned()),
        idempotency: value.idempotency.map(|id| id.as_str().to_owned()),
        checkpoint: value.checkpoint.map(|id| id.as_str().to_owned()),
        transport: value.transport.map(|id| id.as_str().to_owned()),
    }
}

fn terminal_class(value: TerminalClass) -> &'static str {
    match value {
        TerminalClass::Succeeded => "succeeded",
        TerminalClass::Cancelled => "cancelled",
        TerminalClass::Disconnected => "disconnected",
        TerminalClass::Failed => "failed",
    }
}

fn validate_complete_stream(events: &[OwnedExecutionEvent]) -> Result<(), RuntimeEvidenceError> {
    let Some(first) = events.first() else {
        return Err(RuntimeEvidenceError::Policy(
            RuntimeEvidenceReason::MissingTerminal,
        ));
    };
    let mut terminal_count = 0;
    for (index, event) in events.iter().enumerate() {
        validate_owned_event(event)?;
        if event.sequence != index as u64
            || event.observer_sequence != index as u64
            || event.run_id != first.run_id
            || event.plan_identity != first.plan_identity
            || event.recorder != first.recorder
            || events[..index]
                .iter()
                .any(|prior| prior.event_id == event.event_id || prior.identity == event.identity)
            || event
                .relations
                .caused_by
                .iter()
                .chain(&event.relations.derived_from)
                .any(|reference| {
                    !events[..index]
                        .iter()
                        .any(|prior| prior.event_id == *reference)
                })
        {
            return Err(RuntimeEvidenceError::InvalidEvent);
        }
        if matches!(event.terminality, OwnedEventTerminality::Terminal { .. }) {
            terminal_count += 1;
            if index + 1 != events.len() {
                return Err(RuntimeEvidenceError::InvalidEvent);
            }
        }
    }
    if terminal_count != 1 {
        return Err(RuntimeEvidenceError::Policy(
            RuntimeEvidenceReason::MissingTerminal,
        ));
    }
    Ok(())
}
