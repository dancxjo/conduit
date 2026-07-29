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
    SchedulerPolicy, StepObservation, StepOutcome, StepOutcomeKind, StepUsage, StopPolicy,
    TerminalClass, WakeInterest, WakeInterestKind, prepare_all, start_all,
};

/// Opaque executor-mediated value. Payload ownership remains in the exact
/// representation binding; the cord charges `accounted_bytes` against its
/// plan-reserved byte arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeValue {
    pub handle: u64,
    pub accounted_bytes: u32,
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
    fn step(&mut self, io: &mut StepIo<'_, '_>) -> SchedulerStep;

    fn cancel(&mut self, _stop: StopPolicy) {}
}

/// One already-instantiated driver and its portable implementation validator.
pub struct ScheduledNode<'p, N> {
    pub driver: N,
    pub machine: ImplementationMachine<'p>,
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

struct RuntimeCord<'p> {
    policy: FlowPolicy<'p>,
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

#[derive(Clone, Copy, Debug)]
struct CordEventBatch {
    values: [Option<CordEvent>; 3],
    len: u8,
}

#[derive(Clone, Copy, Debug)]
struct CordEvent {
    kind: FlowEventKind,
    occupancy_items: u16,
    occupancy_bytes: u64,
}

impl CordEventBatch {
    const fn new() -> Self {
        Self {
            values: [None, None, None],
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

impl<'p> RuntimeCord<'p> {
    fn allocate(policy: FlowPolicy<'p>, queue_memory_bytes: u64) -> Result<Self, SchedulerError> {
        let slot_count = usize::from(policy.capacity.items());
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

    fn size_at(&self, offset: u16) -> Option<u32> {
        if offset >= self.len {
            return None;
        }
        let slot = (self.head + usize::from(offset)) % self.slots.len();
        self.slots[slot].map(|entry| entry.bytes)
    }

    fn can_fit(&self, value: RuntimeValue, extra_items: u16, extra_bytes: u64) -> bool {
        value.accounted_bytes <= self.policy.capacity.max_value_bytes()
            && self
                .len
                .checked_add(extra_items)
                .is_some_and(|items| items < self.policy.capacity.items())
            && self
                .queued_bytes
                .checked_add(extra_bytes)
                .and_then(|bytes| bytes.checked_add(u64::from(value.accounted_bytes)))
                .is_some_and(|bytes| bytes <= self.policy.capacity.max_queued_bytes())
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
        if let Pressure::Sample(schedule) = self.policy.pressure {
            if arrival % u64::from(schedule.every()) != u64::from(schedule.offset()) {
                self.emit(&mut events, FlowEventKind::ValueSampledOut);
                return (OfferDisposition::Dropped(value), events);
            }
        }
        if value.accounted_bytes > self.policy.capacity.max_value_bytes() {
            self.emit(&mut events, FlowEventKind::ValueRejected);
            return (OfferDisposition::Rejected(value), events);
        }
        let fits = self.len < self.policy.capacity.items()
            && self
                .queued_bytes
                .checked_add(u64::from(value.accounted_bytes))
                .is_some_and(|bytes| bytes <= self.policy.capacity.max_queued_bytes());
        if fits {
            self.push_back(value);
            if self.len >= self.policy.watermarks.high_items() && !self.pressured {
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
            Pressure::Block(_) => {
                self.producer_waiting = true;
                OfferDisposition::Pending(value)
            }
            Pressure::Reject => {
                self.emit(&mut events, FlowEventKind::ValueRejected);
                OfferDisposition::Rejected(value)
            }
            Pressure::Coalesce { .. } => {
                let Some(target) = coalesce_target else {
                    self.emit(&mut events, FlowEventKind::ValueRejected);
                    return (OfferDisposition::Rejected(value), events);
                };
                if target >= self.len {
                    self.emit(&mut events, FlowEventKind::ValueRejected);
                    return (OfferDisposition::Rejected(value), events);
                }
                let slot = (self.head + usize::from(target)) % self.slots.len();
                let old = self.slots[slot].expect("coalescing target is occupied");
                let new_bytes =
                    self.queued_bytes - u64::from(old.bytes) + u64::from(value.accounted_bytes);
                if new_bytes > self.policy.capacity.max_queued_bytes() {
                    self.emit(&mut events, FlowEventKind::ValueRejected);
                    return (OfferDisposition::Rejected(value), events);
                }
                self.slots[slot] = Some(QueuedValue {
                    value,
                    bytes: value.accounted_bytes,
                });
                self.queued_bytes = new_bytes;
                self.emit(&mut events, FlowEventKind::ValueCoalesced { target });
                OfferDisposition::Coalesced {
                    replaced: old.value,
                }
            }
            Pressure::Sample(_) => {
                self.emit(&mut events, FlowEventKind::ValueSampledOut);
                OfferDisposition::Dropped(value)
            }
            Pressure::DropDisposable => {
                self.emit(&mut events, FlowEventKind::ValueDroppedDisposable);
                OfferDisposition::Dropped(value)
            }
            Pressure::Disconnect => {
                self.state = FlowQueueState::Disconnected;
                self.emit(&mut events, FlowEventKind::Disconnected);
                OfferDisposition::Disconnected(value)
            }
            Pressure::Fail => {
                self.state = FlowQueueState::Failed;
                self.emit(&mut events, FlowEventKind::Failed);
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
        if self.pressured && self.len <= self.policy.watermarks.low_items() {
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
        let _sequence = self.flow_sequence;
        self.flow_sequence = self.flow_sequence.wrapping_add(1);
        events.push(CordEvent {
            kind,
            occupancy_items: self.len,
            occupancy_bytes: self.queued_bytes,
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

#[derive(Clone, Copy, Debug)]
struct WaitCondition<'p> {
    interest: WakeInterest<'p>,
    deadline_tick: Option<u64>,
}

struct NodeWorkspace<'p> {
    inputs: Vec<StagedInput>,
    outputs: Vec<StagedOutput>,
    probes: Vec<QueueProbe>,
    interests: Vec<WaitCondition<'p>>,
    observed_interests: Vec<WakeInterest<'p>>,
    work_units: u32,
    host_operations: u16,
    domain_evidence: u16,
    fragments: u16,
}

impl<'p> NodeWorkspace<'p> {
    fn allocate(
        machine: ImplementationMachine<'p>,
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
            observed_interests: try_vec_capacity(interest_capacity)?,
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
        self.observed_interests.clear();
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
pub struct StepIo<'a, 'p> {
    node: usize,
    tick: u64,
    plan: &'p ExecutionPlan<'p>,
    cords: &'a [RuntimeCord<'p>],
    workspace: &'a mut NodeWorkspace<'p>,
}

impl<'p> StepIo<'_, 'p> {
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub fn consume_work(&mut self, units: u32) -> Result<(), SchedulerError> {
        let next = self
            .workspace
            .work_units
            .checked_add(units)
            .ok_or(SchedulerError::StepContractViolation)?;
        if next
            > self.plan.nodes[self.node]
                .execution_profile
                .expect("schema-3 plan profile")
                .limits
                .max_step_work
        {
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
        if plan_cord.to.node != self.plan.nodes[self.node].instance {
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
        if plan_cord.from.node != self.plan.nodes[self.node].instance {
            return Err(SchedulerError::PortAccessViolation);
        }
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
        let sampled_out = matches!(plan_cord.flow.pressure, Pressure::Sample(schedule)
            if arrival % u64::from(schedule.every()) != u64::from(schedule.offset()));
        let fits = self.cords[cord].can_fit(value, staged_items, staged_bytes);
        let (expected, effect) = if sampled_out {
            (SendStatus::Dropped, StagedOutputEffect::Other)
        } else if value.accounted_bytes > plan_cord.flow.capacity.max_value_bytes() {
            (SendStatus::Rejected, StagedOutputEffect::Other)
        } else if fits {
            (SendStatus::Reserved, StagedOutputEffect::Enqueue)
        } else {
            match plan_cord.flow.pressure {
                Pressure::Block(_) => (SendStatus::WouldBlock, StagedOutputEffect::Other),
                Pressure::Reject => (SendStatus::Rejected, StagedOutputEffect::Other),
                Pressure::Coalesce { .. } => {
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
                                        bytes <= plan_cord.flow.capacity.max_queued_bytes()
                                    })
                            })
                    });
                    if valid {
                        (SendStatus::Reserved, StagedOutputEffect::Other)
                    } else {
                        (SendStatus::Rejected, StagedOutputEffect::Other)
                    }
                }
                Pressure::Sample(_) | Pressure::DropDisposable => {
                    (SendStatus::Dropped, StagedOutputEffect::Other)
                }
                Pressure::Disconnect => (SendStatus::Disconnected, StagedOutputEffect::Terminal),
                Pressure::Fail => (SendStatus::Failed, StagedOutputEffect::Terminal),
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
            || contract.producer.node != self.plan.nodes[self.node].instance
            || contract.branches.len() != branch_values.len()
            || contract.branches.len() != coalesce_targets.len()
        {
            return Err(SchedulerError::PortAccessViolation);
        }
        if matches!(contract.duplication, DuplicationRule::SharedHandle)
            && branch_values.windows(2).any(|pair| pair[0] != pair[1])
        {
            return Err(SchedulerError::StepContractViolation);
        }
        for (index, branch) in contract.branches.iter().enumerate() {
            let cord = self
                .plan
                .cords
                .iter()
                .position(|cord| cord.id == *branch)
                .ok_or(SchedulerError::PortAccessViolation)?;
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
        if plan_cord.to.node != self.plan.nodes[self.node].instance {
            return Err(SchedulerError::PortAccessViolation);
        }
        self.wait(WaitCondition {
            interest: WakeInterest {
                kind: WakeInterestKind::Input,
                subject: plan_cord.id,
            },
            deadline_tick: None,
        })
    }

    pub fn wait_for_output(&mut self, cord: usize) -> Result<(), SchedulerError> {
        let plan_cord = self
            .plan
            .cords
            .get(cord)
            .ok_or(SchedulerError::PortAccessViolation)?;
        if plan_cord.from.node != self.plan.nodes[self.node].instance {
            return Err(SchedulerError::PortAccessViolation);
        }
        self.wait(WaitCondition {
            interest: WakeInterest {
                kind: WakeInterestKind::Output,
                subject: plan_cord.id,
            },
            deadline_tick: None,
        })
    }

    pub fn wait_for_timer(
        &mut self,
        subject: Id<'p>,
        deadline_tick: u64,
    ) -> Result<(), SchedulerError> {
        if deadline_tick <= self.tick {
            return Err(SchedulerError::StepContractViolation);
        }
        self.wait(WaitCondition {
            interest: WakeInterest {
                kind: WakeInterestKind::Timer,
                subject,
            },
            deadline_tick: Some(deadline_tick),
        })
    }

    pub fn wait_for_host_operation(&mut self, subject: Id<'p>) -> Result<(), SchedulerError> {
        self.wait(WaitCondition {
            interest: WakeInterest {
                kind: WakeInterestKind::HostOperation,
                subject,
            },
            deadline_tick: None,
        })
    }

    pub fn wait_for_cancellation(&mut self, subject: Id<'p>) -> Result<(), SchedulerError> {
        self.wait(WaitCondition {
            interest: WakeInterest {
                kind: WakeInterestKind::Cancellation,
                subject,
            },
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
        if plan_cord.to.node != self.plan.nodes[self.node].instance {
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
        if plan_cord.from.node != self.plan.nodes[self.node].instance {
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
        let limit = self.plan.nodes[self.node]
            .execution_profile
            .expect("schema-3 plan profile")
            .limits
            .max_fragments_per_step;
        if fragments > limit {
            return Err(SchedulerError::StepContractViolation);
        }
        self.workspace.fragments = fragments;
        Ok(())
    }

    fn wait(&mut self, condition: WaitCondition<'p>) -> Result<(), SchedulerError> {
        if self
            .workspace
            .interests
            .iter()
            .any(|prior| prior.interest == condition.interest)
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
pub struct DeterministicExecutor<'p, N> {
    plan: &'p ExecutionPlan<'p>,
    policy: SchedulerPolicy,
    drivers: Vec<N>,
    machines: Vec<ImplementationMachine<'p>>,
    cords: Vec<RuntimeCord<'p>>,
    workspaces: Vec<NodeWorkspace<'p>>,
    waits: Vec<Vec<WaitCondition<'p>>>,
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
    cancellation_started: Option<(u64, StopPolicy)>,
}

impl<'p, N: SchedulerNode> DeterministicExecutor<'p, N> {
    /// Validate, preallocate, prepare-all, and start-all atomically.
    pub fn start(
        plan: &'p ExecutionPlan<'p>,
        validation: conduit_core::PlanValidationContext<'p>,
        policy: SchedulerPolicy,
        reservation: SchedulerReservation,
        nodes: Vec<ScheduledNode<'p, N>>,
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

        let allocation = compute_allocation(plan, policy)?;
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
            .try_reserve_exact(plan.cords.len())
            .map_err(|_| SchedulerError::AllocationFailed)?;
        for cord in plan.cords {
            cords.push(RuntimeCord::allocate(cord.flow, cord.queue_memory_bytes)?);
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
            let capacity = interest_capacity(machine.profile())?;
            workspaces.push(NodeWorkspace::allocate(*machine, capacity)?);
            waits.push(try_vec_capacity(capacity)?);
        }
        let mut enqueued = try_vec_capacity(machines.len())?;
        enqueued.resize(machines.len(), false);
        let mut yields = try_vec_capacity(machines.len())?;
        yields.resize(machines.len(), 0);

        let event_capacity =
            usize::try_from(policy.max_events).map_err(|_| SchedulerError::AllocationFailed)?;
        let mut executor = Self {
            plan,
            policy,
            drivers,
            machines,
            cords,
            workspaces,
            waits,
            ready: FixedReadyQueue::allocate(plan.nodes.len())?,
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
    pub fn event_count(&self) -> usize {
        self.events.len
    }

    pub fn events(&self) -> impl Iterator<Item = &SchedulerEvent> {
        self.events.as_slice().iter().flatten()
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
            return Ok(self.status);
        }
        if self.decisions >= self.policy.max_decisions {
            return self.fail(SchedulerError::DecisionLimitExceeded);
        }
        let Some(entry) = self.ready.pop() else {
            self.status = SchedulerStatus::Stalled;
            self.check_cancellation_deadline()?;
            self.refresh_terminal_status()?;
            return Ok(self.status);
        };
        self.enqueued[entry.node] = false;
        self.status = SchedulerStatus::Running;
        self.record(
            SchedulerSubject::Node(as_u16(entry.node)?),
            SchedulerEventKind::Decision {
                reason: entry.reason,
            },
            0,
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

    /// Wake a bounded host operation; callback queues remain outside this API
    /// and must fit the implementation profile.
    pub fn notify_host_operation(&mut self, subject: Id<'p>) -> Result<(), SchedulerError> {
        for index in 0..self.waits.len() {
            let should_wake = self.waits[index].iter().any(|wait| {
                wait.interest.kind == WakeInterestKind::HostOperation
                    && wait.interest.subject == subject
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
        Ok(())
    }

    fn step_node(&mut self, node: usize) -> Result<(), SchedulerError> {
        self.workspaces[node].begin_step();
        let step = {
            let mut io = StepIo {
                node,
                tick: self.tick,
                plan: self.plan,
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
        for interest_index in 0..self.workspaces[node].interests.len() {
            let condition = self.workspaces[node].interests[interest_index];
            push_bounded(
                &mut self.workspaces[node].observed_interests,
                condition.interest,
            )?;
        }
        let outcome = match step {
            SchedulerStep::Progress => StepOutcome::Progress,
            SchedulerStep::Pending => {
                StepOutcome::Pending(&self.workspaces[node].observed_interests)
            }
            SchedulerStep::Yielded => StepOutcome::Yielded,
            SchedulerStep::Completed => StepOutcome::Completed,
            SchedulerStep::Failed { code } => StepOutcome::Failed { code },
        };
        let observation = self.machines[node]
            .observe_step(outcome, usage)
            .map_err(map_implementation_error)?;
        self.record(
            SchedulerSubject::Node(as_u16(node)?),
            SchedulerEventKind::NodeOutcome {
                outcome: observation.outcome(),
            },
            0,
            0,
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
        for fanout in self.plan.fanouts.iter().filter(|fanout| {
            fanout.mode == FanOutMode::Coupled
                && fanout.producer.node == self.plan.nodes[node].instance
        }) {
            let mut first = None;
            let mut published = 0_usize;
            for branch in fanout.branches {
                if let Some(output) = self.workspaces[node]
                    .outputs
                    .iter()
                    .find(|output| self.plan.cords[output.cord].id == *branch)
                {
                    if output.expected != SendStatus::Reserved {
                        return Err(SchedulerError::StepContractViolation);
                    }
                    if matches!(fanout.duplication, DuplicationRule::SharedHandle) {
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
            self.wake_for_cord(input.cord)?;
        }
        for output_index in 0..self.workspaces[node].outputs.len() {
            let output = self.workspaces[node].outputs[output_index];
            let (disposition, events) =
                self.cords[output.cord].offer(output.value, output.coalesce_target);
            self.record_cord_events(output.cord, events)?;
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
            self.max_cord_occupancy = self
                .max_cord_occupancy
                .max(self.cords[output.cord].occupancy_items());
            self.wake_for_cord(output.cord)?;
        }
        Ok(())
    }

    fn complete_outputs(&mut self, node: usize) -> Result<(), SchedulerError> {
        for cord in 0..self.plan.cords.len() {
            if self.plan.cords[cord].from.node == self.plan.nodes[node].instance {
                let events = self.cords[cord].complete_source();
                self.record_cord_events(cord, events)?;
                self.wake_for_cord(cord)?;
            }
        }
        Ok(())
    }

    fn wake_for_cord(&mut self, cord: usize) -> Result<(), SchedulerError> {
        let id = self.plan.cords[cord].id;
        let input_ready = self.cords[cord].occupancy_items() > 0
            || self.cords[cord].state() != FlowQueueState::Active;
        let output_ready = self.cords[cord].state() != FlowQueueState::Active
            || self.cords[cord].occupancy_items() < self.cords[cord].policy.capacity.items();
        for index in 0..self.waits.len() {
            let reason = self.waits[index].iter().find_map(|wait| {
                if wait.interest.subject != id {
                    return None;
                }
                match wait.interest.kind {
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
                wait.interest.kind == WakeInterestKind::Timer
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
        self.ready.push(ReadyEntry { node, reason })?;
        self.enqueued[node] = true;
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
            self.record(
                SchedulerSubject::Cord(as_u16(cord)?),
                SchedulerEventKind::Cord(event.kind),
                event.occupancy_items,
                event.occupancy_bytes,
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
    let planned_memory_bytes = planned_memory(plan)?;
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
    let wake_interest_slots = plan.nodes.iter().try_fold(0_u64, |total, node| {
        let profile = node.execution_profile.ok_or(SchedulerError::InvalidPlan)?;
        total
            .checked_add(
                u64::try_from(interest_capacity(profile)?)
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
    let ready_metadata = checked_size(u64::from(ready_slots), size_of::<Option<ReadyEntry>>())?;
    let wake_metadata = checked_size(
        wake_interest_slots
            .checked_mul(3)
            .ok_or(SchedulerError::ArithmeticOverflow)?,
        size_of::<WaitCondition<'_>>(),
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
        size_of::<ImplementationMachine<'_>>()
            + size_of::<NodeWorkspace<'_>>()
            + size_of::<Vec<WaitCondition<'_>>>()
            + size_of::<bool>()
            + size_of::<u32>(),
    )?;
    let cord_metadata = checked_size(
        u64::try_from(plan.cords.len()).map_err(|_| SchedulerError::ArithmeticOverflow)?,
        size_of::<RuntimeCord<'_>>(),
    )?;
    let startup_scratch = checked_size(
        u64::from(ready_slots),
        size_of::<PrepareOutcome<'_>>() + 2 * size_of::<LifecycleUsage>(),
    )?;
    let executor_overhead_bytes = [
        queue_metadata,
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

fn planned_memory(plan: &ExecutionPlan<'_>) -> Result<u64, SchedulerError> {
    let nodes = plan.nodes.iter().try_fold(0_u64, |total, node| {
        total.checked_add(node.allocation.memory_bytes)
    });
    let cords = plan.cords.iter().try_fold(0_u64, |total, cord| {
        total.checked_add(cord.queue_memory_bytes)
    });
    let pools = plan.instance_pools.iter().try_fold(0_u64, |total, pool| {
        total.checked_add(pool.worst_case_budget.memory_bytes)
    });
    nodes
        .and_then(|value| value.checked_add(cords?))
        .and_then(|value| value.checked_add(pools?))
        .ok_or(SchedulerError::ArithmeticOverflow)
}

fn planned_evidence(plan: &ExecutionPlan<'_>) -> Result<u64, SchedulerError> {
    let nodes = plan.nodes.iter().try_fold(0_u64, |total, node| {
        total.checked_add(node.allocation.evidence_bytes)
    });
    let pools = plan.instance_pools.iter().try_fold(0_u64, |total, pool| {
        total.checked_add(pool.worst_case_budget.evidence_bytes)
    });
    nodes
        .and_then(|value| value.checked_add(pools?))
        .ok_or(SchedulerError::ArithmeticOverflow)
}

fn checked_size(count: u64, item_size: usize) -> Result<u64, SchedulerError> {
    count
        .checked_mul(u64::try_from(item_size).map_err(|_| SchedulerError::ArithmeticOverflow)?)
        .ok_or(SchedulerError::ArithmeticOverflow)
}

fn interest_capacity(
    profile: &conduit_core::ExecutionProfile<'_>,
) -> Result<usize, SchedulerError> {
    usize::from(profile.limits.max_input_leases)
        .checked_add(usize::from(profile.limits.max_output_reservations))
        .and_then(|value| value.checked_add(usize::from(profile.limits.max_pending_operations)))
        .and_then(|value| value.checked_add(usize::from(profile.limits.max_timers)))
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
            let mut hosted = RuntimeCord::allocate(policy, 8).unwrap();
            for (sequence, target) in [(0_u64, None), (1, Some(0))] {
                let value = RuntimeValue {
                    handle: sequence,
                    accounted_bytes: 8,
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
                        .map(|event| event.kind)
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
