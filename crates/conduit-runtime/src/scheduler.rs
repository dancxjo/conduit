//! Bounded deterministic streaming executor.
//!
//! The executor consumes exact plan cords and the host-neutral implementation
//! machine. It owns no unbounded channel, task queue, waiter list, or evidence
//! buffer. A hosted async runtime can wake this executor, but is not part of
//! its semantics.

use std::fmt;
use std::mem::size_of;

use conduit_core::{
    DuplicationRule, ExecutionPlan, FanOutMode, FlowEventKind, FlowPolicy, FlowQueueState, Id,
    ImplementationError, ImplementationMachine, InstancePhase, LifecycleUsage, OfferDisposition,
    PlanResourceBudget, PrepareOutcome, Pressure, ReadyQueueDiscipline, SchedulerDecisionReason,
    SchedulerPolicy, SemanticHash, Sensitivity, StepObservation, StepOutcome, StepOutcomeKind,
    StepUsage, StopPolicy, TerminalClass, ValueEnvelopeReason, WakeInterestKind, prepare_all,
    start_all,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeTimestamp {
    /// Index into the cord policy's exact `clock_domains` set.
    pub domain_index: u8,
    pub tick: i64,
    pub uncertainty_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeValueEnvelope {
    pub representation: SemanticHash,
    pub envelope_bytes: u32,
    pub fragment_count: u16,
    pub fragment_bytes: u32,
    pub identity: Option<SemanticHash>,
    pub correlation: Option<SemanticHash>,
    pub causation: Option<SemanticHash>,
    pub provenance: Option<SemanticHash>,
    pub timestamp_count: u8,
    pub timestamps: [RuntimeTimestamp; conduit_core::MAX_VALUE_CLOCK_DOMAINS],
    pub sensitivity: Sensitivity,
}

/// Runtime-owned pressure facts. The semantic coalescing relation is checked
/// while sealing the exact plan; the scheduler needs only the exact bounded
/// operational behavior after a session starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimePressure {
    Block,
    Reject,
    Coalesce,
    Sample { every: u32, offset: u32 },
    DropDisposable,
    Disconnect,
    Fail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeFlowPolicy {
    capacity_items: u16,
    maximum_value_bytes: u32,
    maximum_queued_bytes: u64,
    low_watermark_items: u16,
    high_watermark_items: u16,
    pressure: RuntimePressure,
}

impl RuntimeFlowPolicy {
    fn from_plan(policy: FlowPolicy<'_>) -> Self {
        let pressure = match policy.pressure {
            Pressure::Block(_) => RuntimePressure::Block,
            Pressure::Reject => RuntimePressure::Reject,
            Pressure::Coalesce { .. } => RuntimePressure::Coalesce,
            Pressure::Sample(schedule) => RuntimePressure::Sample {
                every: schedule.every(),
                offset: schedule.offset(),
            },
            Pressure::DropDisposable => RuntimePressure::DropDisposable,
            Pressure::Disconnect => RuntimePressure::Disconnect,
            Pressure::Fail => RuntimePressure::Fail,
        };
        Self {
            capacity_items: policy.capacity.items(),
            maximum_value_bytes: policy.capacity.max_value_bytes(),
            maximum_queued_bytes: policy.capacity.max_queued_bytes(),
            low_watermark_items: policy.watermarks.low_items(),
            high_watermark_items: policy.watermarks.high_items(),
            pressure,
        }
    }

    pub(crate) const fn pressure_name(self) -> &'static str {
        match self.pressure {
            RuntimePressure::Block => "block",
            RuntimePressure::Reject => "reject",
            RuntimePressure::Coalesce => "coalesce",
            RuntimePressure::Sample { .. } => "sample",
            RuntimePressure::DropDisposable => "drop-disposable",
            RuntimePressure::Disconnect => "disconnect",
            RuntimePressure::Fail => "fail",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeEnvelopePolicy {
    representation: SemanticHash,
    maximum_payload_bytes: u32,
    maximum_envelope_bytes: u32,
    maximum_fragments: u16,
    maximum_fragment_bytes: u32,
    maximum_timestamps: u8,
    clock_domain_count: u8,
    identity_allowed: bool,
    correlation_allowed: bool,
    causation_allowed: bool,
    provenance_allowed: bool,
    sensitivity_ceiling: Sensitivity,
}

#[derive(Debug)]
pub(crate) struct RuntimePlanNode {
    pub(crate) instance: String,
    pub(crate) contract_id: String,
    pub(crate) contract_hash: SemanticHash,
    pub(crate) implementation_id: String,
    pub(crate) implementation_hash: SemanticHash,
    pub(crate) artifact_id: String,
    pub(crate) host_id: String,
    pub(crate) host_observation_id: String,
    pub(crate) limits: conduit_core::ExecutionLimits,
}

#[derive(Debug)]
pub(crate) struct RuntimePlanCord {
    pub(crate) id: String,
    pub(crate) from_node: usize,
    pub(crate) to_node: usize,
    pub(crate) from_port: String,
    pub(crate) to_port: String,
    pub(crate) flow: RuntimeFlowPolicy,
    pub(crate) envelope: Option<RuntimeEnvelopePolicy>,
    pub(crate) queue_memory_bytes: u64,
}

#[derive(Debug)]
struct RuntimeFanout {
    producer_node: usize,
    mode: FanOutMode,
    branches: Vec<usize>,
    shared_handle: bool,
}

#[derive(Debug)]
struct RuntimeFeedbackBoundary {
    maximum_retained_items: u16,
    maximum_retained_bytes: u64,
}

/// Exact facts retained by the scheduler after start. It deliberately copies
/// only runtime-relevant plan data, preserving the full plan's canonical
/// identity outside the scheduler while allowing the caller's plan arena to
/// be released immediately after admission.
#[derive(Debug)]
pub(crate) struct RuntimePlan {
    pub(crate) nodes: Vec<RuntimePlanNode>,
    pub(crate) cords: Vec<RuntimePlanCord>,
    fanouts: Vec<RuntimeFanout>,
    feedback_boundaries: Vec<RuntimeFeedbackBoundary>,
    budget: PlanResourceBudget,
}

impl RuntimePlan {
    fn from_exact(plan: &ExecutionPlan<'_>) -> Result<Self, SchedulerError> {
        let mut nodes = try_vec_capacity(plan.nodes.len())?;
        for node in plan.nodes {
            let profile = node.execution_profile.ok_or(SchedulerError::InvalidPlan)?;
            nodes.push(RuntimePlanNode {
                instance: copy_runtime_id(node.instance.as_str())?,
                contract_id: copy_runtime_id(node.contract.id.as_str())?,
                contract_hash: node.contract.semantic_hash,
                implementation_id: copy_runtime_id(node.implementation.id.as_str())?,
                implementation_hash: node.implementation.semantic_hash,
                artifact_id: copy_runtime_id(node.artifact.as_str())?,
                host_id: copy_runtime_id(node.host.as_str())?,
                host_observation_id: copy_runtime_id(node.host_observation.as_str())?,
                limits: profile.limits,
            });
        }

        let mut cords = try_vec_capacity(plan.cords.len())?;
        for cord in plan.cords {
            let from_node = nodes
                .iter()
                .position(|node| node.instance == cord.from.node.as_str())
                .ok_or(SchedulerError::InvalidPlan)?;
            let to_node = nodes
                .iter()
                .position(|node| node.instance == cord.to.node.as_str())
                .ok_or(SchedulerError::InvalidPlan)?;
            let envelope = match plan
                .value_envelopes
                .iter()
                .find(|policy| policy.cord == cord.id)
            {
                Some(policy) => Some(RuntimeEnvelopePolicy {
                    representation: policy.representation.semantic_hash,
                    maximum_payload_bytes: policy.maximum_payload_bytes,
                    maximum_envelope_bytes: policy.maximum_envelope_bytes,
                    maximum_fragments: policy.maximum_fragments,
                    maximum_fragment_bytes: policy.maximum_fragment_bytes,
                    maximum_timestamps: policy.maximum_timestamps,
                    clock_domain_count: u8::try_from(policy.clock_domains.len())
                        .map_err(|_| SchedulerError::InvalidPlan)?,
                    identity_allowed: policy.identity_allowed,
                    correlation_allowed: policy.correlation_allowed,
                    causation_allowed: policy.causation_allowed,
                    provenance_allowed: policy.provenance_allowed,
                    sensitivity_ceiling: policy.sensitivity_ceiling,
                }),
                None => None,
            };
            cords.push(RuntimePlanCord {
                id: copy_runtime_id(cord.id.as_str())?,
                from_node,
                to_node,
                from_port: copy_runtime_id(cord.from.port.as_str())?,
                to_port: copy_runtime_id(cord.to.port.as_str())?,
                flow: RuntimeFlowPolicy::from_plan(cord.flow),
                envelope,
                queue_memory_bytes: cord.queue_memory_bytes,
            });
        }

        let mut fanouts = try_vec_capacity(plan.fanouts.len())?;
        for fanout in plan.fanouts {
            let producer_node = nodes
                .iter()
                .position(|node| node.instance == fanout.producer.node.as_str())
                .ok_or(SchedulerError::InvalidPlan)?;
            let mut branches = try_vec_capacity(fanout.branches.len())?;
            for branch in fanout.branches {
                branches.push(
                    cords
                        .iter()
                        .position(|cord| cord.id == branch.as_str())
                        .ok_or(SchedulerError::InvalidPlan)?,
                );
            }
            fanouts.push(RuntimeFanout {
                producer_node,
                mode: fanout.mode,
                branches,
                shared_handle: matches!(fanout.duplication, DuplicationRule::SharedHandle),
            });
        }

        let mut feedback_boundaries = try_vec_capacity(plan.feedback_boundaries.len())?;
        for boundary in plan.feedback_boundaries {
            feedback_boundaries.push(RuntimeFeedbackBoundary {
                maximum_retained_items: boundary.maximum_retained_items,
                maximum_retained_bytes: boundary.maximum_retained_bytes,
            });
        }
        Ok(Self {
            nodes,
            cords,
            fanouts,
            feedback_boundaries,
            budget: plan.budget,
        })
    }
}

fn runtime_plan_storage_bytes(plan: &ExecutionPlan<'_>) -> Result<u64, SchedulerError> {
    let host_io_bytes = plan.nodes.iter().try_fold(0_u64, |total, node| {
        let profile = node.execution_profile.ok_or(SchedulerError::InvalidPlan)?;
        total
            .checked_add(profile.limits.max_host_buffer_bytes)
            .ok_or(SchedulerError::ArithmeticOverflow)
    })?;
    let node_ids = plan.nodes.iter().try_fold(0_u64, |total, node| {
        [
            node.instance.as_str(),
            node.contract.id.as_str(),
            node.implementation.id.as_str(),
            node.artifact.as_str(),
            node.host.as_str(),
            node.host_observation.as_str(),
        ]
        .into_iter()
        .try_fold(total, |total, id| {
            total.checked_add(u64::try_from(id.len()).ok()?)
        })
    });
    let cord_ids = plan.cords.iter().try_fold(0_u64, |total, cord| {
        [
            cord.id.as_str(),
            cord.from.port.as_str(),
            cord.to.port.as_str(),
        ]
        .into_iter()
        .try_fold(total, |total, id| {
            total.checked_add(u64::try_from(id.len()).ok()?)
        })
    });
    let fanout_branches = plan.fanouts.iter().try_fold(0_u64, |total, fanout| {
        total.checked_add(u64::try_from(fanout.branches.len()).ok()?)
    });
    [
        checked_size(
            u64::try_from(plan.nodes.len()).map_err(|_| SchedulerError::ArithmeticOverflow)?,
            size_of::<RuntimePlanNode>(),
        )?,
        checked_size(
            u64::try_from(plan.cords.len()).map_err(|_| SchedulerError::ArithmeticOverflow)?,
            size_of::<RuntimePlanCord>(),
        )?,
        checked_size(
            u64::try_from(plan.fanouts.len()).map_err(|_| SchedulerError::ArithmeticOverflow)?,
            size_of::<RuntimeFanout>(),
        )?,
        checked_size(
            u64::try_from(plan.feedback_boundaries.len())
                .map_err(|_| SchedulerError::ArithmeticOverflow)?,
            size_of::<RuntimeFeedbackBoundary>(),
        )?,
        checked_size(
            fanout_branches.ok_or(SchedulerError::ArithmeticOverflow)?,
            size_of::<usize>(),
        )?,
        node_ids.ok_or(SchedulerError::ArithmeticOverflow)?,
        cord_ids.ok_or(SchedulerError::ArithmeticOverflow)?,
        host_io_bytes,
    ]
    .into_iter()
    .try_fold(0_u64, u64::checked_add)
    .ok_or(SchedulerError::ArithmeticOverflow)
}

fn copy_runtime_id(value: &str) -> Result<String, SchedulerError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| SchedulerError::AllocationFailed)?;
    owned.push_str(value);
    Ok(owned)
}

impl RuntimeValueEnvelope {
    pub const EMPTY: Self = Self {
        representation: SemanticHash::from_bytes([0; 32]),
        envelope_bytes: 0,
        fragment_count: 0,
        fragment_bytes: 0,
        identity: None,
        correlation: None,
        causation: None,
        provenance: None,
        timestamp_count: 0,
        timestamps: [RuntimeTimestamp {
            domain_index: 0,
            tick: 0,
            uncertainty_ticks: 0,
        }; conduit_core::MAX_VALUE_CLOCK_DOMAINS],
        sensitivity: Sensitivity::Public,
    };
}

pub fn validate_runtime_value_for_cord(
    plan: &ExecutionPlan<'_>,
    cord: Id<'_>,
    value: RuntimeValue,
) -> Result<(), ValueEnvelopeReason> {
    let Some(policy) = plan
        .value_envelopes
        .iter()
        .find(|policy| policy.cord == cord)
    else {
        return (value.envelope == RuntimeValueEnvelope::EMPTY)
            .then_some(())
            .ok_or(ValueEnvelopeReason::UnauthorizedField);
    };
    let envelope = value.envelope;
    if envelope.representation != policy.representation.semantic_hash {
        return Err(ValueEnvelopeReason::RepresentationMismatch);
    }
    if value.accounted_bytes == 0
        || value.accounted_bytes > policy.maximum_payload_bytes
        || envelope.envelope_bytes == 0
        || envelope.envelope_bytes > policy.maximum_envelope_bytes
        || envelope.fragment_count == 0
        || envelope.fragment_count > policy.maximum_fragments
        || envelope.fragment_bytes < value.accounted_bytes
        || envelope.fragment_bytes
            > u32::from(envelope.fragment_count)
                .checked_mul(policy.maximum_fragment_bytes)
                .ok_or(ValueEnvelopeReason::InvalidBound)?
        || envelope.timestamp_count > policy.maximum_timestamps
    {
        return Err(ValueEnvelopeReason::InvalidBound);
    }
    if (envelope.identity.is_some() && !policy.identity_allowed)
        || (envelope.correlation.is_some() && !policy.correlation_allowed)
        || (envelope.causation.is_some() && !policy.causation_allowed)
        || (envelope.provenance.is_some() && !policy.provenance_allowed)
    {
        return Err(ValueEnvelopeReason::UnauthorizedField);
    }
    let zero = SemanticHash::from_bytes([0; 32]);
    if [
        envelope.identity,
        envelope.correlation,
        envelope.causation,
        envelope.provenance,
    ]
    .into_iter()
    .flatten()
    .any(|identity| identity == zero)
    {
        return Err(ValueEnvelopeReason::UnauthorizedField);
    }
    let timestamp_count = usize::from(envelope.timestamp_count);
    for (index, timestamp) in envelope.timestamps[..timestamp_count].iter().enumerate() {
        if usize::from(timestamp.domain_index) >= policy.clock_domains.len()
            || envelope.timestamps[..index]
                .iter()
                .any(|prior| prior.domain_index == timestamp.domain_index)
        {
            return Err(ValueEnvelopeReason::ClockNotAuthorized);
        }
    }
    if envelope.sensitivity > policy.sensitivity_ceiling {
        return Err(ValueEnvelopeReason::SensitivityWidening);
    }
    Ok(())
}

fn validate_runtime_value_for_runtime_cord(
    policy: Option<RuntimeEnvelopePolicy>,
    value: RuntimeValue,
) -> Result<(), ValueEnvelopeReason> {
    let Some(policy) = policy else {
        return (value.envelope == RuntimeValueEnvelope::EMPTY)
            .then_some(())
            .ok_or(ValueEnvelopeReason::UnauthorizedField);
    };
    let envelope = value.envelope;
    if envelope.representation != policy.representation
        || value.accounted_bytes == 0
        || value.accounted_bytes > policy.maximum_payload_bytes
        || envelope.envelope_bytes == 0
        || envelope.envelope_bytes > policy.maximum_envelope_bytes
        || envelope.fragment_count == 0
        || envelope.fragment_count > policy.maximum_fragments
        || envelope.fragment_bytes < value.accounted_bytes
        || envelope.fragment_bytes
            > u32::from(envelope.fragment_count)
                .checked_mul(policy.maximum_fragment_bytes)
                .ok_or(ValueEnvelopeReason::InvalidBound)?
        || envelope.timestamp_count > policy.maximum_timestamps
        || envelope.timestamp_count > policy.clock_domain_count
    {
        return Err(ValueEnvelopeReason::InvalidBound);
    }
    if (envelope.identity.is_some() && !policy.identity_allowed)
        || (envelope.correlation.is_some() && !policy.correlation_allowed)
        || (envelope.causation.is_some() && !policy.causation_allowed)
        || (envelope.provenance.is_some() && !policy.provenance_allowed)
    {
        return Err(ValueEnvelopeReason::UnauthorizedField);
    }
    let zero = SemanticHash::from_bytes([0; 32]);
    if [
        envelope.identity,
        envelope.correlation,
        envelope.causation,
        envelope.provenance,
    ]
    .into_iter()
    .flatten()
    .any(|identity| identity == zero)
    {
        return Err(ValueEnvelopeReason::UnauthorizedField);
    }
    let timestamp_count = usize::from(envelope.timestamp_count);
    for (index, timestamp) in envelope.timestamps[..timestamp_count].iter().enumerate() {
        if usize::from(timestamp.domain_index) >= usize::from(policy.clock_domain_count)
            || envelope.timestamps[..index]
                .iter()
                .any(|prior| prior.domain_index == timestamp.domain_index)
        {
            return Err(ValueEnvelopeReason::ClockNotAuthorized);
        }
    }
    if envelope.sensitivity > policy.sensitivity_ceiling {
        return Err(ValueEnvelopeReason::SensitivityWidening);
    }
    Ok(())
}

/// Opaque executor-mediated value. Payload ownership remains in the exact
/// representation binding; the cord charges `accounted_bytes` against its
/// plan-reserved byte arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeValue {
    pub handle: u64,
    pub accounted_bytes: u32,
    pub envelope: RuntimeValueEnvelope,
}

/// Result of attempting to reserve one output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendStatus {
    Reserved,
    WouldBlock,
    Rejected,
    Dropped,
    Disconnected,
    Failed,
    /// The cord is draining or terminal and cannot accept a new value.
    Terminated,
}

/// Result returned by one scheduler-facing node step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerStep {
    Progress,
    Pending,
    Yielded,
    Completed,
    Failed { code: Id<'static> },
}

/// Small synchronous interface adapted by native, process, WASM, or embedded
/// implementation bindings. No async framework type crosses this boundary.
pub trait SchedulerNode {
    fn prepare(&mut self) -> Result<LifecycleUsage, Id<'static>>;
    fn start(&mut self) -> Result<LifecycleUsage, Id<'static>>;
    fn step(&mut self, io: &mut StepIo<'_>) -> SchedulerStep;

    fn cancel(&mut self, _stop: StopPolicy) {}

    /// Begins one bounded reconciliation of implementation-owned value
    /// storage. Portable drivers have no external value arena by default.
    fn begin_value_reconciliation(&mut self) {}

    /// Marks a queue-owned value as live for the current reconciliation.
    fn mark_value_live(&mut self, _value: RuntimeValue) {}

    /// Marks values retained by implementation state as live.
    fn mark_retained_values(&mut self) {}

    /// Releases unmarked values from the implementation's bounded arena.
    fn finish_value_reconciliation(&mut self) {}

    /// Fixed host value-arena accounting, when this driver exposes one.
    fn value_storage_usage(&self) -> Option<ValueStorageUsage> {
        None
    }
}

/// One already-instantiated driver and its portable implementation validator.
pub struct ScheduledNode<N> {
    pub driver: N,
    pub machine: ImplementationMachine,
}

/// Caller-declared host memory available to atomic startup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerReservation {
    /// Complete runtime ceiling, including plan allocations and executor
    /// overhead.
    pub available_runtime_memory_bytes: u64,
    /// Host-declared ceiling for scheduler metadata, ready/wait state, and
    /// normative event storage.
    pub executor_overhead_limit_bytes: u64,
}

/// Exact preallocation report retained as run-start evidence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SchedulerAllocation {
    pub node_memory_bytes: u64,
    pub cord_memory_bytes: u64,
    pub feedback_memory_bytes: u64,
    pub pool_memory_bytes: u64,
    pub event_stream_memory_bytes: u64,
    pub job_memory_bytes: u64,
    pub planned_memory_bytes: u64,
    pub planned_evidence_bytes: u64,
    pub queue_payload_bytes: u64,
    pub executor_overhead_bytes: u64,
    pub scheduler_evidence_bytes: u64,
    pub queue_slots: u64,
    pub ready_slots: u32,
    pub wake_interest_slots: u64,
    pub transaction_slots: u64,
    pub event_slots: u32,
}

/// Deterministic high-water observations. These are measurements of one run,
/// not semantic guarantees or substitutes for the plan reservation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SchedulerHighWater {
    pub queue_items: u64,
    pub queue_payload_bytes: u64,
    pub ready_slots: u32,
    pub event_slots: u32,
    pub decisions: u64,
}

/// Current and high-water occupancy of host-owned payload storage.
///
/// This is runtime accounting, not a semantic input or authority fact.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ValueStorageUsage {
    pub resident_slots: u32,
    pub resident_bytes: u64,
    pub high_water_slots: u32,
    pub high_water_bytes: u64,
    pub maximum_slots: u32,
    pub maximum_bytes: u64,
}

/// Deterministic run state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerStatus {
    Running,
    Stalled,
    Succeeded,
    Cancelled,
    Disconnected,
    Failed(SchedulerError),
}

/// One bounded executor-owned observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerEvent {
    pub sequence: u64,
    pub tick: u64,
    pub subject: SchedulerSubject,
    pub kind: SchedulerEventKind,
    pub occupancy_items: u16,
    pub occupancy_bytes: u64,
    /// Opaque executor handle observed at this transaction boundary.
    pub value_handle: Option<u64>,
    /// Prior or input handle related to `value_handle`.
    pub related_value_handle: Option<u64>,
    /// Exact local scheduler ticks from readiness to selection.
    pub scheduling_latency_ticks: u64,
    /// Exact deterministic executor ticks charged to the selected step.
    pub processing_latency_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerSubject {
    Run,
    Node(u16),
    Cord(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerEventKind {
    AllocationPrepared,
    NodePrepared,
    RunStarted,
    Decision { reason: SchedulerDecisionReason },
    NodeOutcome { outcome: StepOutcomeKind },
    Cord(FlowEventKind),
    ValueAccepted,
    ValueConsumed,
    DerivationCommitted,
    NodeWoken { reason: SchedulerDecisionReason },
    CancellationRequested { stop: StopPolicy },
    Terminal(TerminalClass),
}

/// Stable scheduler failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    InvalidPolicy,
    InvalidPlan,
    NodeSetMismatch,
    ArithmeticOverflow,
    AllocationExceedsPlan,
    AllocationUnavailable,
    AllocationFailed,
    PrepareFailed,
    StartFailed,
    StepContractViolation,
    PortAccessViolation,
    TransactionCapacityExceeded,
    DecisionLimitExceeded,
    ClockLimitExceeded,
    EvidenceCapacityExceeded,
    ZeroProgressLivelock,
    CancellationDeadlineExceeded,
    NodeFailed,
    QueueFailed,
    ValueEnvelope(ValueEnvelopeReason),
}

impl SchedulerError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "CND-SCH-001",
            Self::InvalidPlan | Self::NodeSetMismatch => "CND-SCH-004",
            Self::ArithmeticOverflow
            | Self::AllocationExceedsPlan
            | Self::AllocationUnavailable
            | Self::AllocationFailed => "CND-SCH-005",
            Self::PrepareFailed | Self::StartFailed => "CND-SCH-006",
            Self::StepContractViolation | Self::PortAccessViolation => "CND-SCH-007",
            Self::TransactionCapacityExceeded => "CND-SCH-008",
            Self::DecisionLimitExceeded | Self::ClockLimitExceeded => "CND-SCH-009",
            Self::EvidenceCapacityExceeded => "CND-SCH-010",
            Self::ZeroProgressLivelock => "CND-SCH-011",
            Self::CancellationDeadlineExceeded => "CND-SCH-012",
            Self::NodeFailed | Self::QueueFailed => "CND-SCH-013",
            Self::ValueEnvelope(reason) => reason.code(),
        }
    }
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPolicy => "scheduler policy is invalid",
            Self::InvalidPlan => "execution plan is not runnable by the scheduler",
            Self::NodeSetMismatch => "scheduled nodes do not exactly match plan nodes",
            Self::ArithmeticOverflow => "scheduler allocation arithmetic overflowed",
            Self::AllocationExceedsPlan => "scheduler allocation exceeds plan memory",
            Self::AllocationUnavailable => "host reservation cannot cover atomic startup",
            Self::AllocationFailed => "fixed scheduler storage allocation failed",
            Self::PrepareFailed => "prepare-all failed before any node started",
            Self::StartFailed => "start-all failed before any node started",
            Self::StepContractViolation => "node violated the bounded step contract",
            Self::ValueEnvelope(_) => "runtime value violates its exact envelope policy",
            Self::PortAccessViolation => "node accessed a cord outside its exact endpoints",
            Self::TransactionCapacityExceeded => "step exceeded preallocated transaction storage",
            Self::DecisionLimitExceeded => "scheduler decision limit was exhausted",
            Self::ClockLimitExceeded => "simulated-clock limit was exhausted",
            Self::EvidenceCapacityExceeded => "normative scheduler evidence storage is full",
            Self::ZeroProgressLivelock => "a node exhausted its bounded yield budget",
            Self::CancellationDeadlineExceeded => "bounded cancellation deadline was exhausted",
            Self::NodeFailed => "a node reported failure",
            Self::QueueFailed => "a cord pressure or terminal transition failed the run",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueuedValue {
    value: RuntimeValue,
    bytes: u32,
}

struct RuntimeCord {
    policy: RuntimeFlowPolicy,
    slots: Box<[Option<QueuedValue>]>,
    // This is the exact plan-owned payload reservation. Value handles point to
    // representation-owned storage, but no other queued payload can exist.
    _payload_reservation: Box<[u8]>,
    head: usize,
    len: u16,
    queued_bytes: u64,
    arrival_sequence: u64,
    flow_sequence: u64,
    pressured: bool,
    producer_waiting: bool,
    consumer_waiting: bool,
    state: FlowQueueState,
    drain_target: Option<FlowQueueState>,
}

struct FeedbackReservation {
    _bytes: Box<[u8]>,
    _items: Box<[Option<RuntimeValue>]>,
}

#[derive(Clone, Copy, Debug)]
struct CordEventBatch {
    values: [Option<CordEvent>; 4],
    len: u8,
}

#[derive(Clone, Copy, Debug)]
struct CordEvent {
    kind: SchedulerEventKind,
    occupancy_items: u16,
    occupancy_bytes: u64,
    value_handle: Option<u64>,
    related_value_handle: Option<u64>,
}

impl CordEventBatch {
    const fn new() -> Self {
        Self {
            values: [None, None, None, None],
            len: 0,
        }
    }

    fn push(&mut self, event: CordEvent) {
        let index = usize::from(self.len);
        if index < self.values.len() {
            self.values[index] = Some(event);
            self.len += 1;
        }
    }

    fn iter(&self) -> impl Iterator<Item = CordEvent> + '_ {
        self.values[..usize::from(self.len)]
            .iter()
            .copied()
            .flatten()
    }
}

impl RuntimeCord {
    fn allocate(
        policy: RuntimeFlowPolicy,
        queue_memory_bytes: u64,
    ) -> Result<Self, SchedulerError> {
        let slot_count = usize::from(policy.capacity_items);
        let payload_len =
            usize::try_from(queue_memory_bytes).map_err(|_| SchedulerError::AllocationFailed)?;
        let mut slot_vec = Vec::new();
        slot_vec
            .try_reserve_exact(slot_count)
            .map_err(|_| SchedulerError::AllocationFailed)?;
        slot_vec.resize(slot_count, None);
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(payload_len)
            .map_err(|_| SchedulerError::AllocationFailed)?;
        payload.resize(payload_len, 0);
        Ok(Self {
            policy,
            slots: slot_vec.into_boxed_slice(),
            _payload_reservation: payload.into_boxed_slice(),
            head: 0,
            len: 0,
            queued_bytes: 0,
            arrival_sequence: 0,
            flow_sequence: 0,
            pressured: false,
            producer_waiting: false,
            consumer_waiting: false,
            state: FlowQueueState::Active,
            drain_target: None,
        })
    }

    const fn occupancy_items(&self) -> u16 {
        self.len
    }

    const fn occupancy_bytes(&self) -> u64 {
        self.queued_bytes
    }

    const fn state(&self) -> FlowQueueState {
        self.state
    }

    fn value_at(&self, offset: u16) -> Option<RuntimeValue> {
        if offset >= self.len {
            return None;
        }
        let slot = (self.head + usize::from(offset)) % self.slots.len();
        self.slots[slot].map(|entry| entry.value)
    }

    fn visit_values(&self, mut visit: impl FnMut(RuntimeValue)) {
        for offset in 0..self.len {
            let slot = (self.head + usize::from(offset)) % self.slots.len();
            if let Some(entry) = self.slots[slot] {
                visit(entry.value);
            }
        }
    }

    fn size_at(&self, offset: u16) -> Option<u32> {
        if offset >= self.len {
            return None;
        }
        let slot = (self.head + usize::from(offset)) % self.slots.len();
        self.slots[slot].map(|entry| entry.bytes)
    }

    fn can_fit(&self, value: RuntimeValue, extra_items: u16, extra_bytes: u64) -> bool {
        value.accounted_bytes <= self.policy.maximum_value_bytes
            && self
                .len
                .checked_add(extra_items)
                .is_some_and(|items| items < self.policy.capacity_items)
            && self
                .queued_bytes
                .checked_add(extra_bytes)
                .and_then(|bytes| bytes.checked_add(u64::from(value.accounted_bytes)))
                .is_some_and(|bytes| bytes <= self.policy.maximum_queued_bytes)
    }

    fn mark_producer_waiting(&mut self) -> CordEventBatch {
        let mut events = CordEventBatch::new();
        self.producer_waiting = true;
        if !self.pressured {
            self.pressured = true;
            self.emit(&mut events, FlowEventKind::PressureEntered);
        }
        events
    }

    fn mark_consumer_waiting(&mut self) {
        if self.state == FlowQueueState::Active {
            self.consumer_waiting = true;
        }
    }

    fn offer(
        &mut self,
        value: RuntimeValue,
        coalesce_target: Option<u16>,
    ) -> (OfferDisposition<RuntimeValue>, CordEventBatch) {
        let mut events = CordEventBatch::new();
        if self.state != FlowQueueState::Active {
            return (OfferDisposition::Terminated(value), events);
        }
        let arrival = self.arrival_sequence;
        self.arrival_sequence = self.arrival_sequence.wrapping_add(1);
        if let RuntimePressure::Sample { every, offset } = self.policy.pressure {
            if arrival % u64::from(every) != u64::from(offset) {
                self.emit_value(
                    &mut events,
                    FlowEventKind::ValueSampledOut,
                    value.handle,
                    None,
                );
                return (OfferDisposition::Dropped(value), events);
            }
        }
        if value.accounted_bytes > self.policy.maximum_value_bytes {
            self.emit_value(
                &mut events,
                FlowEventKind::ValueRejected,
                value.handle,
                None,
            );
            return (OfferDisposition::Rejected(value), events);
        }
        let fits = self.len < self.policy.capacity_items
            && self
                .queued_bytes
                .checked_add(u64::from(value.accounted_bytes))
                .is_some_and(|bytes| bytes <= self.policy.maximum_queued_bytes);
        if fits {
            self.push_back(value);
            self.emit_scheduler(
                &mut events,
                SchedulerEventKind::ValueAccepted,
                Some(value.handle),
                None,
            );
            if self.len >= self.policy.high_watermark_items && !self.pressured {
                self.pressured = true;
                self.emit(&mut events, FlowEventKind::PressureEntered);
            }
            if self.consumer_waiting {
                self.consumer_waiting = false;
                self.emit(&mut events, FlowEventKind::ConsumerReady);
            }
            return (OfferDisposition::Enqueued, events);
        }
        if !self.pressured {
            self.pressured = true;
            self.emit(&mut events, FlowEventKind::PressureEntered);
        }
        let disposition = match self.policy.pressure {
            RuntimePressure::Block => {
                self.producer_waiting = true;
                OfferDisposition::Pending(value)
            }
            RuntimePressure::Reject => {
                self.emit_value(
                    &mut events,
                    FlowEventKind::ValueRejected,
                    value.handle,
                    None,
                );
                OfferDisposition::Rejected(value)
            }
            RuntimePressure::Coalesce => {
                let Some(target) = coalesce_target else {
                    self.emit_value(
                        &mut events,
                        FlowEventKind::ValueRejected,
                        value.handle,
                        None,
                    );
                    return (OfferDisposition::Rejected(value), events);
                };
                if target >= self.len {
                    self.emit_value(
                        &mut events,
                        FlowEventKind::ValueRejected,
                        value.handle,
                        None,
                    );
                    return (OfferDisposition::Rejected(value), events);
                }
                let slot = (self.head + usize::from(target)) % self.slots.len();
                let old = self.slots[slot].expect("coalescing target is occupied");
                let new_bytes =
                    self.queued_bytes - u64::from(old.bytes) + u64::from(value.accounted_bytes);
                if new_bytes > self.policy.maximum_queued_bytes {
                    self.emit_value(
                        &mut events,
                        FlowEventKind::ValueRejected,
                        value.handle,
                        None,
                    );
                    return (OfferDisposition::Rejected(value), events);
                }
                self.slots[slot] = Some(QueuedValue {
                    value,
                    bytes: value.accounted_bytes,
                });
                self.queued_bytes = new_bytes;
                self.emit_value(
                    &mut events,
                    FlowEventKind::ValueCoalesced { target },
                    value.handle,
                    Some(old.value.handle),
                );
                OfferDisposition::Coalesced {
                    replaced: old.value,
                }
            }
            RuntimePressure::Sample { .. } => {
                self.emit_value(
                    &mut events,
                    FlowEventKind::ValueSampledOut,
                    value.handle,
                    None,
                );
                OfferDisposition::Dropped(value)
            }
            RuntimePressure::DropDisposable => {
                self.emit_value(
                    &mut events,
                    FlowEventKind::ValueDroppedDisposable,
                    value.handle,
                    None,
                );
                OfferDisposition::Dropped(value)
            }
            RuntimePressure::Disconnect => {
                self.state = FlowQueueState::Disconnected;
                self.emit_value(&mut events, FlowEventKind::Disconnected, value.handle, None);
                OfferDisposition::Disconnected(value)
            }
            RuntimePressure::Fail => {
                self.state = FlowQueueState::Failed;
                self.emit_value(&mut events, FlowEventKind::Failed, value.handle, None);
                OfferDisposition::Failed(value)
            }
        };
        (disposition, events)
    }

    fn pop(&mut self) -> (Option<RuntimeValue>, CordEventBatch) {
        let mut events = CordEventBatch::new();
        if self.len == 0 {
            self.mark_consumer_waiting();
            return (None, events);
        }
        let entry = self.slots[self.head]
            .take()
            .expect("queue head is occupied");
        self.head = (self.head + 1) % self.slots.len();
        self.len -= 1;
        self.queued_bytes -= u64::from(entry.bytes);
        self.emit_scheduler(
            &mut events,
            SchedulerEventKind::ValueConsumed,
            Some(entry.value.handle),
            None,
        );
        if self.pressured && self.len <= self.policy.low_watermark_items {
            self.pressured = false;
            self.emit(&mut events, FlowEventKind::PressureCleared);
        }
        if self.producer_waiting {
            self.producer_waiting = false;
            self.emit(&mut events, FlowEventKind::ProducerReady);
        }
        self.finish_drain_if_empty(&mut events);
        (Some(entry.value), events)
    }

    fn complete_source(&mut self) -> CordEventBatch {
        self.begin_drain(FlowQueueState::Completed)
    }

    fn terminate(&mut self, class: TerminalClass, stop: StopPolicy) -> CordEventBatch {
        let target = match class {
            TerminalClass::Succeeded => FlowQueueState::Completed,
            TerminalClass::Disconnected => FlowQueueState::Disconnected,
            TerminalClass::Cancelled => FlowQueueState::Cancelled,
            TerminalClass::Failed => FlowQueueState::Failed,
        };
        if class == TerminalClass::Succeeded || stop == StopPolicy::Drain {
            return self.begin_drain(target);
        }
        let mut events = CordEventBatch::new();
        let items = self.len;
        let bytes = self.queued_bytes;
        while self.len != 0 {
            let entry = self.slots[self.head]
                .take()
                .expect("queue prefix is occupied");
            self.head = (self.head + 1) % self.slots.len();
            self.len -= 1;
            self.queued_bytes -= u64::from(entry.bytes);
        }
        if items != 0 {
            self.emit(
                &mut events,
                FlowEventKind::ValuesDiscardedOnAbort { items, bytes },
            );
        }
        let wake_producer = self.producer_waiting;
        let wake_consumer = self.consumer_waiting;
        self.producer_waiting = false;
        self.consumer_waiting = false;
        self.pressured = false;
        self.drain_target = None;
        self.state = target;
        match target {
            FlowQueueState::Cancelled => self.emit(
                &mut events,
                FlowEventKind::Cancelled {
                    wake_producer,
                    wake_consumer,
                },
            ),
            FlowQueueState::Failed => self.emit(&mut events, FlowEventKind::Failed),
            FlowQueueState::Disconnected => {
                self.emit(&mut events, FlowEventKind::Disconnected);
            }
            FlowQueueState::Active | FlowQueueState::Draining | FlowQueueState::Completed => {}
        }
        events
    }

    fn begin_drain(&mut self, target: FlowQueueState) -> CordEventBatch {
        let mut events = CordEventBatch::new();
        if !matches!(
            self.state,
            FlowQueueState::Active | FlowQueueState::Draining
        ) {
            return events;
        }
        if self.state == FlowQueueState::Draining {
            let current = self.drain_target.unwrap_or(target);
            let selected = more_severe_terminal(current, target);
            if selected != current {
                self.drain_target = Some(selected);
                self.emit(
                    &mut events,
                    FlowEventKind::DrainStarted { terminal: selected },
                );
            }
            return events;
        }
        let wake_producer = self.producer_waiting;
        let wake_consumer = self.consumer_waiting;
        self.producer_waiting = false;
        self.consumer_waiting = false;
        if self.len == 0 {
            self.state = target;
            self.emit_terminal(&mut events, target, wake_producer, wake_consumer);
        } else {
            self.state = FlowQueueState::Draining;
            self.drain_target = Some(target);
            self.emit(
                &mut events,
                FlowEventKind::DrainStarted { terminal: target },
            );
            if wake_producer {
                self.emit(&mut events, FlowEventKind::ProducerReady);
            }
        }
        events
    }

    fn finish_drain_if_empty(&mut self, events: &mut CordEventBatch) {
        if self.len == 0 && self.state == FlowQueueState::Draining {
            let target = self
                .drain_target
                .take()
                .expect("draining cord has terminal target");
            self.state = target;
            self.emit_terminal(events, target, false, false);
        }
    }

    fn emit_terminal(
        &mut self,
        events: &mut CordEventBatch,
        target: FlowQueueState,
        wake_producer: bool,
        wake_consumer: bool,
    ) {
        match target {
            FlowQueueState::Completed => self.emit(events, FlowEventKind::Completed),
            FlowQueueState::Cancelled => self.emit(
                events,
                FlowEventKind::Cancelled {
                    wake_producer,
                    wake_consumer,
                },
            ),
            FlowQueueState::Failed => self.emit(events, FlowEventKind::Failed),
            FlowQueueState::Disconnected => self.emit(events, FlowEventKind::Disconnected),
            FlowQueueState::Active | FlowQueueState::Draining => {}
        }
    }

    fn push_back(&mut self, value: RuntimeValue) {
        let slot = (self.head + usize::from(self.len)) % self.slots.len();
        self.slots[slot] = Some(QueuedValue {
            value,
            bytes: value.accounted_bytes,
        });
        self.len += 1;
        self.queued_bytes += u64::from(value.accounted_bytes);
    }

    fn emit(&mut self, events: &mut CordEventBatch, kind: FlowEventKind) {
        self.emit_scheduler(events, SchedulerEventKind::Cord(kind), None, None);
    }

    fn emit_value(
        &mut self,
        events: &mut CordEventBatch,
        kind: FlowEventKind,
        value_handle: u64,
        related_value_handle: Option<u64>,
    ) {
        self.emit_scheduler(
            events,
            SchedulerEventKind::Cord(kind),
            Some(value_handle),
            related_value_handle,
        );
    }

    fn emit_scheduler(
        &mut self,
        events: &mut CordEventBatch,
        kind: SchedulerEventKind,
        value_handle: Option<u64>,
        related_value_handle: Option<u64>,
    ) {
        let _sequence = self.flow_sequence;
        self.flow_sequence = self.flow_sequence.wrapping_add(1);
        events.push(CordEvent {
            kind,
            occupancy_items: self.len,
            occupancy_bytes: self.queued_bytes,
            value_handle,
            related_value_handle,
        });
    }
}

const fn terminal_rank(state: FlowQueueState) -> u8 {
    match state {
        FlowQueueState::Completed => 0,
        FlowQueueState::Disconnected => 1,
        FlowQueueState::Cancelled => 2,
        FlowQueueState::Failed => 3,
        FlowQueueState::Active | FlowQueueState::Draining => 0,
    }
}

const fn more_severe_terminal(left: FlowQueueState, right: FlowQueueState) -> FlowQueueState {
    if terminal_rank(right) > terminal_rank(left) {
        right
    } else {
        left
    }
}

#[derive(Clone, Copy, Debug)]
struct StagedInput {
    cord: usize,
    value: RuntimeValue,
}

#[derive(Clone, Copy, Debug)]
struct StagedOutput {
    cord: usize,
    value: RuntimeValue,
    coalesce_target: Option<u16>,
    expected: SendStatus,
    effect: StagedOutputEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StagedOutputEffect {
    Enqueue,
    Other,
    Terminal,
}

#[derive(Clone, Copy, Debug)]
enum QueueProbe {
    ProducerBlocked(usize),
    ConsumerEmpty(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaitSubject {
    Cord(usize),
    Named(Id<'static>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WaitCondition {
    kind: WakeInterestKind,
    subject: WaitSubject,
    deadline_tick: Option<u64>,
}

struct NodeWorkspace {
    inputs: Vec<StagedInput>,
    outputs: Vec<StagedOutput>,
    probes: Vec<QueueProbe>,
    interests: Vec<WaitCondition>,
    work_units: u32,
    host_operations: u16,
    domain_evidence: u16,
    fragments: u16,
}

impl NodeWorkspace {
    fn allocate(
        machine: ImplementationMachine,
        interest_capacity: usize,
    ) -> Result<Self, SchedulerError> {
        let limits = machine.profile().limits;
        Ok(Self {
            inputs: try_vec_capacity(usize::from(limits.max_input_leases))?,
            outputs: try_vec_capacity(usize::from(limits.max_output_reservations))?,
            probes: try_vec_capacity(
                usize::from(limits.max_input_leases)
                    .checked_add(usize::from(limits.max_output_reservations))
                    .ok_or(SchedulerError::ArithmeticOverflow)?,
            )?,
            interests: try_vec_capacity(interest_capacity)?,
            work_units: 0,
            host_operations: 0,
            domain_evidence: 0,
            fragments: 0,
        })
    }

    fn begin_step(&mut self) {
        self.inputs.clear();
        self.outputs.clear();
        self.probes.clear();
        self.interests.clear();
        self.work_units = 0;
        self.host_operations = 0;
        self.domain_evidence = 0;
        self.fragments = 0;
    }
}

fn try_vec_capacity<T>(capacity: usize) -> Result<Vec<T>, SchedulerError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| SchedulerError::AllocationFailed)?;
    Ok(values)
}

/// Executor-mediated step view. Reads and writes are staged and either commit
/// together after a valid progress/completion result or roll back without
/// changing cord ownership.
pub struct StepIo<'a> {
    node: usize,
    tick: u64,
    plan: &'a RuntimePlan,
    cords: &'a [RuntimeCord],
    workspace: &'a mut NodeWorkspace,
}

impl StepIo<'_> {
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub fn remaining_work(&self) -> u32 {
        self.plan.nodes[self.node]
            .limits
            .max_step_work
            .saturating_sub(self.workspace.work_units)
    }

    pub fn consume_work(&mut self, units: u32) -> Result<(), SchedulerError> {
        let next = self
            .workspace
            .work_units
            .checked_add(units)
            .ok_or(SchedulerError::StepContractViolation)?;
        if next > self.plan.nodes[self.node].limits.max_step_work {
            return Err(SchedulerError::StepContractViolation);
        }
        self.workspace.work_units = next;
        Ok(())
    }

    /// Peek and lease the next input. The queue is unchanged until commit.
    pub fn receive(&mut self, cord: usize) -> Result<Option<RuntimeValue>, SchedulerError> {
        let plan_cord = self
            .plan
            .cords
            .get(cord)
            .ok_or(SchedulerError::PortAccessViolation)?;
        if plan_cord.to_node != self.node {
            return Err(SchedulerError::PortAccessViolation);
        }
        let offset = self
            .workspace
            .inputs
            .iter()
            .filter(|input| input.cord == cord)
            .count();
        let offset =
            u16::try_from(offset).map_err(|_| SchedulerError::TransactionCapacityExceeded)?;
        let Some(value) = self.cords[cord].value_at(offset) else {
            push_bounded(&mut self.workspace.probes, QueueProbe::ConsumerEmpty(cord))?;
            return Ok(None);
        };
        push_bounded(&mut self.workspace.inputs, StagedInput { cord, value })?;
        Ok(Some(value))
    }

    /// Reserve one output without exposing the queue or async-runtime channel.
    pub fn send(
        &mut self,
        cord: usize,
        value: RuntimeValue,
        coalesce_target: Option<u16>,
    ) -> Result<SendStatus, SchedulerError> {
        let plan_cord = self
            .plan
            .cords
            .get(cord)
            .ok_or(SchedulerError::PortAccessViolation)?;
        if plan_cord.from_node != self.node {
            return Err(SchedulerError::PortAccessViolation);
        }
        validate_runtime_value_for_runtime_cord(plan_cord.envelope, value)
            .map_err(SchedulerError::ValueEnvelope)?;
        if self.cords[cord].state() != FlowQueueState::Active {
            return Ok(SendStatus::Terminated);
        }
        if self
            .workspace
            .outputs
            .iter()
            .any(|output| output.cord == cord && output.effect == StagedOutputEffect::Terminal)
        {
            return Ok(SendStatus::Terminated);
        }
        let staged_attempts = self
            .workspace
            .outputs
            .iter()
            .filter(|output| output.cord == cord)
            .count();
        let staged_items = self
            .workspace
            .outputs
            .iter()
            .filter(|output| output.cord == cord && output.effect == StagedOutputEffect::Enqueue)
            .count();
        let staged_items =
            u16::try_from(staged_items).map_err(|_| SchedulerError::TransactionCapacityExceeded)?;
        let staged_bytes = self
            .workspace
            .outputs
            .iter()
            .filter(|output| output.cord == cord && output.effect == StagedOutputEffect::Enqueue)
            .try_fold(0_u64, |total, output| {
                total.checked_add(u64::from(output.value.accounted_bytes))
            })
            .ok_or(SchedulerError::ArithmeticOverflow)?;
        let arrival = self.cords[cord]
            .arrival_sequence
            .checked_add(
                u64::try_from(staged_attempts)
                    .map_err(|_| SchedulerError::TransactionCapacityExceeded)?,
            )
            .ok_or(SchedulerError::ArithmeticOverflow)?;
        let sampled_out = matches!(plan_cord.flow.pressure, RuntimePressure::Sample { every, offset }
            if arrival % u64::from(every) != u64::from(offset));
        let fits = self.cords[cord].can_fit(value, staged_items, staged_bytes);
        let (expected, effect) = if sampled_out {
            (SendStatus::Dropped, StagedOutputEffect::Other)
        } else if value.accounted_bytes > plan_cord.flow.maximum_value_bytes {
            (SendStatus::Rejected, StagedOutputEffect::Other)
        } else if fits {
            (SendStatus::Reserved, StagedOutputEffect::Enqueue)
        } else {
            match plan_cord.flow.pressure {
                RuntimePressure::Block => (SendStatus::WouldBlock, StagedOutputEffect::Other),
                RuntimePressure::Reject => (SendStatus::Rejected, StagedOutputEffect::Other),
                RuntimePressure::Coalesce => {
                    let valid = coalesce_target.is_some_and(|target| {
                        target < self.cords[cord].occupancy_items()
                            && !self.workspace.outputs.iter().any(|output| {
                                output.cord == cord && output.coalesce_target.is_some()
                            })
                            && self.cords[cord].size_at(target).is_some_and(|old_bytes| {
                                self.cords[cord]
                                    .occupancy_bytes()
                                    .checked_add(staged_bytes)
                                    .and_then(|bytes| bytes.checked_sub(u64::from(old_bytes)))
                                    .and_then(|bytes| {
                                        bytes.checked_add(u64::from(value.accounted_bytes))
                                    })
                                    .is_some_and(|bytes| {
                                        bytes <= plan_cord.flow.maximum_queued_bytes
                                    })
                            })
                    });
                    if valid {
                        (SendStatus::Reserved, StagedOutputEffect::Other)
                    } else {
                        (SendStatus::Rejected, StagedOutputEffect::Other)
                    }
                }
                RuntimePressure::Sample { .. } | RuntimePressure::DropDisposable => {
                    (SendStatus::Dropped, StagedOutputEffect::Other)
                }
                RuntimePressure::Disconnect => {
                    (SendStatus::Disconnected, StagedOutputEffect::Terminal)
                }
                RuntimePressure::Fail => (SendStatus::Failed, StagedOutputEffect::Terminal),
            }
        };
        if expected == SendStatus::WouldBlock {
            push_bounded(
                &mut self.workspace.probes,
                QueueProbe::ProducerBlocked(cord),
            )?;
            return Ok(expected);
        }
        push_bounded(
            &mut self.workspace.outputs,
            StagedOutput {
                cord,
                value,
                coalesce_target,
                expected,
                effect,
            },
        )?;
        Ok(expected)
    }

    /// Stage every branch of one plan-pinned coupled fan-out. If any branch
    /// cannot accept, the caller returns pending/failed and all earlier
    /// reservations roll back together.
    pub fn send_coupled(
        &mut self,
        fanout: usize,
        branch_values: &[RuntimeValue],
        coalesce_targets: &[Option<u16>],
    ) -> Result<SendStatus, SchedulerError> {
        let contract = self
            .plan
            .fanouts
            .get(fanout)
            .ok_or(SchedulerError::PortAccessViolation)?;
        if contract.mode != FanOutMode::Coupled
            || contract.producer_node != self.node
            || contract.branches.len() != branch_values.len()
            || contract.branches.len() != coalesce_targets.len()
        {
            return Err(SchedulerError::PortAccessViolation);
        }
        if contract.shared_handle && branch_values.windows(2).any(|pair| pair[0] != pair[1]) {
            return Err(SchedulerError::StepContractViolation);
        }
        for (index, &cord) in contract.branches.iter().enumerate() {
            let status = self.send(cord, branch_values[index], coalesce_targets[index])?;
            if status != SendStatus::Reserved {
                return Ok(status);
            }
        }
        Ok(SendStatus::Reserved)
    }

    pub fn wait_for_input(&mut self, cord: usize) -> Result<(), SchedulerError> {
        let plan_cord = self
            .plan
            .cords
            .get(cord)
            .ok_or(SchedulerError::PortAccessViolation)?;
        if plan_cord.to_node != self.node {
            return Err(SchedulerError::PortAccessViolation);
        }
        self.wait(WaitCondition {
            kind: WakeInterestKind::Input,
            subject: WaitSubject::Cord(cord),
            deadline_tick: None,
        })
    }

    pub fn wait_for_output(&mut self, cord: usize) -> Result<(), SchedulerError> {
        let plan_cord = self
            .plan
            .cords
            .get(cord)
            .ok_or(SchedulerError::PortAccessViolation)?;
        if plan_cord.from_node != self.node {
            return Err(SchedulerError::PortAccessViolation);
        }
        self.wait(WaitCondition {
            kind: WakeInterestKind::Output,
            subject: WaitSubject::Cord(cord),
            deadline_tick: None,
        })
    }

    pub fn wait_for_timer(
        &mut self,
        subject: Id<'static>,
        deadline_tick: u64,
    ) -> Result<(), SchedulerError> {
        if deadline_tick <= self.tick {
            return Err(SchedulerError::StepContractViolation);
        }
        self.wait(WaitCondition {
            kind: WakeInterestKind::Timer,
            subject: WaitSubject::Named(subject),
            deadline_tick: Some(deadline_tick),
        })
    }

    pub fn wait_for_host_operation(&mut self, subject: Id<'static>) -> Result<(), SchedulerError> {
        self.wait(WaitCondition {
            kind: WakeInterestKind::HostOperation,
            subject: WaitSubject::Named(subject),
            deadline_tick: None,
        })
    }

    pub fn wait_for_cancellation(&mut self, subject: Id<'static>) -> Result<(), SchedulerError> {
        self.wait(WaitCondition {
            kind: WakeInterestKind::Cancellation,
            subject: WaitSubject::Named(subject),
            deadline_tick: None,
        })
    }

    pub fn record_host_progress(&mut self) -> Result<(), SchedulerError> {
        self.workspace.host_operations = self
            .workspace
            .host_operations
            .checked_add(1)
            .ok_or(SchedulerError::StepContractViolation)?;
        Ok(())
    }

    /// Current exact state of one input cord, for terminal-aware drains.
    pub fn input_state(&self, cord: usize) -> Result<FlowQueueState, SchedulerError> {
        let plan_cord = self
            .plan
            .cords
            .get(cord)
            .ok_or(SchedulerError::PortAccessViolation)?;
        if plan_cord.to_node != self.node {
            return Err(SchedulerError::PortAccessViolation);
        }
        Ok(self.cords[cord].state())
    }

    /// Current exact state of one output cord.
    pub fn output_state(&self, cord: usize) -> Result<FlowQueueState, SchedulerError> {
        let plan_cord = self
            .plan
            .cords
            .get(cord)
            .ok_or(SchedulerError::PortAccessViolation)?;
        if plan_cord.from_node != self.node {
            return Err(SchedulerError::PortAccessViolation);
        }
        Ok(self.cords[cord].state())
    }

    pub fn request_domain_evidence(&mut self) -> Result<(), SchedulerError> {
        self.workspace.domain_evidence = self
            .workspace
            .domain_evidence
            .checked_add(1)
            .ok_or(SchedulerError::StepContractViolation)?;
        Ok(())
    }

    /// Account one bounded representation fragment assembled during this
    /// step. Fragment payload remains in the implementation's exact scratch or
    /// output reservation; the scheduler stores no spill buffer.
    pub fn record_fragment(&mut self) -> Result<(), SchedulerError> {
        let fragments = self
            .workspace
            .fragments
            .checked_add(1)
            .ok_or(SchedulerError::StepContractViolation)?;
        let limit = self.plan.nodes[self.node].limits.max_fragments_per_step;
        if fragments > limit {
            return Err(SchedulerError::StepContractViolation);
        }
        self.workspace.fragments = fragments;
        Ok(())
    }

    fn wait(&mut self, condition: WaitCondition) -> Result<(), SchedulerError> {
        if self
            .workspace
            .interests
            .iter()
            .any(|prior| prior.kind == condition.kind && prior.subject == condition.subject)
        {
            return Err(SchedulerError::StepContractViolation);
        }
        push_bounded(&mut self.workspace.interests, condition)
    }
}

fn push_bounded<T>(values: &mut Vec<T>, value: T) -> Result<(), SchedulerError> {
    if values.len() == values.capacity() {
        return Err(SchedulerError::TransactionCapacityExceeded);
    }
    values.push(value);
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct ReadyEntry {
    node: usize,
    reason: SchedulerDecisionReason,
    ready_tick: u64,
}

struct FixedReadyQueue {
    slots: Box<[Option<ReadyEntry>]>,
    head: usize,
    len: usize,
}

impl FixedReadyQueue {
    fn allocate(nodes: usize) -> Result<Self, SchedulerError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(nodes)
            .map_err(|_| SchedulerError::AllocationFailed)?;
        values.resize(nodes, None);
        Ok(Self {
            slots: values.into_boxed_slice(),
            head: 0,
            len: 0,
        })
    }

    fn push(&mut self, value: ReadyEntry) -> Result<(), SchedulerError> {
        if self.len == self.slots.len() {
            return Err(SchedulerError::TransactionCapacityExceeded);
        }
        let index = (self.head + self.len) % self.slots.len();
        self.slots[index] = Some(value);
        self.len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Option<ReadyEntry> {
        if self.len == 0 {
            return None;
        }
        let value = self.slots[self.head].take();
        self.head = (self.head + 1) % self.slots.len();
        self.len -= 1;
        value
    }

    const fn len(&self) -> usize {
        self.len
    }
}

struct FixedEventLog {
    slots: Box<[Option<SchedulerEvent>]>,
    len: usize,
}

impl FixedEventLog {
    fn allocate(capacity: usize) -> Result<Self, SchedulerError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| SchedulerError::AllocationFailed)?;
        values.resize(capacity, None);
        Ok(Self {
            slots: values.into_boxed_slice(),
            len: 0,
        })
    }

    fn push(&mut self, value: SchedulerEvent) -> Result<(), SchedulerError> {
        let Some(slot) = self.slots.get_mut(self.len) else {
            return Err(SchedulerError::EvidenceCapacityExceeded);
        };
        *slot = Some(value);
        self.len += 1;
        Ok(())
    }

    fn as_slice(&self) -> &[Option<SchedulerEvent>] {
        &self.slots[..self.len]
    }
}

/// Single-threaded deterministic executor and simulated clock.
pub struct DeterministicExecutor<N> {
    runtime: RuntimePlan,
    policy: SchedulerPolicy,
    drivers: Vec<N>,
    machines: Vec<ImplementationMachine>,
    cords: Vec<RuntimeCord>,
    _feedback_reservations: Vec<FeedbackReservation>,
    workspaces: Vec<NodeWorkspace>,
    waits: Vec<Vec<WaitCondition>>,
    ready: FixedReadyQueue,
    enqueued: Vec<bool>,
    yields: Vec<u32>,
    events: FixedEventLog,
    allocation: SchedulerAllocation,
    status: SchedulerStatus,
    tick: u64,
    decisions: u64,
    next_event_sequence: u64,
    max_ready_depth: u32,
    max_cord_occupancy: u16,
    max_queue_items: u64,
    max_queue_payload_bytes: u64,
    cancellation_started: Option<(u64, StopPolicy)>,
}

impl<N: SchedulerNode> DeterministicExecutor<N> {
    /// Validate, preallocate, prepare-all, and start-all atomically.
    pub fn start(
        plan: &ExecutionPlan<'_>,
        validation: conduit_core::PlanValidationContext<'_>,
        policy: SchedulerPolicy,
        reservation: SchedulerReservation,
        nodes: Vec<ScheduledNode<N>>,
    ) -> Result<Self, SchedulerError> {
        policy
            .validate()
            .map_err(|_| SchedulerError::InvalidPolicy)?;
        if policy.ready_queue != ReadyQueueDiscipline::RoundRobin {
            return Err(SchedulerError::InvalidPolicy);
        }
        crate::validate_hosted_execution_plan(plan, validation)
            .map_err(|_| SchedulerError::InvalidPlan)?;
        if nodes.len() != plan.nodes.len() || nodes.is_empty() {
            return Err(SchedulerError::NodeSetMismatch);
        }
        for (node, planned) in nodes.iter().zip(plan.nodes) {
            let Some(profile) = planned.execution_profile else {
                return Err(SchedulerError::InvalidPlan);
            };
            if node.machine.profile().semantic_hash != profile.semantic_hash {
                return Err(SchedulerError::NodeSetMismatch);
            }
        }

        let runtime_storage_bytes = runtime_plan_storage_bytes(plan)?;
        let mut allocation = compute_allocation(plan, policy)?;
        allocation.executor_overhead_bytes = allocation
            .executor_overhead_bytes
            .checked_add(runtime_storage_bytes)
            .ok_or(SchedulerError::ArithmeticOverflow)?;
        if allocation.executor_overhead_bytes > reservation.executor_overhead_limit_bytes {
            return Err(SchedulerError::AllocationUnavailable);
        }
        let total = allocation
            .planned_memory_bytes
            .checked_add(allocation.executor_overhead_bytes)
            .ok_or(SchedulerError::ArithmeticOverflow)?;
        if total > reservation.available_runtime_memory_bytes {
            return Err(SchedulerError::AllocationUnavailable);
        }
        let residual = plan
            .budget
            .memory_bytes
            .checked_sub(allocation.planned_memory_bytes)
            .ok_or(SchedulerError::AllocationExceedsPlan)?;
        if allocation.executor_overhead_bytes > residual {
            return Err(SchedulerError::AllocationExceedsPlan);
        }
        let evidence_residual = plan
            .budget
            .evidence_bytes
            .checked_sub(allocation.planned_evidence_bytes)
            .ok_or(SchedulerError::AllocationExceedsPlan)?;
        if allocation.scheduler_evidence_bytes > evidence_residual {
            return Err(SchedulerError::AllocationExceedsPlan);
        }

        let runtime = RuntimePlan::from_exact(plan)?;
        let mut drivers = Vec::new();
        let mut machines = Vec::new();
        drivers
            .try_reserve_exact(nodes.len())
            .map_err(|_| SchedulerError::AllocationFailed)?;
        machines
            .try_reserve_exact(nodes.len())
            .map_err(|_| SchedulerError::AllocationFailed)?;
        for node in nodes {
            drivers.push(node.driver);
            machines.push(node.machine);
        }
        let mut cords = Vec::new();
        cords
            .try_reserve_exact(runtime.cords.len())
            .map_err(|_| SchedulerError::AllocationFailed)?;
        for cord in &runtime.cords {
            cords.push(RuntimeCord::allocate(cord.flow, cord.queue_memory_bytes)?);
        }
        let mut feedback_reservations = Vec::new();
        feedback_reservations
            .try_reserve_exact(runtime.feedback_boundaries.len())
            .map_err(|_| SchedulerError::AllocationFailed)?;
        for boundary in &runtime.feedback_boundaries {
            let byte_count = usize::try_from(boundary.maximum_retained_bytes)
                .map_err(|_| SchedulerError::AllocationFailed)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(byte_count)
                .map_err(|_| SchedulerError::AllocationFailed)?;
            bytes.resize(byte_count, 0);
            let item_count = usize::from(boundary.maximum_retained_items);
            let mut items = Vec::new();
            items
                .try_reserve_exact(item_count)
                .map_err(|_| SchedulerError::AllocationFailed)?;
            items.resize(item_count, None);
            feedback_reservations.push(FeedbackReservation {
                _bytes: bytes.into_boxed_slice(),
                _items: items.into_boxed_slice(),
            });
        }

        let mut workspaces = Vec::new();
        let mut waits = Vec::new();
        workspaces
            .try_reserve_exact(machines.len())
            .map_err(|_| SchedulerError::AllocationFailed)?;
        waits
            .try_reserve_exact(machines.len())
            .map_err(|_| SchedulerError::AllocationFailed)?;
        for machine in &machines {
            let capacity = interest_capacity(machine.profile().limits)?;
            workspaces.push(NodeWorkspace::allocate(*machine, capacity)?);
            waits.push(try_vec_capacity(capacity)?);
        }
        let mut enqueued = try_vec_capacity(machines.len())?;
        enqueued.resize(machines.len(), false);
        let mut yields = try_vec_capacity(machines.len())?;
        yields.resize(machines.len(), 0);

        let event_capacity =
            usize::try_from(policy.max_events).map_err(|_| SchedulerError::AllocationFailed)?;
        let ready = FixedReadyQueue::allocate(runtime.nodes.len())?;
        let mut executor = Self {
            runtime,
            policy,
            drivers,
            machines,
            cords,
            _feedback_reservations: feedback_reservations,
            workspaces,
            waits,
            ready,
            enqueued,
            yields,
            events: FixedEventLog::allocate(event_capacity)?,
            allocation,
            status: SchedulerStatus::Running,
            tick: 0,
            decisions: 0,
            next_event_sequence: 0,
            max_ready_depth: 0,
            max_cord_occupancy: 0,
            max_queue_items: 0,
            max_queue_payload_bytes: 0,
            cancellation_started: None,
        };
        executor.record(
            SchedulerSubject::Run,
            SchedulerEventKind::AllocationPrepared,
            0,
            0,
        )?;

        let mut prepare_outcomes = try_vec_capacity(executor.machines.len())?;
        let mut prepare_usages = try_vec_capacity(executor.machines.len())?;
        for driver in &mut executor.drivers {
            match driver.prepare() {
                Ok(usage) => {
                    prepare_outcomes.push(PrepareOutcome::Ready);
                    prepare_usages.push(usage);
                }
                Err(code) => {
                    let _ = code;
                    return Err(SchedulerError::PrepareFailed);
                }
            }
        }
        prepare_all(&mut executor.machines, &prepare_outcomes, &prepare_usages)
            .map_err(|_| SchedulerError::PrepareFailed)?;
        for index in 0..executor.machines.len() {
            executor.record(
                SchedulerSubject::Node(as_u16(index)?),
                SchedulerEventKind::NodePrepared,
                0,
                0,
            )?;
        }

        let mut start_usages = try_vec_capacity(executor.machines.len())?;
        for driver in &mut executor.drivers {
            start_usages.push(driver.start().map_err(|_| SchedulerError::StartFailed)?);
        }
        start_all(&mut executor.machines, &start_usages)
            .map_err(|_| SchedulerError::StartFailed)?;
        executor.record(SchedulerSubject::Run, SchedulerEventKind::RunStarted, 0, 0)?;
        for index in 0..executor.machines.len() {
            executor.enqueue(index, SchedulerDecisionReason::Initial)?;
        }
        Ok(executor)
    }

    #[must_use]
    pub const fn allocation(&self) -> SchedulerAllocation {
        self.allocation
    }

    /// Exact aggregate resource budget copied from the admitted plan.
    #[must_use]
    pub const fn plan_budget(&self) -> PlanResourceBudget {
        self.runtime.budget
    }

    #[must_use]
    pub const fn policy(&self) -> SchedulerPolicy {
        self.policy
    }

    #[must_use]
    pub const fn status(&self) -> SchedulerStatus {
        self.status
    }

    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub const fn decisions(&self) -> u64 {
        self.decisions
    }

    #[must_use]
    pub const fn max_ready_depth(&self) -> u32 {
        self.max_ready_depth
    }

    #[must_use]
    pub const fn max_cord_occupancy(&self) -> u16 {
        self.max_cord_occupancy
    }

    #[must_use]
    pub fn high_water(&self) -> SchedulerHighWater {
        SchedulerHighWater {
            queue_items: self.max_queue_items,
            queue_payload_bytes: self.max_queue_payload_bytes,
            ready_slots: self.max_ready_depth,
            event_slots: u32::try_from(self.events.len).unwrap_or(u32::MAX),
            decisions: self.decisions,
        }
    }

    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.len
    }

    /// Fixed hosted value-arena accounting, when the installed drivers expose
    /// one. The value is a measurement, not a semantic input.
    #[must_use]
    pub fn value_storage_usage(&self) -> Option<ValueStorageUsage> {
        self.drivers
            .first()
            .and_then(SchedulerNode::value_storage_usage)
    }

    pub fn events(&self) -> impl Iterator<Item = &SchedulerEvent> {
        self.events.as_slice().iter().flatten()
    }

    pub(crate) fn project_exact_evidence(
        &self,
        plan_identity: &str,
        plan_epoch: u64,
        run_id: &str,
    ) -> Vec<crate::ExactEvidenceRecord> {
        let events = self.events().copied().collect::<Vec<_>>();
        crate::exact_evidence::project_runtime_exact_evidence(
            &self.runtime,
            plan_identity,
            plan_epoch,
            run_id,
            &events,
        )
    }

    #[must_use]
    pub fn cord_occupancy(&self, cord: usize) -> Option<(u16, u64, FlowQueueState)> {
        self.cords.get(cord).map(|value| {
            (
                value.occupancy_items(),
                value.occupancy_bytes(),
                value.state(),
            )
        })
    }

    /// Execute one fair ready-queue decision.
    pub fn run_one(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        if !matches!(
            self.status,
            SchedulerStatus::Running | SchedulerStatus::Stalled
        ) {
            self.reconcile_host_values();
            return Ok(self.status);
        }
        if self
            .policy
            .lifetime_decision_limit()
            .is_some_and(|limit| self.decisions >= limit)
        {
            return self.fail(SchedulerError::DecisionLimitExceeded);
        }
        let Some(entry) = self.ready.pop() else {
            self.status = SchedulerStatus::Stalled;
            self.check_cancellation_deadline()?;
            self.refresh_terminal_status()?;
            self.reconcile_host_values();
            return Ok(self.status);
        };
        self.enqueued[entry.node] = false;
        self.status = SchedulerStatus::Running;
        let scheduling_latency_ticks = self
            .tick
            .checked_sub(entry.ready_tick)
            .ok_or(SchedulerError::ClockLimitExceeded)?;
        self.record_observation(
            SchedulerSubject::Node(as_u16(entry.node)?),
            SchedulerEventKind::Decision {
                reason: entry.reason,
            },
            0,
            0,
            None,
            None,
            scheduling_latency_ticks,
            0,
        )?;
        self.decisions += 1;
        self.tick = self
            .tick
            .checked_add(1)
            .ok_or(SchedulerError::ClockLimitExceeded)?;
        if self.tick > self.policy.max_tick {
            return self.fail(SchedulerError::ClockLimitExceeded);
        }

        let result = self.step_node(entry.node);
        if let Err(error) = result {
            return self.fail(error);
        }
        self.wake_due_timers()?;
        self.check_cancellation_deadline()?;
        self.refresh_terminal_status()?;
        self.reconcile_host_values();
        Ok(self.status)
    }

    /// Run until no node is ready or a terminal condition is reached.
    pub fn run_until_stalled(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        loop {
            let before = self.decisions;
            let status = self.run_one()?;
            if !matches!(status, SchedulerStatus::Running) || self.decisions == before {
                return Ok(status);
            }
        }
    }

    /// Advance the deterministic clock and wake exact timer interests.
    pub fn advance_to(&mut self, tick: u64) -> Result<(), SchedulerError> {
        if tick < self.tick || tick > self.policy.max_tick {
            return Err(SchedulerError::ClockLimitExceeded);
        }
        self.tick = tick;
        self.wake_due_timers()?;
        self.check_cancellation_deadline()
    }

    /// Earliest exact timer deadline currently retained by any node.
    #[must_use]
    pub fn next_timer_deadline(&self) -> Option<u64> {
        self.waits
            .iter()
            .flat_map(|waits| waits.iter())
            .filter(|wait| wait.kind == WakeInterestKind::Timer)
            .filter_map(|wait| wait.deadline_tick)
            .filter(|deadline| *deadline > self.tick)
            .min()
    }

    /// Wake a bounded host operation; callback queues remain outside this API
    /// and must fit the implementation profile.
    pub fn notify_host_operation(&mut self, subject: Id<'static>) -> Result<(), SchedulerError> {
        for index in 0..self.waits.len() {
            let should_wake = self.waits[index].iter().any(|wait| {
                wait.kind == WakeInterestKind::HostOperation
                    && wait.subject == WaitSubject::Named(subject)
            });
            if should_wake {
                self.clear_waits_and_enqueue(index, SchedulerDecisionReason::HostOperationReady)?;
            }
        }
        Ok(())
    }

    /// Request exact drain or abort cancellation and wake all blocked states.
    pub fn cancel(&mut self, stop: StopPolicy) -> Result<(), SchedulerError> {
        if !matches!(
            self.status,
            SchedulerStatus::Running | SchedulerStatus::Stalled
        ) {
            return Ok(());
        }
        self.record(
            SchedulerSubject::Run,
            SchedulerEventKind::CancellationRequested { stop },
            0,
            0,
        )?;
        self.cancellation_started = Some((self.tick, stop));
        for index in 0..self.drivers.len() {
            self.drivers[index].cancel(stop);
            if !matches!(self.machines[index].phase(), InstancePhase::Terminal(_)) {
                self.machines[index]
                    .cancel()
                    .map_err(|_| SchedulerError::StepContractViolation)?;
                self.clear_waits_and_enqueue(index, SchedulerDecisionReason::Cancellation)?;
            }
        }
        for cord in 0..self.cords.len() {
            let events = self.cords[cord].terminate(TerminalClass::Cancelled, stop);
            self.record_cord_events(cord, events)?;
        }
        if stop == StopPolicy::Abort {
            for machine in &mut self.machines {
                if !matches!(machine.phase(), InstancePhase::Terminal(_)) {
                    machine
                        .abort()
                        .map_err(|_| SchedulerError::StepContractViolation)?;
                }
            }
            self.status = SchedulerStatus::Cancelled;
            self.record(
                SchedulerSubject::Run,
                SchedulerEventKind::Terminal(TerminalClass::Cancelled),
                0,
                0,
            )?;
        }
        self.reconcile_host_values();
        Ok(())
    }

    fn step_node(&mut self, node: usize) -> Result<(), SchedulerError> {
        self.workspaces[node].begin_step();
        let step = {
            let mut io = StepIo {
                node,
                tick: self.tick,
                plan: &self.runtime,
                cords: &self.cords,
                workspace: &mut self.workspaces[node],
            };
            self.drivers[node].step(&mut io)
        };

        self.apply_probes(node)?;
        let commit = matches!(step, SchedulerStep::Progress | SchedulerStep::Completed);
        if commit {
            self.validate_coupled_publication(node)?;
        }
        let usage = self.step_usage(node, commit)?;
        let observation = if step == SchedulerStep::Pending {
            self.validate_runtime_waits(node)?;
            self.machines[node]
                .observe_pending_validated(self.workspaces[node].interests.len(), usage)
                .map_err(map_implementation_error)?
        } else {
            let outcome = match step {
                SchedulerStep::Progress => StepOutcome::Progress,
                SchedulerStep::Pending => unreachable!("pending is handled above"),
                SchedulerStep::Yielded => StepOutcome::Yielded,
                SchedulerStep::Completed => StepOutcome::Completed,
                SchedulerStep::Failed { code } => StepOutcome::Failed { code },
            };
            self.machines[node]
                .observe_step(outcome, usage)
                .map_err(map_implementation_error)?
        };
        self.record_observation(
            SchedulerSubject::Node(as_u16(node)?),
            SchedulerEventKind::NodeOutcome {
                outcome: observation.outcome(),
            },
            0,
            0,
            None,
            None,
            0,
            1,
        )?;

        if commit {
            self.commit_transaction(node)?;
        }
        match step {
            SchedulerStep::Progress => {
                self.yields[node] = 0;
                self.enqueue(node, SchedulerDecisionReason::Progress)?;
            }
            SchedulerStep::Pending => {
                self.yields[node] = 0;
                self.waits[node].clear();
                std::mem::swap(&mut self.waits[node], &mut self.workspaces[node].interests);
            }
            SchedulerStep::Yielded => {
                self.yields[node] = self.yields[node]
                    .checked_add(1)
                    .ok_or(SchedulerError::ZeroProgressLivelock)?;
                if self.yields[node] > self.policy.max_consecutive_yields {
                    return Err(SchedulerError::ZeroProgressLivelock);
                }
                self.enqueue(node, SchedulerDecisionReason::FairYield)?;
            }
            SchedulerStep::Completed => {
                self.yields[node] = 0;
                self.complete_outputs(node)?;
            }
            SchedulerStep::Failed { .. } => {
                return Err(SchedulerError::NodeFailed);
            }
        }
        Ok(())
    }

    fn validate_runtime_waits(&self, node: usize) -> Result<(), SchedulerError> {
        for wait in &self.workspaces[node].interests {
            match wait.subject {
                WaitSubject::Cord(index) => {
                    if self.runtime.cords.get(index).is_none() {
                        return Err(SchedulerError::StepContractViolation);
                    }
                }
                WaitSubject::Named(id) if Id::new(id.as_str()).is_err() => {
                    return Err(SchedulerError::StepContractViolation);
                }
                WaitSubject::Named(_) => {}
            }
        }
        Ok(())
    }

    fn step_usage(&self, node: usize, commit: bool) -> Result<StepUsage, SchedulerError> {
        let workspace = &self.workspaces[node];
        let input_bytes = workspace.inputs.iter().try_fold(0_u64, |total, input| {
            total.checked_add(u64::from(input.value.accounted_bytes))
        });
        let output_bytes = workspace.outputs.iter().try_fold(0_u64, |total, output| {
            total.checked_add(u64::from(output.value.accounted_bytes))
        });
        let operations = if commit {
            workspace
                .inputs
                .len()
                .checked_add(workspace.outputs.len())
                .and_then(|value| value.checked_add(usize::from(workspace.host_operations)))
                .ok_or(SchedulerError::ArithmeticOverflow)?
        } else {
            0
        };
        Ok(StepUsage {
            work_units: workspace.work_units,
            observable_operations: u16::try_from(operations)
                .map_err(|_| SchedulerError::StepContractViolation)?,
            committed_transactions: u16::from(
                commit && (!workspace.inputs.is_empty() || !workspace.outputs.is_empty()),
            ),
            input_leases: if commit {
                u16::try_from(workspace.inputs.len())
                    .map_err(|_| SchedulerError::StepContractViolation)?
            } else {
                0
            },
            input_bytes: if commit {
                input_bytes.ok_or(SchedulerError::ArithmeticOverflow)?
            } else {
                0
            },
            output_reservations: if commit {
                u16::try_from(workspace.outputs.len())
                    .map_err(|_| SchedulerError::StepContractViolation)?
            } else {
                0
            },
            output_bytes: if commit {
                output_bytes.ok_or(SchedulerError::ArithmeticOverflow)?
            } else {
                0
            },
            domain_evidence: workspace.domain_evidence,
            fragments: workspace.fragments,
            ..StepUsage::default()
        })
    }

    fn validate_coupled_publication(&self, node: usize) -> Result<(), SchedulerError> {
        for fanout in self
            .runtime
            .fanouts
            .iter()
            .filter(|fanout| fanout.mode == FanOutMode::Coupled && fanout.producer_node == node)
        {
            let mut first = None;
            let mut published = 0_usize;
            for &branch in &fanout.branches {
                if let Some(output) = self.workspaces[node]
                    .outputs
                    .iter()
                    .find(|output| output.cord == branch)
                {
                    if output.expected != SendStatus::Reserved {
                        return Err(SchedulerError::StepContractViolation);
                    }
                    if fanout.shared_handle {
                        if first.is_some_and(|value| value != output.value) {
                            return Err(SchedulerError::StepContractViolation);
                        }
                        first = Some(output.value);
                    }
                    published += 1;
                }
            }
            if published != 0 && published != fanout.branches.len() {
                return Err(SchedulerError::StepContractViolation);
            }
        }
        Ok(())
    }

    fn apply_probes(&mut self, node: usize) -> Result<(), SchedulerError> {
        for probe_index in 0..self.workspaces[node].probes.len() {
            let probe = self.workspaces[node].probes[probe_index];
            match probe {
                QueueProbe::ProducerBlocked(cord) => {
                    let events = self.cords[cord].mark_producer_waiting();
                    self.record_cord_events(cord, events)?;
                }
                QueueProbe::ConsumerEmpty(cord) => self.cords[cord].mark_consumer_waiting(),
            }
        }
        Ok(())
    }

    fn commit_transaction(&mut self, node: usize) -> Result<(), SchedulerError> {
        for input_index in 0..self.workspaces[node].inputs.len() {
            let input = self.workspaces[node].inputs[input_index];
            let (value, events) = self.cords[input.cord].pop();
            if value != Some(input.value) {
                return Err(SchedulerError::StepContractViolation);
            }
            self.record_cord_events(input.cord, events)?;
            self.observe_queue_high_water()?;
            self.wake_for_cord(input.cord)?;
        }
        for output_index in 0..self.workspaces[node].outputs.len() {
            let output = self.workspaces[node].outputs[output_index];
            let (disposition, events) =
                self.cords[output.cord].offer(output.value, output.coalesce_target);
            self.record_cord_events(output.cord, events)?;
            let committed_publication = matches!(
                disposition,
                OfferDisposition::Enqueued | OfferDisposition::Coalesced { .. }
            );
            let actual = match &disposition {
                OfferDisposition::Enqueued | OfferDisposition::Coalesced { .. } => {
                    SendStatus::Reserved
                }
                OfferDisposition::Pending(_) => SendStatus::WouldBlock,
                OfferDisposition::Rejected(_) => SendStatus::Rejected,
                OfferDisposition::Dropped(_) => SendStatus::Dropped,
                OfferDisposition::Disconnected(_) => SendStatus::Disconnected,
                OfferDisposition::Failed(_) => SendStatus::Failed,
                OfferDisposition::Terminated(_) => SendStatus::Terminated,
            };
            if actual != output.expected {
                return Err(SchedulerError::StepContractViolation);
            }
            match disposition {
                OfferDisposition::Pending(_) => {
                    return Err(SchedulerError::StepContractViolation);
                }
                OfferDisposition::Failed(_) => return Err(SchedulerError::QueueFailed),
                OfferDisposition::Disconnected(_) => {
                    self.status = SchedulerStatus::Disconnected;
                }
                OfferDisposition::Enqueued
                | OfferDisposition::Rejected(_)
                | OfferDisposition::Coalesced { .. }
                | OfferDisposition::Dropped(_)
                | OfferDisposition::Terminated(_) => {}
            }
            if committed_publication {
                for input_index in 0..self.workspaces[node].inputs.len() {
                    let input = self.workspaces[node].inputs[input_index];
                    self.record_observation(
                        SchedulerSubject::Node(as_u16(node)?),
                        SchedulerEventKind::DerivationCommitted,
                        0,
                        0,
                        Some(output.value.handle),
                        Some(input.value.handle),
                        0,
                        0,
                    )?;
                }
            }
            self.max_cord_occupancy = self
                .max_cord_occupancy
                .max(self.cords[output.cord].occupancy_items());
            self.observe_queue_high_water()?;
            self.wake_for_cord(output.cord)?;
        }
        Ok(())
    }

    fn observe_queue_high_water(&mut self) -> Result<(), SchedulerError> {
        let (items, bytes) = self
            .cords
            .iter()
            .try_fold((0_u64, 0_u64), |(items, bytes), cord| {
                Some((
                    items.checked_add(u64::from(cord.occupancy_items()))?,
                    bytes.checked_add(cord.occupancy_bytes())?,
                ))
            })
            .ok_or(SchedulerError::ArithmeticOverflow)?;
        self.max_queue_items = self.max_queue_items.max(items);
        self.max_queue_payload_bytes = self.max_queue_payload_bytes.max(bytes);
        Ok(())
    }

    fn complete_outputs(&mut self, node: usize) -> Result<(), SchedulerError> {
        for cord in 0..self.runtime.cords.len() {
            if self.runtime.cords[cord].from_node == node {
                let events = self.cords[cord].complete_source();
                self.record_cord_events(cord, events)?;
                self.wake_for_cord(cord)?;
            }
        }
        Ok(())
    }

    fn wake_for_cord(&mut self, cord: usize) -> Result<(), SchedulerError> {
        let input_ready = self.cords[cord].occupancy_items() > 0
            || self.cords[cord].state() != FlowQueueState::Active;
        let output_ready = self.cords[cord].state() != FlowQueueState::Active
            || self.cords[cord].occupancy_items() < self.cords[cord].policy.capacity_items;
        for index in 0..self.waits.len() {
            let reason = self.waits[index].iter().find_map(|wait| {
                if wait.subject != WaitSubject::Cord(cord) {
                    return None;
                }
                match wait.kind {
                    WakeInterestKind::Input if input_ready => {
                        Some(SchedulerDecisionReason::InputReady)
                    }
                    WakeInterestKind::Output if output_ready => {
                        Some(SchedulerDecisionReason::OutputReady)
                    }
                    _ => None,
                }
            });
            if let Some(reason) = reason {
                self.clear_waits_and_enqueue(index, reason)?;
            }
        }
        Ok(())
    }

    fn wake_due_timers(&mut self) -> Result<(), SchedulerError> {
        for index in 0..self.waits.len() {
            let should_wake = self.waits[index].iter().any(|wait| {
                wait.kind == WakeInterestKind::Timer
                    && wait
                        .deadline_tick
                        .is_some_and(|deadline| deadline <= self.tick)
            });
            if should_wake {
                self.clear_waits_and_enqueue(index, SchedulerDecisionReason::TimerReady)?;
            }
        }
        Ok(())
    }

    fn clear_waits_and_enqueue(
        &mut self,
        node: usize,
        reason: SchedulerDecisionReason,
    ) -> Result<(), SchedulerError> {
        self.waits[node].clear();
        self.enqueue(node, reason)
    }

    fn enqueue(
        &mut self,
        node: usize,
        reason: SchedulerDecisionReason,
    ) -> Result<(), SchedulerError> {
        if self.enqueued[node] || matches!(self.machines[node].phase(), InstancePhase::Terminal(_))
        {
            return Ok(());
        }
        self.ready.push(ReadyEntry {
            node,
            reason,
            ready_tick: self.tick,
        })?;
        self.enqueued[node] = true;
        if self.status == SchedulerStatus::Stalled {
            self.status = SchedulerStatus::Running;
        }
        self.max_ready_depth = self
            .max_ready_depth
            .max(u32::try_from(self.ready.len()).map_err(|_| SchedulerError::ArithmeticOverflow)?);
        self.record(
            SchedulerSubject::Node(as_u16(node)?),
            SchedulerEventKind::NodeWoken { reason },
            0,
            0,
        )
    }

    fn record_cord_events(
        &mut self,
        cord: usize,
        events: CordEventBatch,
    ) -> Result<(), SchedulerError> {
        for event in events.iter() {
            self.record_observation(
                SchedulerSubject::Cord(as_u16(cord)?),
                event.kind,
                event.occupancy_items,
                event.occupancy_bytes,
                event.value_handle,
                event.related_value_handle,
                0,
                0,
            )?;
        }
        Ok(())
    }

    fn record(
        &mut self,
        subject: SchedulerSubject,
        kind: SchedulerEventKind,
        occupancy_items: u16,
        occupancy_bytes: u64,
    ) -> Result<(), SchedulerError> {
        self.record_observation(
            subject,
            kind,
            occupancy_items,
            occupancy_bytes,
            None,
            None,
            0,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record_observation(
        &mut self,
        subject: SchedulerSubject,
        kind: SchedulerEventKind,
        occupancy_items: u16,
        occupancy_bytes: u64,
        value_handle: Option<u64>,
        related_value_handle: Option<u64>,
        scheduling_latency_ticks: u64,
        processing_latency_ticks: u64,
    ) -> Result<(), SchedulerError> {
        let sequence = self.next_event_sequence;
        self.next_event_sequence = sequence
            .checked_add(1)
            .ok_or(SchedulerError::EvidenceCapacityExceeded)?;
        self.events.push(SchedulerEvent {
            sequence,
            tick: self.tick,
            subject,
            kind,
            occupancy_items,
            occupancy_bytes,
            value_handle,
            related_value_handle,
            scheduling_latency_ticks,
            processing_latency_ticks,
        })
    }

    fn check_cancellation_deadline(&mut self) -> Result<(), SchedulerError> {
        let Some((started, _)) = self.cancellation_started else {
            return Ok(());
        };
        for machine in &self.machines {
            if !matches!(machine.phase(), InstancePhase::Terminal(_)) {
                let deadline = started
                    .checked_add(machine.profile().limits.cancellation_ticks)
                    .ok_or(SchedulerError::CancellationDeadlineExceeded)?;
                if self.tick > deadline {
                    return self
                        .fail(SchedulerError::CancellationDeadlineExceeded)
                        .map(|_| ());
                }
            }
        }
        Ok(())
    }

    fn refresh_terminal_status(&mut self) -> Result<(), SchedulerError> {
        if matches!(
            self.status,
            SchedulerStatus::Failed(_)
                | SchedulerStatus::Disconnected
                | SchedulerStatus::Cancelled
                | SchedulerStatus::Succeeded
        ) {
            return Ok(());
        }
        if self
            .machines
            .iter()
            .all(|machine| matches!(machine.phase(), InstancePhase::Terminal(_)))
        {
            let terminal =
                if self.machines.iter().any(|machine| {
                    machine.phase() == InstancePhase::Terminal(TerminalClass::Failed)
                }) {
                    TerminalClass::Failed
                } else if self.machines.iter().any(|machine| {
                    machine.phase() == InstancePhase::Terminal(TerminalClass::Cancelled)
                }) {
                    TerminalClass::Cancelled
                } else {
                    TerminalClass::Succeeded
                };
            self.status = match terminal {
                TerminalClass::Succeeded => SchedulerStatus::Succeeded,
                TerminalClass::Cancelled => SchedulerStatus::Cancelled,
                TerminalClass::Disconnected => SchedulerStatus::Disconnected,
                TerminalClass::Failed => SchedulerStatus::Failed(SchedulerError::NodeFailed),
            };
            self.record(
                SchedulerSubject::Run,
                SchedulerEventKind::Terminal(terminal),
                0,
                0,
            )?;
        } else if self.ready.len() == 0 {
            self.status = SchedulerStatus::Stalled;
        }
        Ok(())
    }

    fn reconcile_host_values(&mut self) {
        if self.drivers.is_empty() {
            return;
        }
        self.drivers[0].begin_value_reconciliation();
        for cord in &self.cords {
            cord.visit_values(|value| self.drivers[0].mark_value_live(value));
        }
        for node in 0..self.drivers.len() {
            if !matches!(self.machines[node].phase(), InstancePhase::Terminal(_)) {
                self.drivers[node].mark_retained_values();
            }
        }
        self.drivers[0].finish_value_reconciliation();
    }

    fn fail(&mut self, error: SchedulerError) -> Result<SchedulerStatus, SchedulerError> {
        self.status = SchedulerStatus::Failed(error);
        for cord in 0..self.cords.len() {
            let events = self.cords[cord].terminate(TerminalClass::Failed, StopPolicy::Abort);
            // If evidence is already full, preserve the original scheduler
            // error rather than allocating or replacing evidence.
            let _ = self.record_cord_events(cord, events);
        }
        for machine in &mut self.machines {
            if !matches!(machine.phase(), InstancePhase::Terminal(_)) {
                let _ = machine.abort();
            }
        }
        let _ = self.record(
            SchedulerSubject::Run,
            SchedulerEventKind::Terminal(TerminalClass::Failed),
            0,
            0,
        );
        self.reconcile_host_values();
        Err(error)
    }
}

fn map_implementation_error(error: ImplementationError) -> SchedulerError {
    match error {
        ImplementationError::StepBoundExceeded
        | ImplementationError::FalseProgress
        | ImplementationError::UnqualifiedPending
        | ImplementationError::TransactionViolation
        | ImplementationError::IllegalLifecycle => SchedulerError::StepContractViolation,
        _ => SchedulerError::StepContractViolation,
    }
}

fn compute_allocation(
    plan: &ExecutionPlan<'_>,
    policy: SchedulerPolicy,
) -> Result<SchedulerAllocation, SchedulerError> {
    let node_memory_bytes = sum_memory(plan.nodes.iter().map(|node| node.allocation.memory_bytes))?;
    let cord_memory_bytes = sum_memory(plan.cords.iter().map(|cord| cord.queue_memory_bytes))?;
    let feedback_memory_bytes = sum_memory(
        plan.feedback_boundaries
            .iter()
            .map(|boundary| boundary.maximum_retained_bytes),
    )?;
    let pool_memory_bytes = sum_memory(
        plan.instance_pools
            .iter()
            .map(|pool| pool.worst_case_budget.memory_bytes),
    )?;
    let event_stream_memory_bytes = sum_memory(
        plan.event_streams
            .iter()
            .map(|stream| stream.allocation.memory_bytes),
    )?;
    let job_memory_bytes = sum_memory(plan.jobs.iter().map(|job| job.allocation.memory_bytes))?;
    let planned_memory_bytes = [
        node_memory_bytes,
        cord_memory_bytes,
        feedback_memory_bytes,
        pool_memory_bytes,
        event_stream_memory_bytes,
        job_memory_bytes,
    ]
    .into_iter()
    .try_fold(0_u64, u64::checked_add)
    .ok_or(SchedulerError::ArithmeticOverflow)?;
    let planned_evidence_bytes = planned_evidence(plan)?;
    let queue_payload_bytes = plan.cords.iter().try_fold(0_u64, |total, cord| {
        total.checked_add(cord.queue_memory_bytes)
    });
    let queue_slots = plan
        .cords
        .iter()
        .try_fold(0_u64, |total, cord| {
            total.checked_add(u64::from(cord.flow.capacity.items()))
        })
        .ok_or(SchedulerError::ArithmeticOverflow)?;
    let feedback_slots = plan
        .feedback_boundaries
        .iter()
        .try_fold(0_u64, |total, boundary| {
            total.checked_add(u64::from(boundary.maximum_retained_items))
        })
        .ok_or(SchedulerError::ArithmeticOverflow)?;
    let wake_interest_slots = plan.nodes.iter().try_fold(0_u64, |total, node| {
        let profile = node.execution_profile.ok_or(SchedulerError::InvalidPlan)?;
        total
            .checked_add(
                u64::try_from(interest_capacity(profile.limits)?)
                    .map_err(|_| SchedulerError::ArithmeticOverflow)?,
            )
            .ok_or(SchedulerError::ArithmeticOverflow)
    })?;
    let transaction_slots = plan.nodes.iter().try_fold(0_u64, |total, node| {
        let limits = node
            .execution_profile
            .ok_or(SchedulerError::InvalidPlan)?
            .limits;
        total
            .checked_add(u64::from(limits.max_input_leases))
            .and_then(|value| value.checked_add(u64::from(limits.max_output_reservations)))
            .ok_or(SchedulerError::ArithmeticOverflow)
    })?;
    let ready_slots =
        u32::try_from(plan.nodes.len()).map_err(|_| SchedulerError::ArithmeticOverflow)?;
    let event_slots = policy.max_events;

    let queue_metadata = checked_size(queue_slots, size_of::<Option<QueuedValue>>())?;
    let feedback_metadata = checked_size(feedback_slots, size_of::<Option<RuntimeValue>>())?;
    let ready_metadata = checked_size(u64::from(ready_slots), size_of::<Option<ReadyEntry>>())?;
    let wake_metadata = checked_size(
        wake_interest_slots
            .checked_mul(3)
            .ok_or(SchedulerError::ArithmeticOverflow)?,
        size_of::<WaitCondition>(),
    )?;
    let transaction_metadata = checked_size(
        transaction_slots
            .checked_mul(2)
            .ok_or(SchedulerError::ArithmeticOverflow)?,
        size_of::<StagedInput>().max(size_of::<StagedOutput>()),
    )?;
    let event_metadata = checked_size(u64::from(event_slots), size_of::<Option<SchedulerEvent>>())?;
    let node_metadata = checked_size(
        u64::from(ready_slots),
        size_of::<ImplementationMachine>()
            + size_of::<NodeWorkspace>()
            + size_of::<Vec<WaitCondition>>()
            + size_of::<bool>()
            + size_of::<u32>(),
    )?;
    let cord_metadata = checked_size(
        u64::try_from(plan.cords.len()).map_err(|_| SchedulerError::ArithmeticOverflow)?,
        size_of::<RuntimeCord>(),
    )?;
    let startup_scratch = checked_size(
        u64::from(ready_slots),
        size_of::<PrepareOutcome<'_>>() + 2 * size_of::<LifecycleUsage>(),
    )?;
    let executor_overhead_bytes = [
        queue_metadata,
        feedback_metadata,
        ready_metadata,
        wake_metadata,
        transaction_metadata,
        event_metadata,
        node_metadata,
        cord_metadata,
        startup_scratch,
    ]
    .into_iter()
    .try_fold(0_u64, |total, value| total.checked_add(value))
    .ok_or(SchedulerError::ArithmeticOverflow)?;

    Ok(SchedulerAllocation {
        node_memory_bytes,
        cord_memory_bytes,
        feedback_memory_bytes,
        pool_memory_bytes,
        event_stream_memory_bytes,
        job_memory_bytes,
        planned_memory_bytes,
        planned_evidence_bytes,
        queue_payload_bytes: queue_payload_bytes.ok_or(SchedulerError::ArithmeticOverflow)?,
        executor_overhead_bytes,
        scheduler_evidence_bytes: event_metadata,
        queue_slots,
        ready_slots,
        wake_interest_slots,
        transaction_slots,
        event_slots,
    })
}

fn sum_memory(mut values: impl Iterator<Item = u64>) -> Result<u64, SchedulerError> {
    values
        .try_fold(0_u64, u64::checked_add)
        .ok_or(SchedulerError::ArithmeticOverflow)
}

fn planned_evidence(plan: &ExecutionPlan<'_>) -> Result<u64, SchedulerError> {
    let nodes = plan.nodes.iter().try_fold(0_u64, |total, node| {
        total.checked_add(node.allocation.evidence_bytes)
    });
    let streams = plan.event_streams.iter().try_fold(0_u64, |total, stream| {
        total.checked_add(stream.allocation.evidence_bytes)
    });
    let jobs = plan.jobs.iter().try_fold(0_u64, |total, job| {
        total.checked_add(job.allocation.evidence_bytes)
    });
    let pools = plan.instance_pools.iter().try_fold(0_u64, |total, pool| {
        total.checked_add(pool.worst_case_budget.evidence_bytes)
    });
    nodes
        .and_then(|value| value.checked_add(streams?))
        .and_then(|value| value.checked_add(jobs?))
        .and_then(|value| value.checked_add(pools?))
        .ok_or(SchedulerError::ArithmeticOverflow)
}

fn checked_size(count: u64, item_size: usize) -> Result<u64, SchedulerError> {
    count
        .checked_mul(u64::try_from(item_size).map_err(|_| SchedulerError::ArithmeticOverflow)?)
        .ok_or(SchedulerError::ArithmeticOverflow)
}

fn interest_capacity(limits: conduit_core::ExecutionLimits) -> Result<usize, SchedulerError> {
    usize::from(limits.max_input_leases)
        .checked_add(usize::from(limits.max_output_reservations))
        .and_then(|value| value.checked_add(usize::from(limits.max_pending_operations)))
        .and_then(|value| value.checked_add(usize::from(limits.max_timers)))
        .and_then(|value| value.checked_add(1))
        .ok_or(SchedulerError::ArithmeticOverflow)
}

fn as_u16(value: usize) -> Result<u16, SchedulerError> {
    u16::try_from(value).map_err(|_| SchedulerError::ArithmeticOverflow)
}

#[allow(dead_code)]
fn _budget_marker(_: PlanResourceBudget, _: StepObservation) {}

#[cfg(test)]
mod tests {
    use conduit_core::{
        BlockingFairness, BoundedFlowQueue, FlowCapacity, FlowOffer, FlowTypeFacts, FlowWatermarks,
        SampleSchedule, TraitProof,
    };

    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum DispositionKind {
        Enqueued,
        Pending,
        Rejected,
        Coalesced,
        Dropped,
        Disconnected,
        Failed,
        Terminated,
    }

    fn core_kind(value: &OfferDisposition<RuntimeValue>) -> DispositionKind {
        match value {
            OfferDisposition::Enqueued => DispositionKind::Enqueued,
            OfferDisposition::Pending(_) => DispositionKind::Pending,
            OfferDisposition::Rejected(_) => DispositionKind::Rejected,
            OfferDisposition::Coalesced { .. } => DispositionKind::Coalesced,
            OfferDisposition::Dropped(_) => DispositionKind::Dropped,
            OfferDisposition::Disconnected(_) => DispositionKind::Disconnected,
            OfferDisposition::Failed(_) => DispositionKind::Failed,
            OfferDisposition::Terminated(_) => DispositionKind::Terminated,
        }
    }

    #[test]
    fn hosted_cord_offer_traces_match_allocator_free_reference() {
        let capacity = FlowCapacity::new(1, 8, 8).unwrap();
        let watermarks = FlowWatermarks::new(0, 1, capacity).unwrap();
        let policies = [
            FlowPolicy::new(
                capacity,
                Pressure::Block(BlockingFairness::Fifo),
                watermarks,
            )
            .unwrap(),
            FlowPolicy::new(capacity, Pressure::Reject, watermarks).unwrap(),
            FlowPolicy::new(
                capacity,
                Pressure::Coalesce {
                    relation: Id("merge"),
                },
                watermarks,
            )
            .unwrap(),
            FlowPolicy::new(
                capacity,
                Pressure::Sample(SampleSchedule::new(2, 0).unwrap()),
                watermarks,
            )
            .unwrap(),
            FlowPolicy::new(capacity, Pressure::DropDisposable, watermarks).unwrap(),
            FlowPolicy::new(capacity, Pressure::Disconnect, watermarks).unwrap(),
            FlowPolicy::new(capacity, Pressure::Fail, watermarks).unwrap(),
        ];
        let coalescers = [Id("merge")];
        let facts = FlowTypeFacts {
            disposable: TraitProof::Proven,
            coalescers: Some(&coalescers),
        };

        for policy in policies {
            let mut slots = [None];
            let mut core = BoundedFlowQueue::new(&mut slots, policy, facts).unwrap();
            let mut hosted =
                RuntimeCord::allocate(RuntimeFlowPolicy::from_plan(policy), 8).unwrap();
            for (sequence, target) in [(0_u64, None), (1, Some(0))] {
                let value = RuntimeValue {
                    handle: sequence,
                    accounted_bytes: 8,
                    envelope: RuntimeValueEnvelope::EMPTY,
                };
                let core_transition = core.offer(
                    value,
                    FlowOffer {
                        size_bytes: 8,
                        coalesce_target: target,
                    },
                );
                let (hosted_disposition, hosted_events) = hosted.offer(value, target);
                assert_eq!(
                    core_kind(&core_transition.disposition),
                    core_kind(&hosted_disposition),
                    "policy {} arrival {sequence}",
                    policy.pressure.as_str()
                );
                assert_eq!(
                    core_transition
                        .events
                        .iter()
                        .map(|event| event.kind)
                        .collect::<Vec<_>>(),
                    hosted_events
                        .iter()
                        .filter_map(|event| match event.kind {
                            SchedulerEventKind::Cord(kind) => Some(kind),
                            SchedulerEventKind::ValueAccepted
                            | SchedulerEventKind::ValueConsumed => None,
                            _ => panic!("unexpected non-cord queue event"),
                        })
                        .collect::<Vec<_>>(),
                    "policy {} arrival {sequence}",
                    policy.pressure.as_str()
                );
                assert_eq!(core.occupancy_items(), hosted.occupancy_items());
                assert_eq!(core.occupancy_bytes(), hosted.occupancy_bytes());
                assert_eq!(core.state(), hosted.state());
            }
        }
    }
}
