#![no_std]

//! Fixed-storage executor substrate for constrained firmware.
//!
//! This crate receives an already lowered static representation tied to one
//! exact plan hash. It contains no source parser, registry, dynamic loader,
//! allocator, transport, firmware updater, or device-specific semantic node.

use core::fmt;
use core::mem::size_of;

use conduit_core::{
    CanonicalDescriptor, CanonicalValue, FieldDisposition, Id, MapField, PinnedDescriptor,
    SemanticHash,
};

pub const EMBEDDED_PROFILE_SCHEMA_VERSION: u32 = 0;
pub const STATIC_PLAN_SCHEMA_VERSION: u32 = 0;
pub const HIL_PROTOCOL_VERSION: u16 = 0;
pub const MAXIMUM_NODES: u16 = 32;
pub const MAXIMUM_CORDS: u16 = 48;
pub const MAXIMUM_PORTS: u16 = 96;
pub const MAXIMUM_QUEUE_SLOTS: u16 = 128;
pub const MAXIMUM_VALUE_BYTES: u16 = 64;
pub const MAXIMUM_EVIDENCE_RECORDS: u16 = 512;
pub const MAXIMUM_TIMERS: u16 = 32;
pub const MAXIMUM_INTERESTS_PER_NODE: u8 = 8;
pub const MAXIMUM_NESTING: u8 = 8;
pub const MAXIMUM_TIMER_DELAY: u32 = i32::MAX as u32;
pub const RP2040_SRAM_BYTES: u32 = 264 * 1024;
pub const RP2040_FLASH_BYTES: u32 = 2 * 1024 * 1024;

const ZERO_HASH: SemanticHash = SemanticHash::from_bytes([0; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedProfile {
    pub identity: SemanticHash,
    pub maximum_nodes: u16,
    pub maximum_cords: u16,
    pub maximum_ports: u16,
    pub maximum_queue_slots: u16,
    pub maximum_value_bytes: u16,
    pub maximum_evidence_records: u16,
    pub maximum_timers: u16,
    pub maximum_interests_per_node: u8,
    pub maximum_nesting: u8,
    pub maximum_timer_delay: u32,
    pub static_ram_budget_bytes: u32,
    pub stack_budget_bytes: u32,
    pub flash_budget_bytes: u32,
}

impl EmbeddedProfile {
    pub fn computed_identity(&self) -> Result<SemanticHash, EmbeddedError> {
        let fields = [
            semantic(
                "maximum_nodes",
                CanonicalValue::Integer(i128::from(self.maximum_nodes)),
            ),
            semantic(
                "maximum_cords",
                CanonicalValue::Integer(i128::from(self.maximum_cords)),
            ),
            semantic(
                "maximum_ports",
                CanonicalValue::Integer(i128::from(self.maximum_ports)),
            ),
            semantic(
                "maximum_queue_slots",
                CanonicalValue::Integer(i128::from(self.maximum_queue_slots)),
            ),
            semantic(
                "maximum_value_bytes",
                CanonicalValue::Integer(i128::from(self.maximum_value_bytes)),
            ),
            semantic(
                "maximum_evidence_records",
                CanonicalValue::Integer(i128::from(self.maximum_evidence_records)),
            ),
            semantic(
                "maximum_timers",
                CanonicalValue::Integer(i128::from(self.maximum_timers)),
            ),
            semantic(
                "maximum_interests_per_node",
                CanonicalValue::Integer(i128::from(self.maximum_interests_per_node)),
            ),
            semantic(
                "maximum_nesting",
                CanonicalValue::Integer(i128::from(self.maximum_nesting)),
            ),
            semantic(
                "maximum_timer_delay",
                CanonicalValue::Integer(i128::from(self.maximum_timer_delay)),
            ),
            semantic(
                "static_ram_budget_bytes",
                CanonicalValue::Integer(i128::from(self.static_ram_budget_bytes)),
            ),
            semantic(
                "stack_budget_bytes",
                CanonicalValue::Integer(i128::from(self.stack_budget_bytes)),
            ),
            semantic(
                "flash_budget_bytes",
                CanonicalValue::Integer(i128::from(self.flash_budget_bytes)),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/embedded-execution-profile"),
            schema_version: EMBEDDED_PROFILE_SCHEMA_VERSION,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
        .map_err(|_| EmbeddedError::InvalidProfile)
    }

    pub fn seal(&mut self) -> Result<(), EmbeddedError> {
        validate_profile_shape(self)?;
        self.identity = self.computed_identity()?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), EmbeddedError> {
        validate_profile_shape(self)?;
        if self.identity != self.computed_identity()? {
            return Err(EmbeddedError::ProfileIdentityMismatch);
        }
        Ok(())
    }
}

fn validate_profile_shape(profile: &EmbeddedProfile) -> Result<(), EmbeddedError> {
    if profile.maximum_nodes == 0
        || profile.maximum_nodes > MAXIMUM_NODES
        || profile.maximum_cords == 0
        || profile.maximum_cords > MAXIMUM_CORDS
        || profile.maximum_ports == 0
        || profile.maximum_ports > MAXIMUM_PORTS
        || profile.maximum_queue_slots == 0
        || profile.maximum_queue_slots > MAXIMUM_QUEUE_SLOTS
        || profile.maximum_value_bytes == 0
        || profile.maximum_value_bytes > MAXIMUM_VALUE_BYTES
        || profile.maximum_evidence_records == 0
        || profile.maximum_evidence_records > MAXIMUM_EVIDENCE_RECORDS
        || profile.maximum_timers == 0
        || profile.maximum_timers > MAXIMUM_TIMERS
        || profile.maximum_interests_per_node == 0
        || profile.maximum_interests_per_node > MAXIMUM_INTERESTS_PER_NODE
        || profile.maximum_nesting == 0
        || profile.maximum_nesting > MAXIMUM_NESTING
        || profile.maximum_timer_delay == 0
        || profile.maximum_timer_delay > MAXIMUM_TIMER_DELAY
        || profile.static_ram_budget_bytes == 0
        || profile.static_ram_budget_bytes > RP2040_SRAM_BYTES
        || profile.stack_budget_bytes == 0
        || profile.stack_budget_bytes >= profile.static_ram_budget_bytes
        || profile.flash_budget_bytes == 0
        || profile.flash_budget_bytes > RP2040_FLASH_BYTES
    {
        return Err(EmbeddedError::InvalidProfile);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticNode<'a> {
    pub semantic_path: Id<'a>,
    pub implementation: PinnedDescriptor<'a>,
    /// Exact firmware driver binding expected at this node ordinal.
    pub driver: PinnedDescriptor<'a>,
    pub input_ports: u8,
    pub output_ports: u8,
    pub maximum_step_work: u16,
    pub nesting_depth: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticCord<'a> {
    pub semantic_id: Id<'a>,
    pub producer_node: u16,
    pub producer_port: u8,
    pub consumer_node: u16,
    pub consumer_port: u8,
    pub slot_start: u16,
    pub capacity: u16,
    pub maximum_value_bytes: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticPlan<'a> {
    pub schema_version: u32,
    /// Identity of the generated node/driver/port/queue representation.
    pub generated_plan_hash: SemanticHash,
    pub full_plan_hash: SemanticHash,
    pub profile_hash: SemanticHash,
    pub nodes: &'a [StaticNode<'a>],
    pub cords: &'a [StaticCord<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageShape {
    pub nodes: u16,
    pub cords: u16,
    pub ports: u16,
    pub queue_slots: u16,
    pub value_bytes: u16,
    pub evidence_records: u16,
    pub timers: u16,
    pub interests_per_node: u8,
    pub static_bytes: u32,
}

impl StorageShape {
    #[must_use]
    pub fn of<
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const V: usize,
        const E: usize,
        const T: usize,
        const I: usize,
    >() -> Self {
        Self {
            nodes: saturating_u16(N),
            cords: saturating_u16(C),
            ports: saturating_u16(P),
            queue_slots: saturating_u16(Q),
            value_bytes: saturating_u16(V),
            evidence_records: saturating_u16(E),
            timers: saturating_u16(T),
            interests_per_node: saturating_u8(I),
            static_bytes: u32::try_from(size_of::<EmbeddedStorage<N, C, P, Q, V, E, T, I>>())
                .unwrap_or(u32::MAX),
        }
    }
}

pub fn validate_static_plan(
    plan: &StaticPlan<'_>,
    profile: &EmbeddedProfile,
    storage: StorageShape,
) -> Result<PreflightReport, EmbeddedError> {
    profile.validate()?;
    if plan.schema_version != STATIC_PLAN_SCHEMA_VERSION
        || plan.generated_plan_hash == ZERO_HASH
        || plan.full_plan_hash == ZERO_HASH
        || plan.profile_hash != profile.identity
        || plan.nodes.is_empty()
    {
        return Err(EmbeddedError::InvalidStaticPlan);
    }
    let node_count = u16::try_from(plan.nodes.len()).map_err(|_| EmbeddedError::ProfileExceeded)?;
    let cord_count = u16::try_from(plan.cords.len()).map_err(|_| EmbeddedError::ProfileExceeded)?;
    let mut ports = 0_u16;
    for (index, node) in plan.nodes.iter().enumerate() {
        if !valid_pin(node.implementation)
            || !valid_pin(node.driver)
            || node.maximum_step_work == 0
            || node.nesting_depth == 0
            || node.nesting_depth > profile.maximum_nesting
        {
            return Err(EmbeddedError::UnsupportedFeature);
        }
        if plan.nodes[..index]
            .iter()
            .any(|other| other.semantic_path == node.semantic_path)
        {
            return Err(EmbeddedError::InvalidStaticPlan);
        }
        ports = ports
            .checked_add(u16::from(node.input_ports))
            .and_then(|value| value.checked_add(u16::from(node.output_ports)))
            .ok_or(EmbeddedError::ArithmeticOverflow)?;
    }
    let mut slots = 0_u16;
    for (index, cord) in plan.cords.iter().enumerate() {
        if cord.capacity == 0
            || cord.maximum_value_bytes == 0
            || cord.maximum_value_bytes > profile.maximum_value_bytes
            || usize::from(cord.producer_node) >= plan.nodes.len()
            || usize::from(cord.consumer_node) >= plan.nodes.len()
            || cord.producer_port >= plan.nodes[usize::from(cord.producer_node)].output_ports
            || cord.consumer_port >= plan.nodes[usize::from(cord.consumer_node)].input_ports
        {
            return Err(EmbeddedError::UnsupportedFeature);
        }
        let end = cord
            .slot_start
            .checked_add(cord.capacity)
            .ok_or(EmbeddedError::ArithmeticOverflow)?;
        slots = slots.max(end);
        for other in &plan.cords[..index] {
            if cord.semantic_id == other.semantic_id {
                return Err(EmbeddedError::InvalidStaticPlan);
            }
            let other_end = other
                .slot_start
                .checked_add(other.capacity)
                .ok_or(EmbeddedError::ArithmeticOverflow)?;
            if cord.slot_start < other_end && other.slot_start < end {
                return Err(EmbeddedError::InvalidStaticPlan);
            }
            if (cord.producer_node, cord.producer_port)
                == (other.producer_node, other.producer_port)
                || (cord.consumer_node, cord.consumer_port)
                    == (other.consumer_node, other.consumer_port)
            {
                return Err(EmbeddedError::UnsupportedFeature);
            }
        }
    }
    if node_count > profile.maximum_nodes
        || node_count > storage.nodes
        || cord_count > profile.maximum_cords
        || cord_count > storage.cords
        || ports > profile.maximum_ports
        || ports > storage.ports
        || slots > profile.maximum_queue_slots
        || slots > storage.queue_slots
        || profile.maximum_value_bytes > storage.value_bytes
        || profile.maximum_evidence_records > storage.evidence_records
        || profile.maximum_timers > storage.timers
        || profile.maximum_interests_per_node > storage.interests_per_node
        || storage.static_bytes > profile.static_ram_budget_bytes
    {
        return Err(EmbeddedError::ProfileExceeded);
    }
    Ok(PreflightReport {
        generated_plan_hash: plan.generated_plan_hash,
        full_plan_hash: plan.full_plan_hash,
        profile_hash: profile.identity,
        nodes: node_count,
        cords: cord_count,
        ports,
        queue_slots: slots,
        static_storage_bytes: storage.static_bytes,
        stack_budget_bytes: profile.stack_budget_bytes,
        flash_budget_bytes: profile.flash_budget_bytes,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreflightReport {
    pub generated_plan_hash: SemanticHash,
    pub full_plan_hash: SemanticHash,
    pub profile_hash: SemanticHash,
    pub nodes: u16,
    pub cords: u16,
    pub ports: u16,
    pub queue_slots: u16,
    pub static_storage_bytes: u32,
    pub stack_budget_bytes: u32,
    pub flash_budget_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedValue<const V: usize> {
    pub length: u16,
    pub bytes: [u8; V],
}

impl<const V: usize> EmbeddedValue<V> {
    pub const EMPTY: Self = Self {
        length: 0,
        bytes: [0; V],
    };

    pub fn from_slice(bytes: &[u8]) -> Result<Self, EmbeddedError> {
        if bytes.len() > V || bytes.len() > usize::from(u16::MAX) {
            return Err(EmbeddedError::ValueTooLarge);
        }
        let mut value = Self::EMPTY;
        value.length = u16::try_from(bytes.len()).map_err(|_| EmbeddedError::ValueTooLarge)?;
        value.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(value)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedInterest {
    Input(u8),
    Output(u8),
    Timer(u32),
    Cancellation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterestSet<const I: usize> {
    values: [Option<EmbeddedInterest>; I],
}

impl<const I: usize> InterestSet<I> {
    pub const EMPTY: Self = Self { values: [None; I] };

    pub fn one(value: EmbeddedInterest) -> Self {
        let mut set = Self::EMPTY;
        if I > 0 {
            set.values[0] = Some(value);
        }
        set
    }

    pub fn push(&mut self, value: EmbeddedInterest) -> Result<(), EmbeddedError> {
        if self
            .values
            .iter()
            .flatten()
            .any(|current| *current == value)
        {
            return Err(EmbeddedError::InvalidInterest);
        }
        let slot = self
            .values
            .iter_mut()
            .find(|current| current.is_none())
            .ok_or(EmbeddedError::InterestCapacityExceeded)?;
        *slot = Some(value);
        Ok(())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.iter().all(Option::is_none)
    }

    pub fn iter(&self) -> impl Iterator<Item = EmbeddedInterest> + '_ {
        self.values.iter().copied().flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedOutcome {
    Progress,
    Pending,
    Yielded,
    Completed,
    Failed(Id<'static>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedStep<const I: usize> {
    pub outcome: EmbeddedOutcome,
    pub interests: InterestSet<I>,
}

impl<const I: usize> EmbeddedStep<I> {
    #[must_use]
    pub const fn progress() -> Self {
        Self {
            outcome: EmbeddedOutcome::Progress,
            interests: InterestSet::EMPTY,
        }
    }

    #[must_use]
    pub const fn completed() -> Self {
        Self {
            outcome: EmbeddedOutcome::Completed,
            interests: InterestSet::EMPTY,
        }
    }

    #[must_use]
    pub fn pending(interests: InterestSet<I>) -> Self {
        Self {
            outcome: EmbeddedOutcome::Pending,
            interests,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostReply<const V: usize> {
    Completed(EmbeddedValue<V>),
    Pending,
    Failed(Id<'static>),
}

pub trait EmbeddedHostServices<const V: usize> {
    fn invoke(&mut self, binding: u16, request: EmbeddedValue<V>) -> HostReply<V>;
}

pub trait EmbeddedNode<H, const V: usize, const P: usize, const I: usize>
where
    H: EmbeddedHostServices<V>,
{
    /// Exact driver implementation bound into the generated static plan.
    fn descriptor(&self) -> PinnedDescriptor<'static>;

    fn prepare(&mut self, _host: &mut H) -> Result<(), Id<'static>> {
        Ok(())
    }

    fn start(&mut self, _host: &mut H) -> Result<(), Id<'static>> {
        Ok(())
    }

    fn step(&mut self, context: &mut StepContext<'_, H, V, P>) -> EmbeddedStep<I>;

    fn cancel(&mut self, _host: &mut H) {}
}

pub struct StepContext<'a, H, const V: usize, const P: usize>
where
    H: EmbeddedHostServices<V>,
{
    host: &'a mut H,
    tick: u32,
    maximum_work: u16,
    work: u16,
    inputs: [Option<EmbeddedValue<V>>; P],
    input_closed: [bool; P],
    consumed: [bool; P],
    output_ready: [bool; P],
    outputs: [Option<EmbeddedValue<V>>; P],
    host_progress: bool,
    fault: Option<EmbeddedError>,
}

impl<H, const V: usize, const P: usize> StepContext<'_, H, V, P>
where
    H: EmbeddedHostServices<V>,
{
    #[must_use]
    pub const fn tick(&self) -> u32 {
        self.tick
    }

    #[must_use]
    pub fn input(&self, port: u8) -> Option<EmbeddedValue<V>> {
        self.inputs.get(usize::from(port)).copied().flatten()
    }

    #[must_use]
    pub fn input_closed(&self, port: u8) -> bool {
        self.input_closed
            .get(usize::from(port))
            .copied()
            .unwrap_or(true)
    }

    pub fn consume(&mut self, port: u8) -> Result<EmbeddedValue<V>, EmbeddedError> {
        self.charge_work(1)?;
        let index = usize::from(port);
        let Some(value) = self.inputs.get(index).copied().flatten() else {
            return self.fail(EmbeddedError::PortAccessViolation);
        };
        if self.consumed.get(index).copied().unwrap_or(true) {
            return self.fail(EmbeddedError::PortAccessViolation);
        }
        self.consumed[index] = true;
        Ok(value)
    }

    #[must_use]
    pub fn output_ready(&self, port: u8) -> bool {
        self.output_ready
            .get(usize::from(port))
            .copied()
            .unwrap_or(false)
    }

    pub fn send(&mut self, port: u8, value: EmbeddedValue<V>) -> Result<(), EmbeddedError> {
        self.charge_work(1)?;
        let index = usize::from(port);
        if !self.output_ready.get(index).copied().unwrap_or(false)
            || self.outputs.get(index).is_none()
            || self.outputs[index].is_some()
        {
            return self.fail(EmbeddedError::PortAccessViolation);
        }
        self.outputs[index] = Some(value);
        Ok(())
    }

    pub fn invoke_host(
        &mut self,
        binding: u16,
        request: EmbeddedValue<V>,
    ) -> Result<HostReply<V>, EmbeddedError> {
        self.charge_work(1)?;
        let reply = self.host.invoke(binding, request);
        self.host_progress |= matches!(reply, HostReply::Completed(_));
        Ok(reply)
    }

    pub fn charge_work(&mut self, units: u16) -> Result<(), EmbeddedError> {
        let Some(work) = self.work.checked_add(units) else {
            return self.fail(EmbeddedError::StepWorkExceeded);
        };
        self.work = work;
        if work > self.maximum_work {
            return self.fail(EmbeddedError::StepWorkExceeded);
        }
        Ok(())
    }

    fn fail<T>(&mut self, error: EmbeddedError) -> Result<T, EmbeddedError> {
        if self.fault.is_none() {
            self.fault = Some(error);
        }
        Err(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunIdentity {
    pub boot_id: [u8; 16],
    pub run_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunControl {
    pub maximum_decisions: u32,
    pub cancellation_at_decision: Option<u32>,
    pub initial_tick: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedEventKind {
    AllocationPrepared,
    NodePrepared,
    RunStarted,
    Decision,
    ValueAccepted,
    ValueConsumed,
    PressureEntered,
    PressureCleared,
    NodeCompleted,
    CancellationRequested,
    RunSucceeded,
    RunCancelled,
    RunFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedSubject {
    Run,
    Node(u16),
    Cord(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedEvent<const V: usize> {
    pub plan: SemanticHash,
    pub run: RunIdentity,
    pub sequence: u32,
    pub tick: u32,
    pub subject: EmbeddedSubject,
    pub kind: EmbeddedEventKind,
    pub value: Option<EmbeddedValue<V>>,
}

impl<const V: usize> EmbeddedEvent<V> {
    const EMPTY: Self = Self {
        plan: ZERO_HASH,
        run: RunIdentity {
            boot_id: [0; 16],
            run_sequence: 0,
        },
        sequence: 0,
        tick: 0,
        subject: EmbeddedSubject::Run,
        kind: EmbeddedEventKind::AllocationPrepared,
        value: None,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerWait {
    node: u16,
    deadline: u32,
}

pub struct EmbeddedStorage<
    const N: usize,
    const C: usize,
    const P: usize,
    const Q: usize,
    const V: usize,
    const E: usize,
    const T: usize,
    const I: usize,
> {
    queue: [Option<EmbeddedValue<V>>; Q],
    queue_heads: [u16; C],
    queue_lengths: [u16; C],
    node_completed: [bool; N],
    ready: [bool; N],
    waits: [InterestSet<I>; N],
    timers: [Option<TimerWait>; T],
    events: [EmbeddedEvent<V>; E],
    event_count: u16,
    _port_scratch: [u8; P],
}

impl<
    const N: usize,
    const C: usize,
    const P: usize,
    const Q: usize,
    const V: usize,
    const E: usize,
    const T: usize,
    const I: usize,
> EmbeddedStorage<N, C, P, Q, V, E, T, I>
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queue: [None; Q],
            queue_heads: [0; C],
            queue_lengths: [0; C],
            node_completed: [false; N],
            ready: [false; N],
            waits: [InterestSet::EMPTY; N],
            timers: [None; T],
            events: [EmbeddedEvent::EMPTY; E],
            event_count: 0,
            _port_scratch: [0; P],
        }
    }

    fn reset(&mut self) {
        self.queue.fill(None);
        self.queue_heads.fill(0);
        self.queue_lengths.fill(0);
        self.node_completed.fill(false);
        self.ready.fill(false);
        self.waits.fill(InterestSet::EMPTY);
        self.timers.fill(None);
        self.events.fill(EmbeddedEvent::EMPTY);
        self.event_count = 0;
    }

    #[must_use]
    pub fn events(&self) -> &[EmbeddedEvent<V>] {
        &self.events[..usize::from(self.event_count)]
    }
}

impl<
    const N: usize,
    const C: usize,
    const P: usize,
    const Q: usize,
    const V: usize,
    const E: usize,
    const T: usize,
    const I: usize,
> Default for EmbeddedStorage<N, C, P, Q, V, E, T, I>
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Succeeded,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunSummary {
    pub status: RunStatus,
    pub decisions: u32,
    pub final_tick: u32,
    pub evidence_records: u16,
    pub maximum_queue_occupancy: u16,
}

#[allow(clippy::too_many_arguments)]
pub fn execute_static_plan<
    D,
    H,
    const N: usize,
    const C: usize,
    const P: usize,
    const Q: usize,
    const V: usize,
    const E: usize,
    const T: usize,
    const I: usize,
>(
    plan: &StaticPlan<'_>,
    profile: &EmbeddedProfile,
    storage: &mut EmbeddedStorage<N, C, P, Q, V, E, T, I>,
    drivers: &mut [D],
    host: &mut H,
    identity: RunIdentity,
    control: RunControl,
) -> Result<RunSummary, EmbeddedError>
where
    H: EmbeddedHostServices<V>,
    D: EmbeddedNode<H, V, P, I>,
{
    validate_static_plan(plan, profile, StorageShape::of::<N, C, P, Q, V, E, T, I>())?;
    if drivers.len() != plan.nodes.len()
        || control.maximum_decisions == 0
        || control
            .cancellation_at_decision
            .is_some_and(|value| value > control.maximum_decisions)
    {
        return Err(EmbeddedError::InvalidRun);
    }
    if drivers
        .iter()
        .zip(plan.nodes)
        .any(|(driver, node)| driver.descriptor() != node.driver)
    {
        return Err(EmbeddedError::DriverBindingMismatch);
    }
    storage.reset();
    let mut state = ExecutionState {
        plan,
        storage,
        identity,
        tick: control.initial_tick,
        sequence: 0,
        decisions: 0,
        cursor: 0,
        maximum_queue_occupancy: 0,
        maximum_evidence_records: profile.maximum_evidence_records,
        maximum_timers: profile.maximum_timers,
        maximum_interests_per_node: profile.maximum_interests_per_node,
        maximum_timer_delay: profile.maximum_timer_delay,
    };
    state.ensure_event_capacity(
        plan.nodes
            .len()
            .checked_add(2)
            .ok_or(EmbeddedError::ArithmeticOverflow)?,
    )?;
    state.record(
        EmbeddedSubject::Run,
        EmbeddedEventKind::AllocationPrepared,
        None,
    )?;
    for (index, driver) in drivers.iter_mut().enumerate() {
        driver
            .prepare(host)
            .map_err(|_| EmbeddedError::PrepareFailed)?;
        state.record(
            EmbeddedSubject::Node(as_u16(index)?),
            EmbeddedEventKind::NodePrepared,
            None,
        )?;
    }
    for driver in drivers.iter_mut() {
        driver.start(host).map_err(|_| EmbeddedError::StartFailed)?;
    }
    for ready in &mut state.storage.ready[..drivers.len()] {
        *ready = true;
    }
    state.record(EmbeddedSubject::Run, EmbeddedEventKind::RunStarted, None)?;

    loop {
        if control.cancellation_at_decision == Some(state.decisions) {
            state.ensure_event_capacity(2)?;
            state.record(
                EmbeddedSubject::Run,
                EmbeddedEventKind::CancellationRequested,
                None,
            )?;
            for driver in drivers.iter_mut() {
                driver.cancel(host);
            }
            state.storage.queue.fill(None);
            state.storage.queue_lengths.fill(0);
            state.record(EmbeddedSubject::Run, EmbeddedEventKind::RunCancelled, None)?;
            return Ok(state.summary(RunStatus::Cancelled));
        }
        if state.storage.node_completed[..drivers.len()]
            .iter()
            .all(|completed| *completed)
            && state.storage.queue_lengths[..plan.cords.len()]
                .iter()
                .all(|length| *length == 0)
        {
            state.record(EmbeddedSubject::Run, EmbeddedEventKind::RunSucceeded, None)?;
            return Ok(state.summary(RunStatus::Succeeded));
        }
        if state.decisions >= control.maximum_decisions {
            return Err(EmbeddedError::DecisionLimitExceeded);
        }
        state.wake_waiters()?;
        let Some(node_index) = state.next_ready(drivers.len()) else {
            if let Some(deadline) = state.next_timer_deadline() {
                state.tick = deadline;
                state.wake_waiters()?;
                continue;
            }
            return Err(EmbeddedError::Stalled);
        };
        state.ensure_event_capacity(state.maximum_step_evidence(node_index)?)?;
        state.decisions += 1;
        state.tick = state.tick.wrapping_add(1);
        state.record(
            EmbeddedSubject::Node(as_u16(node_index)?),
            EmbeddedEventKind::Decision,
            None,
        )?;
        let node = plan.nodes[node_index];
        let (reply, work, consumed, outputs, host_progress, fault) = {
            let mut context = state.context::<H, P>(node_index, node.maximum_step_work, host)?;
            let reply = drivers[node_index].step(&mut context);
            (
                reply,
                context.work,
                context.consumed,
                context.outputs,
                context.host_progress,
                context.fault,
            )
        };
        if let Some(error) = fault {
            return Err(error);
        }
        state.apply_step(node_index, reply, work, consumed, outputs, host_progress)?;
    }
}

struct ExecutionState<
    'a,
    'p,
    const N: usize,
    const C: usize,
    const P: usize,
    const Q: usize,
    const V: usize,
    const E: usize,
    const T: usize,
    const I: usize,
> {
    plan: &'p StaticPlan<'p>,
    storage: &'a mut EmbeddedStorage<N, C, P, Q, V, E, T, I>,
    identity: RunIdentity,
    tick: u32,
    sequence: u32,
    decisions: u32,
    cursor: usize,
    maximum_queue_occupancy: u16,
    maximum_evidence_records: u16,
    maximum_timers: u16,
    maximum_interests_per_node: u8,
    maximum_timer_delay: u32,
}

impl<
    const N: usize,
    const C: usize,
    const P: usize,
    const Q: usize,
    const V: usize,
    const E: usize,
    const T: usize,
    const I: usize,
> ExecutionState<'_, '_, N, C, P, Q, V, E, T, I>
{
    fn summary(&self, status: RunStatus) -> RunSummary {
        RunSummary {
            status,
            decisions: self.decisions,
            final_tick: self.tick,
            evidence_records: self.storage.event_count,
            maximum_queue_occupancy: self.maximum_queue_occupancy,
        }
    }

    fn ensure_event_capacity(&self, additional: usize) -> Result<(), EmbeddedError> {
        let required = usize::from(self.storage.event_count)
            .checked_add(additional)
            .ok_or(EmbeddedError::ArithmeticOverflow)?;
        if required > usize::from(self.maximum_evidence_records) || required > E {
            return Err(EmbeddedError::EvidenceCapacityExceeded);
        }
        Ok(())
    }

    fn maximum_step_evidence(&self, node: usize) -> Result<usize, EmbeddedError> {
        let ports = usize::from(self.plan.nodes[node].input_ports)
            .checked_add(usize::from(self.plan.nodes[node].output_ports))
            .ok_or(EmbeddedError::ArithmeticOverflow)?;
        ports
            .checked_mul(2)
            .and_then(|records| records.checked_add(2))
            .ok_or(EmbeddedError::ArithmeticOverflow)
    }

    fn record(
        &mut self,
        subject: EmbeddedSubject,
        kind: EmbeddedEventKind,
        value: Option<EmbeddedValue<V>>,
    ) -> Result<(), EmbeddedError> {
        let index = usize::from(self.storage.event_count);
        if self.storage.event_count >= self.maximum_evidence_records {
            return Err(EmbeddedError::EvidenceCapacityExceeded);
        }
        let slot = self
            .storage
            .events
            .get_mut(index)
            .ok_or(EmbeddedError::EvidenceCapacityExceeded)?;
        *slot = EmbeddedEvent {
            plan: self.plan.full_plan_hash,
            run: self.identity,
            sequence: self.sequence,
            tick: self.tick,
            subject,
            kind,
            value,
        };
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(EmbeddedError::ArithmeticOverflow)?;
        self.storage.event_count = self
            .storage
            .event_count
            .checked_add(1)
            .ok_or(EmbeddedError::EvidenceCapacityExceeded)?;
        Ok(())
    }

    fn next_ready(&mut self, count: usize) -> Option<usize> {
        for offset in 0..count {
            let index = (self.cursor + offset) % count;
            if self.storage.ready[index] && !self.storage.node_completed[index] {
                self.cursor = (index + 1) % count;
                return Some(index);
            }
        }
        None
    }

    fn context<'h, H, const PORTS: usize>(
        &mut self,
        node: usize,
        maximum_work: u16,
        host: &'h mut H,
    ) -> Result<StepContext<'h, H, V, PORTS>, EmbeddedError>
    where
        H: EmbeddedHostServices<V>,
    {
        let mut inputs = [None; PORTS];
        let mut input_closed = [false; PORTS];
        let mut output_ready = [false; PORTS];
        for (cord_index, cord) in self.plan.cords.iter().enumerate() {
            if usize::from(cord.consumer_node) == node {
                let port = usize::from(cord.consumer_port);
                if port >= PORTS {
                    return Err(EmbeddedError::PortAccessViolation);
                }
                inputs[port] = self.peek(cord_index);
                input_closed[port] = self.storage.node_completed[usize::from(cord.producer_node)]
                    && self.storage.queue_lengths[cord_index] == 0;
            }
            if usize::from(cord.producer_node) == node {
                let port = usize::from(cord.producer_port);
                if port >= PORTS {
                    return Err(EmbeddedError::PortAccessViolation);
                }
                output_ready[port] = self.storage.queue_lengths[cord_index] < cord.capacity;
            }
        }
        Ok(StepContext {
            host,
            tick: self.tick,
            maximum_work,
            work: 0,
            inputs,
            input_closed,
            consumed: [false; PORTS],
            output_ready,
            outputs: [None; PORTS],
            host_progress: false,
            fault: None,
        })
    }

    fn apply_step(
        &mut self,
        node: usize,
        reply: EmbeddedStep<I>,
        work: u16,
        consumed: [bool; P],
        outputs: [Option<EmbeddedValue<V>>; P],
        host_progress: bool,
    ) -> Result<(), EmbeddedError> {
        if work > self.plan.nodes[node].maximum_step_work {
            return Err(EmbeddedError::StepWorkExceeded);
        }
        let staged = consumed.iter().any(|value| *value) || outputs.iter().any(Option::is_some);
        match reply.outcome {
            EmbeddedOutcome::Progress | EmbeddedOutcome::Completed => {
                if matches!(reply.outcome, EmbeddedOutcome::Progress) && !staged && !host_progress {
                    return Err(EmbeddedError::FalseProgress);
                }
                self.commit(node, consumed, outputs)?;
                self.storage.waits[node] = InterestSet::EMPTY;
                self.clear_timers_for(node);
                if matches!(reply.outcome, EmbeddedOutcome::Completed) {
                    self.storage.node_completed[node] = true;
                    self.storage.ready[node] = false;
                    self.record(
                        EmbeddedSubject::Node(as_u16(node)?),
                        EmbeddedEventKind::NodeCompleted,
                        None,
                    )?;
                } else {
                    self.storage.ready[node] = true;
                }
            }
            EmbeddedOutcome::Pending => {
                if staged || reply.interests.is_empty() {
                    return Err(EmbeddedError::InvalidInterest);
                }
                self.install_waits(node, reply.interests)?;
                self.storage.ready[node] = false;
            }
            EmbeddedOutcome::Yielded => {
                if staged || work != self.plan.nodes[node].maximum_step_work {
                    return Err(EmbeddedError::FalseProgress);
                }
                self.storage.ready[node] = true;
            }
            EmbeddedOutcome::Failed(_code) => {
                self.record(EmbeddedSubject::Run, EmbeddedEventKind::RunFailed, None)?;
                return Err(EmbeddedError::NodeFailed);
            }
        }
        Ok(())
    }

    fn commit(
        &mut self,
        node: usize,
        consumed: [bool; P],
        outputs: [Option<EmbeddedValue<V>>; P],
    ) -> Result<(), EmbeddedError> {
        for (port, value) in outputs
            .iter()
            .enumerate()
            .filter_map(|(port, value)| value.map(|value| (port, value)))
        {
            let cord_index = self
                .output_cord(node, port)
                .ok_or(EmbeddedError::PortAccessViolation)?;
            if self.storage.queue_lengths[cord_index] >= self.plan.cords[cord_index].capacity
                || value.length > self.plan.cords[cord_index].maximum_value_bytes
            {
                return Err(EmbeddedError::PortAccessViolation);
            }
        }
        for (port, should_consume) in consumed.iter().copied().enumerate() {
            if should_consume {
                let cord_index = self
                    .input_cord(node, port)
                    .ok_or(EmbeddedError::PortAccessViolation)?;
                let was_full =
                    self.storage.queue_lengths[cord_index] == self.plan.cords[cord_index].capacity;
                let value = self.pop(cord_index)?;
                self.record(
                    EmbeddedSubject::Cord(as_u16(cord_index)?),
                    EmbeddedEventKind::ValueConsumed,
                    Some(value),
                )?;
                if was_full {
                    self.record(
                        EmbeddedSubject::Cord(as_u16(cord_index)?),
                        EmbeddedEventKind::PressureCleared,
                        None,
                    )?;
                }
            }
        }
        for (port, value) in outputs
            .iter()
            .enumerate()
            .filter_map(|(port, value)| value.map(|value| (port, value)))
        {
            let cord_index = self
                .output_cord(node, port)
                .ok_or(EmbeddedError::PortAccessViolation)?;
            self.push(cord_index, value)?;
            self.record(
                EmbeddedSubject::Cord(as_u16(cord_index)?),
                EmbeddedEventKind::ValueAccepted,
                Some(value),
            )?;
            if self.storage.queue_lengths[cord_index] == self.plan.cords[cord_index].capacity {
                self.record(
                    EmbeddedSubject::Cord(as_u16(cord_index)?),
                    EmbeddedEventKind::PressureEntered,
                    None,
                )?;
            }
            self.maximum_queue_occupancy = self
                .maximum_queue_occupancy
                .max(self.storage.queue_lengths[cord_index]);
        }
        Ok(())
    }

    fn input_cord(&self, node: usize, port: usize) -> Option<usize> {
        self.plan.cords.iter().position(|cord| {
            usize::from(cord.consumer_node) == node && usize::from(cord.consumer_port) == port
        })
    }

    fn output_cord(&self, node: usize, port: usize) -> Option<usize> {
        self.plan.cords.iter().position(|cord| {
            usize::from(cord.producer_node) == node && usize::from(cord.producer_port) == port
        })
    }

    fn peek(&self, cord_index: usize) -> Option<EmbeddedValue<V>> {
        if self.storage.queue_lengths[cord_index] == 0 {
            return None;
        }
        let cord = self.plan.cords[cord_index];
        let offset = self.storage.queue_heads[cord_index] % cord.capacity;
        self.storage.queue[usize::from(cord.slot_start + offset)]
    }

    fn pop(&mut self, cord_index: usize) -> Result<EmbeddedValue<V>, EmbeddedError> {
        let cord = self.plan.cords[cord_index];
        if self.storage.queue_lengths[cord_index] == 0 {
            return Err(EmbeddedError::PortAccessViolation);
        }
        let offset = self.storage.queue_heads[cord_index] % cord.capacity;
        let slot = usize::from(cord.slot_start + offset);
        let value = self.storage.queue[slot]
            .take()
            .ok_or(EmbeddedError::PortAccessViolation)?;
        self.storage.queue_heads[cord_index] =
            (self.storage.queue_heads[cord_index] + 1) % cord.capacity;
        self.storage.queue_lengths[cord_index] -= 1;
        Ok(value)
    }

    fn push(&mut self, cord_index: usize, value: EmbeddedValue<V>) -> Result<(), EmbeddedError> {
        let cord = self.plan.cords[cord_index];
        let length = self.storage.queue_lengths[cord_index];
        if length >= cord.capacity {
            return Err(EmbeddedError::PortAccessViolation);
        }
        let offset = (self.storage.queue_heads[cord_index] + length) % cord.capacity;
        let slot = usize::from(cord.slot_start + offset);
        if self.storage.queue[slot].replace(value).is_some() {
            return Err(EmbeddedError::PortAccessViolation);
        }
        self.storage.queue_lengths[cord_index] += 1;
        Ok(())
    }

    fn install_waits(&mut self, node: usize, waits: InterestSet<I>) -> Result<(), EmbeddedError> {
        self.clear_timers_for(node);
        let interest_count = waits.iter().count();
        if interest_count > usize::from(self.maximum_interests_per_node) {
            return Err(EmbeddedError::InterestCapacityExceeded);
        }
        for interest in waits.iter() {
            match interest {
                EmbeddedInterest::Input(port) => {
                    if self.input_cord(node, usize::from(port)).is_none() {
                        return Err(EmbeddedError::InvalidInterest);
                    }
                }
                EmbeddedInterest::Output(port) => {
                    if self.output_cord(node, usize::from(port)).is_none() {
                        return Err(EmbeddedError::InvalidInterest);
                    }
                }
                EmbeddedInterest::Timer(deadline) => {
                    let delay = deadline.wrapping_sub(self.tick);
                    if delay == 0 || delay > MAXIMUM_TIMER_DELAY || delay > self.maximum_timer_delay
                    {
                        return Err(EmbeddedError::InvalidTimer);
                    }
                    let timer = self.storage.timers[..usize::from(self.maximum_timers)]
                        .iter_mut()
                        .find(|timer| timer.is_none())
                        .ok_or(EmbeddedError::TimerCapacityExceeded)?;
                    *timer = Some(TimerWait {
                        node: as_u16(node)?,
                        deadline,
                    });
                }
                EmbeddedInterest::Cancellation => {}
            }
        }
        self.storage.waits[node] = waits;
        Ok(())
    }

    fn clear_timers_for(&mut self, node: usize) {
        for timer in &mut self.storage.timers[..usize::from(self.maximum_timers)] {
            if timer.is_some_and(|timer| usize::from(timer.node) == node) {
                *timer = None;
            }
        }
    }

    fn wake_waiters(&mut self) -> Result<(), EmbeddedError> {
        for node in 0..self.plan.nodes.len() {
            if self.storage.node_completed[node] || self.storage.ready[node] {
                continue;
            }
            let mut wake = false;
            for interest in self.storage.waits[node].iter() {
                wake |= match interest {
                    EmbeddedInterest::Input(port) => self
                        .input_cord(node, usize::from(port))
                        .is_some_and(|cord| {
                            self.storage.queue_lengths[cord] > 0
                                || (self.storage.node_completed
                                    [usize::from(self.plan.cords[cord].producer_node)]
                                    && self.storage.queue_lengths[cord] == 0)
                        }),
                    EmbeddedInterest::Output(port) => self
                        .output_cord(node, usize::from(port))
                        .is_some_and(|cord| {
                            self.storage.queue_lengths[cord] < self.plan.cords[cord].capacity
                        }),
                    EmbeddedInterest::Timer(deadline) => deadline_reached(self.tick, deadline),
                    EmbeddedInterest::Cancellation => false,
                };
            }
            if wake {
                self.storage.ready[node] = true;
                self.storage.waits[node] = InterestSet::EMPTY;
                self.clear_timers_for(node);
            }
        }
        Ok(())
    }

    fn next_timer_deadline(&self) -> Option<u32> {
        self.storage.timers[..usize::from(self.maximum_timers)]
            .iter()
            .flatten()
            .map(|timer| timer.deadline)
            .min_by_key(|deadline| deadline.wrapping_sub(self.tick))
    }
}

#[must_use]
pub const fn deadline_reached(now: u32, deadline: u32) -> bool {
    now.wrapping_sub(deadline) <= MAXIMUM_TIMER_DELAY
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirmwareReplacementLevel {
    Cold,
    Quiescent,
    StatefulHot,
}

pub fn validate_firmware_replacement(
    level: FirmwareReplacementLevel,
    old_generation_bytes: u32,
    new_generation_bytes: u32,
    available_overlap_bytes: u32,
) -> Result<(), EmbeddedError> {
    match level {
        FirmwareReplacementLevel::Cold => Ok(()),
        FirmwareReplacementLevel::Quiescent => {
            let overlap = old_generation_bytes
                .checked_add(new_generation_bytes)
                .ok_or(EmbeddedError::ArithmeticOverflow)?;
            if overlap > available_overlap_bytes {
                Err(EmbeddedError::ReplacementUnsupported)
            } else {
                Ok(())
            }
        }
        FirmwareReplacementLevel::StatefulHot => Err(EmbeddedError::ReplacementUnsupported),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HilRequest {
    pub protocol_version: u16,
    pub nonce: [u8; 16],
    pub expected_plan_hash: SemanticHash,
    pub maximum_decisions: u32,
}

impl HilRequest {
    pub const ENCODED_BYTES: usize = 58;

    pub fn encode(&self, output: &mut [u8; Self::ENCODED_BYTES]) {
        output[..4].copy_from_slice(b"CNH0");
        output[4..6].copy_from_slice(&self.protocol_version.to_be_bytes());
        output[6..22].copy_from_slice(&self.nonce);
        output[22..54].copy_from_slice(self.expected_plan_hash.as_bytes());
        output[54..58].copy_from_slice(&self.maximum_decisions.to_be_bytes());
    }

    pub fn decode(input: &[u8; Self::ENCODED_BYTES]) -> Result<Self, EmbeddedError> {
        let protocol_version = u16::from_be_bytes([input[4], input[5]]);
        if &input[..4] != b"CNH0" || protocol_version != HIL_PROTOCOL_VERSION {
            return Err(EmbeddedError::UnsupportedHilProtocol);
        }
        let mut nonce = [0; 16];
        nonce.copy_from_slice(&input[6..22]);
        let mut plan = [0; 32];
        plan.copy_from_slice(&input[22..54]);
        let maximum_decisions = u32::from_be_bytes([input[54], input[55], input[56], input[57]]);
        if maximum_decisions == 0 {
            return Err(EmbeddedError::InvalidRun);
        }
        Ok(Self {
            protocol_version,
            nonce,
            expected_plan_hash: SemanticHash::from_bytes(plan),
            maximum_decisions,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HilRunStatus {
    Succeeded,
    Cancelled,
    Failed,
}

impl HilRunStatus {
    const fn code(self) -> u8 {
        match self {
            Self::Succeeded => 1,
            Self::Cancelled => 2,
            Self::Failed => 3,
        }
    }

    fn from_code(value: u8) -> Result<Self, EmbeddedError> {
        match value {
            1 => Ok(Self::Succeeded),
            2 => Ok(Self::Cancelled),
            3 => Ok(Self::Failed),
            _ => Err(EmbeddedError::UnsupportedHilProtocol),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HilRunHeader {
    pub protocol_version: u16,
    pub nonce: [u8; 16],
    pub plan_hash: SemanticHash,
    pub firmware_identity: SemanticHash,
    pub capability_report_hash: SemanticHash,
    pub run: RunIdentity,
    pub status: HilRunStatus,
    pub decisions: u32,
    pub evidence_records: u16,
}

impl HilRunHeader {
    pub const ENCODED_BYTES: usize = 149;

    pub fn encode(&self, output: &mut [u8; Self::ENCODED_BYTES]) {
        output[..4].copy_from_slice(b"CNR0");
        output[4..6].copy_from_slice(&self.protocol_version.to_be_bytes());
        output[6..22].copy_from_slice(&self.nonce);
        output[22..54].copy_from_slice(self.plan_hash.as_bytes());
        output[54..86].copy_from_slice(self.firmware_identity.as_bytes());
        output[86..118].copy_from_slice(self.capability_report_hash.as_bytes());
        output[118..134].copy_from_slice(&self.run.boot_id);
        output[134..142].copy_from_slice(&self.run.run_sequence.to_be_bytes());
        output[142] = self.status.code();
        output[143..147].copy_from_slice(&self.decisions.to_be_bytes());
        output[147..149].copy_from_slice(&self.evidence_records.to_be_bytes());
    }

    pub fn decode(input: &[u8; Self::ENCODED_BYTES]) -> Result<Self, EmbeddedError> {
        let protocol_version = u16::from_be_bytes([input[4], input[5]]);
        if &input[..4] != b"CNR0" || protocol_version != HIL_PROTOCOL_VERSION {
            return Err(EmbeddedError::UnsupportedHilProtocol);
        }
        let mut nonce = [0; 16];
        nonce.copy_from_slice(&input[6..22]);
        let mut plan = [0; 32];
        plan.copy_from_slice(&input[22..54]);
        let mut firmware_identity = [0; 32];
        firmware_identity.copy_from_slice(&input[54..86]);
        let mut capability_report_hash = [0; 32];
        capability_report_hash.copy_from_slice(&input[86..118]);
        let mut boot_id = [0; 16];
        boot_id.copy_from_slice(&input[118..134]);
        Ok(Self {
            protocol_version,
            nonce,
            plan_hash: SemanticHash::from_bytes(plan),
            firmware_identity: SemanticHash::from_bytes(firmware_identity),
            capability_report_hash: SemanticHash::from_bytes(capability_report_hash),
            run: RunIdentity {
                boot_id,
                run_sequence: u64::from_be_bytes([
                    input[134], input[135], input[136], input[137], input[138], input[139],
                    input[140], input[141],
                ]),
            },
            status: HilRunStatus::from_code(input[142])?,
            decisions: u32::from_be_bytes([input[143], input[144], input[145], input[146]]),
            evidence_records: u16::from_be_bytes([input[147], input[148]]),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HilEventFrame {
    pub nonce: [u8; 16],
    pub event: EmbeddedEvent<16>,
}

impl HilEventFrame {
    pub const ENCODED_BYTES: usize = 108;

    pub fn encode(&self, output: &mut [u8; Self::ENCODED_BYTES]) -> Result<(), EmbeddedError> {
        output[..4].copy_from_slice(b"CNE0");
        output[4..6].copy_from_slice(&HIL_PROTOCOL_VERSION.to_be_bytes());
        output[6..22].copy_from_slice(&self.nonce);
        output[22..54].copy_from_slice(self.event.plan.as_bytes());
        output[54..70].copy_from_slice(&self.event.run.boot_id);
        output[70..78].copy_from_slice(&self.event.run.run_sequence.to_be_bytes());
        output[78..82].copy_from_slice(&self.event.sequence.to_be_bytes());
        output[82..86].copy_from_slice(&self.event.tick.to_be_bytes());
        let (subject_kind, subject_index) = match self.event.subject {
            EmbeddedSubject::Run => (0, 0),
            EmbeddedSubject::Node(index) => (1, index),
            EmbeddedSubject::Cord(index) => (2, index),
        };
        output[86] = subject_kind;
        output[87..89].copy_from_slice(&subject_index.to_be_bytes());
        output[89] = event_kind_code(self.event.kind);
        let value_length = self.event.value.map_or(0, |value| value.length);
        if value_length > 16 {
            return Err(EmbeddedError::ValueTooLarge);
        }
        output[90..92].copy_from_slice(&value_length.to_be_bytes());
        output[92..108].fill(0);
        if let Some(value) = self.event.value {
            output[92..92 + usize::from(value.length)].copy_from_slice(value.as_slice());
        }
        Ok(())
    }

    pub fn decode(input: &[u8; Self::ENCODED_BYTES]) -> Result<Self, EmbeddedError> {
        if &input[..4] != b"CNE0"
            || u16::from_be_bytes([input[4], input[5]]) != HIL_PROTOCOL_VERSION
        {
            return Err(EmbeddedError::UnsupportedHilProtocol);
        }
        let mut nonce = [0; 16];
        nonce.copy_from_slice(&input[6..22]);
        let mut plan = [0; 32];
        plan.copy_from_slice(&input[22..54]);
        let mut boot_id = [0; 16];
        boot_id.copy_from_slice(&input[54..70]);
        let subject_index = u16::from_be_bytes([input[87], input[88]]);
        let subject = match input[86] {
            0 if subject_index == 0 => EmbeddedSubject::Run,
            1 => EmbeddedSubject::Node(subject_index),
            2 => EmbeddedSubject::Cord(subject_index),
            _ => return Err(EmbeddedError::UnsupportedHilProtocol),
        };
        let value_length = u16::from_be_bytes([input[90], input[91]]);
        if value_length > 16 {
            return Err(EmbeddedError::UnsupportedHilProtocol);
        }
        let value = if value_length == 0 {
            None
        } else {
            Some(
                EmbeddedValue::from_slice(&input[92..92 + usize::from(value_length)])
                    .map_err(|_| EmbeddedError::UnsupportedHilProtocol)?,
            )
        };
        Ok(Self {
            nonce,
            event: EmbeddedEvent {
                plan: SemanticHash::from_bytes(plan),
                run: RunIdentity {
                    boot_id,
                    run_sequence: u64::from_be_bytes([
                        input[70], input[71], input[72], input[73], input[74], input[75],
                        input[76], input[77],
                    ]),
                },
                sequence: u32::from_be_bytes([input[78], input[79], input[80], input[81]]),
                tick: u32::from_be_bytes([input[82], input[83], input[84], input[85]]),
                subject,
                kind: event_kind_from_code(input[89])?,
                value,
            },
        })
    }
}

const fn event_kind_code(kind: EmbeddedEventKind) -> u8 {
    match kind {
        EmbeddedEventKind::AllocationPrepared => 1,
        EmbeddedEventKind::NodePrepared => 2,
        EmbeddedEventKind::RunStarted => 3,
        EmbeddedEventKind::Decision => 4,
        EmbeddedEventKind::ValueAccepted => 5,
        EmbeddedEventKind::ValueConsumed => 6,
        EmbeddedEventKind::PressureEntered => 7,
        EmbeddedEventKind::PressureCleared => 8,
        EmbeddedEventKind::NodeCompleted => 9,
        EmbeddedEventKind::CancellationRequested => 10,
        EmbeddedEventKind::RunSucceeded => 11,
        EmbeddedEventKind::RunCancelled => 12,
        EmbeddedEventKind::RunFailed => 13,
    }
}

fn event_kind_from_code(value: u8) -> Result<EmbeddedEventKind, EmbeddedError> {
    match value {
        1 => Ok(EmbeddedEventKind::AllocationPrepared),
        2 => Ok(EmbeddedEventKind::NodePrepared),
        3 => Ok(EmbeddedEventKind::RunStarted),
        4 => Ok(EmbeddedEventKind::Decision),
        5 => Ok(EmbeddedEventKind::ValueAccepted),
        6 => Ok(EmbeddedEventKind::ValueConsumed),
        7 => Ok(EmbeddedEventKind::PressureEntered),
        8 => Ok(EmbeddedEventKind::PressureCleared),
        9 => Ok(EmbeddedEventKind::NodeCompleted),
        10 => Ok(EmbeddedEventKind::CancellationRequested),
        11 => Ok(EmbeddedEventKind::RunSucceeded),
        12 => Ok(EmbeddedEventKind::RunCancelled),
        13 => Ok(EmbeddedEventKind::RunFailed),
        _ => Err(EmbeddedError::UnsupportedHilProtocol),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedError {
    InvalidProfile,
    ProfileIdentityMismatch,
    InvalidStaticPlan,
    ProfileExceeded,
    UnsupportedFeature,
    ArithmeticOverflow,
    ValueTooLarge,
    InvalidInterest,
    InterestCapacityExceeded,
    InvalidTimer,
    TimerCapacityExceeded,
    PortAccessViolation,
    StepWorkExceeded,
    FalseProgress,
    PrepareFailed,
    StartFailed,
    InvalidRun,
    DecisionLimitExceeded,
    EvidenceCapacityExceeded,
    Stalled,
    NodeFailed,
    ReplacementUnsupported,
    UnsupportedHilProtocol,
    DriverBindingMismatch,
}

impl EmbeddedError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidProfile | Self::ProfileIdentityMismatch => "CND-EMB-001",
            Self::InvalidStaticPlan => "CND-EMB-002",
            Self::ProfileExceeded | Self::UnsupportedFeature => "CND-EMB-003",
            Self::ArithmeticOverflow => "CND-EMB-004",
            Self::ValueTooLarge | Self::PortAccessViolation => "CND-EMB-005",
            Self::InvalidInterest | Self::InterestCapacityExceeded => "CND-EMB-006",
            Self::InvalidTimer | Self::TimerCapacityExceeded => "CND-EMB-007",
            Self::StepWorkExceeded | Self::FalseProgress => "CND-EMB-008",
            Self::PrepareFailed | Self::StartFailed => "CND-EMB-009",
            Self::InvalidRun
            | Self::DecisionLimitExceeded
            | Self::EvidenceCapacityExceeded
            | Self::Stalled => "CND-EMB-010",
            Self::NodeFailed => "CND-EMB-011",
            Self::ReplacementUnsupported => "CND-EMB-012",
            Self::UnsupportedHilProtocol => "CND-EMB-013",
            Self::DriverBindingMismatch => "CND-EMB-014",
        }
    }
}

impl fmt::Display for EmbeddedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidProfile => "embedded execution profile is invalid",
            Self::ProfileIdentityMismatch => "embedded profile identity mismatch",
            Self::InvalidStaticPlan => "compact static plan is invalid",
            Self::ProfileExceeded => "plan exceeds selected profile or caller storage",
            Self::UnsupportedFeature => "plan uses an unsupported embedded feature",
            Self::ArithmeticOverflow => "embedded accounting arithmetic overflowed",
            Self::ValueTooLarge => "fixed value representation is too large",
            Self::InvalidInterest => "pending wake interest is invalid",
            Self::InterestCapacityExceeded => "pending wake interest storage is full",
            Self::InvalidTimer => "timer deadline is invalid or ambiguous across wraparound",
            Self::TimerCapacityExceeded => "fixed timer storage is full",
            Self::PortAccessViolation => "step accessed an unplanned port or queue state",
            Self::StepWorkExceeded => "step exceeded its exact work ceiling",
            Self::FalseProgress => "step reported progress without an observed commit",
            Self::PrepareFailed => "prepare-all failed before start",
            Self::StartFailed => "start-all failed",
            Self::InvalidRun => "run control or driver set is invalid",
            Self::DecisionLimitExceeded => "run decision limit was exhausted",
            Self::EvidenceCapacityExceeded => "fixed normative evidence storage is full",
            Self::Stalled => "no exact wake interest can make progress",
            Self::NodeFailed => "embedded node reported failure",
            Self::ReplacementUnsupported => "firmware replacement level or overlap is unsupported",
            Self::UnsupportedHilProtocol => "unsupported RP2040 HIL protocol",
            Self::DriverBindingMismatch => {
                "firmware driver identity does not match the generated node binding"
            }
        })
    }
}

fn valid_pin(pin: PinnedDescriptor<'_>) -> bool {
    Id::new(pin.id.as_str()).is_ok() && pin.semantic_hash != ZERO_HASH
}

fn semantic<'a>(name: &'static str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn as_u16(value: usize) -> Result<u16, EmbeddedError> {
    u16::try_from(value).map_err(|_| EmbeddedError::ArithmeticOverflow)
}

const fn saturating_u16(value: usize) -> u16 {
    if value > u16::MAX as usize {
        u16::MAX
    } else {
        value as u16
    }
}

const fn saturating_u8(value: usize) -> u8 {
    if value > u8::MAX as usize {
        u8::MAX
    } else {
        value as u8
    }
}
