//! Exact bounded flow policies and an allocator-free reference queue.

use core::fmt;

use crate::{CompatibilityOutcome, Id};

/// Fairness contract for blocked producers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockingFairness {
    /// Producers resume in arrival order.
    Fifo,
}

/// Exact deterministic sampling schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SampleSchedule {
    every: u32,
    offset: u32,
}

impl SampleSchedule {
    /// Selects sequence numbers `offset + n * every`.
    pub const fn new(every: u32, offset: u32) -> Result<Self, FlowPolicyError> {
        if every == 0 || offset >= every {
            return Err(FlowPolicyError::InvalidSampleSchedule);
        }
        Ok(Self { every, offset })
    }

    /// Sampling period.
    #[must_use]
    pub const fn every(self) -> u32 {
        self.every
    }

    /// Selected offset within the period.
    #[must_use]
    pub const fn offset(self) -> u32 {
        self.offset
    }

    const fn selects(self, sequence: u64) -> bool {
        sequence % self.every as u64 == self.offset as u64
    }
}

/// Behavior when an arrival cannot fit within exact cord capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pressure<'a> {
    /// Keep the producer's value pending outside the queue.
    Block(BlockingFairness),
    /// Reject the attempted write without accepting ownership.
    Reject,
    /// Replace one provider-selected queued value under this exact relation.
    Coalesce {
        /// Domain-owned replacement relation.
        relation: Id<'a>,
    },
    /// Admit only values selected by an exact sequence schedule.
    Sample(SampleSchedule),
    /// Drop an incoming value only after the type proves disposability.
    DropDisposable,
    /// Terminate the cord connection.
    Disconnect,
    /// Fail the affected execution scope.
    Fail,
}

impl Pressure<'_> {
    /// Stable external policy spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block(_) => "block",
            Self::Reject => "reject",
            Self::Coalesce { .. } => "coalesce",
            Self::Sample(_) => "sample",
            Self::DropDisposable => "drop-disposable",
            Self::Disconnect => "disconnect",
            Self::Fail => "fail",
        }
    }

    /// Whether the policy can discard or replace accepted semantic values.
    #[must_use]
    pub const fn permits_loss(self) -> bool {
        matches!(
            self,
            Self::Coalesce { .. } | Self::Sample(_) | Self::DropDisposable
        )
    }
}

/// Exact finite queue-accounting limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowCapacity {
    items: u16,
    max_value_bytes: u32,
    max_queued_bytes: u64,
}

impl FlowCapacity {
    /// Constructs positive, non-sentinel item and byte bounds.
    pub const fn new(
        items: u16,
        max_value_bytes: u32,
        max_queued_bytes: u64,
    ) -> Result<Self, FlowPolicyError> {
        if items == 0 || max_value_bytes == 0 || max_queued_bytes == 0 {
            return Err(FlowPolicyError::ZeroCapacity);
        }
        if max_queued_bytes < max_value_bytes as u64 {
            return Err(FlowPolicyError::InconsistentByteCapacity);
        }
        Ok(Self {
            items,
            max_value_bytes,
            max_queued_bytes,
        })
    }

    /// Maximum resident values.
    #[must_use]
    pub const fn items(self) -> u16 {
        self.items
    }

    /// Maximum accounted bytes for one value.
    #[must_use]
    pub const fn max_value_bytes(self) -> u32 {
        self.max_value_bytes
    }

    /// Maximum accounted bytes across all resident values.
    #[must_use]
    pub const fn max_queued_bytes(self) -> u64 {
        self.max_queued_bytes
    }
}

/// Exact pressure-entry and clearance thresholds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowWatermarks {
    low_items: u16,
    high_items: u16,
}

impl FlowWatermarks {
    /// Constructs hysteretic watermarks within capacity.
    pub const fn new(
        low_items: u16,
        high_items: u16,
        capacity: FlowCapacity,
    ) -> Result<Self, FlowPolicyError> {
        if high_items == 0 || high_items > capacity.items || low_items >= high_items {
            return Err(FlowPolicyError::InvalidWatermarks);
        }
        Ok(Self {
            low_items,
            high_items,
        })
    }

    /// Occupancy at or below which pressure clears.
    #[must_use]
    pub const fn low_items(self) -> u16 {
        self.low_items
    }

    /// Occupancy at or above which pressure begins.
    #[must_use]
    pub const fn high_items(self) -> u16 {
        self.high_items
    }
}

/// Exact bounded flow policy recorded in a resolved plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowPolicy<'a> {
    /// Exact finite capacity and accounting bounds.
    pub capacity: FlowCapacity,
    /// Saturation behavior.
    pub pressure: Pressure<'a>,
    /// Exact pressure observation thresholds.
    pub watermarks: FlowWatermarks,
}

impl<'a> FlowPolicy<'a> {
    /// Validates one exact resolved policy.
    pub fn new(
        capacity: FlowCapacity,
        pressure: Pressure<'a>,
        watermarks: FlowWatermarks,
    ) -> Result<Self, FlowPolicyError> {
        if let Pressure::Coalesce { relation } = pressure {
            if Id::new(relation.as_str()).is_err() {
                return Err(FlowPolicyError::InvalidCoalescer);
            }
        }
        Ok(Self {
            capacity,
            pressure,
            watermarks,
        })
    }

    /// Checks type-owned semantic prerequisites without collapsing unknown.
    #[must_use]
    pub fn assess_type_facts(self, facts: FlowTypeFacts<'_>) -> FlowPolicyDecision {
        match self.pressure {
            Pressure::Coalesce { relation } => match facts.coalescers {
                None => {
                    FlowPolicyDecision::indeterminate(FlowPolicyReason::CoalescingProviderRequired)
                }
                Some(relations) if relations.contains(&relation) => {
                    FlowPolicyDecision::compatible()
                }
                Some(_) => FlowPolicyDecision::incompatible(
                    FlowPolicyReason::CoalescingRelationUnavailable,
                ),
            },
            Pressure::DropDisposable => match facts.disposable {
                TraitProof::Proven => FlowPolicyDecision::compatible(),
                TraitProof::Disproven => {
                    FlowPolicyDecision::incompatible(FlowPolicyReason::TypeNotDisposable)
                }
                TraitProof::Indeterminate => FlowPolicyDecision::indeterminate(
                    FlowPolicyReason::DisposabilityProviderRequired,
                ),
            },
            Pressure::Block(_)
            | Pressure::Reject
            | Pressure::Sample(_)
            | Pressure::Disconnect
            | Pressure::Fail => FlowPolicyDecision::compatible(),
        }
    }
}

/// Validation failure for exact policy construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowPolicyError {
    /// An item or byte bound is zero.
    ZeroCapacity,
    /// Total byte capacity cannot hold one maximum-sized value.
    InconsistentByteCapacity,
    /// Watermarks do not form `low < high <= capacity`.
    InvalidWatermarks,
    /// A sampling period is zero or its offset is outside the period.
    InvalidSampleSchedule,
    /// A coalescing relation is not a portable identifier.
    InvalidCoalescer,
}

impl FlowPolicyError {
    /// Stable diagnostic family code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ZeroCapacity => "CND-FLW-001",
            Self::InconsistentByteCapacity
            | Self::InvalidWatermarks
            | Self::InvalidSampleSchedule
            | Self::InvalidCoalescer => "CND-FLW-003",
        }
    }
}

impl fmt::Display for FlowPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroCapacity => "flow capacity must be positive",
            Self::InconsistentByteCapacity => {
                "aggregate byte capacity cannot hold one maximum value"
            }
            Self::InvalidWatermarks => "flow watermarks must satisfy low < high <= capacity",
            Self::InvalidSampleSchedule => {
                "sample schedule requires positive period and offset below period"
            }
            Self::InvalidCoalescer => "coalescer relation is not a portable identifier",
        };
        formatter.write_str(message)
    }
}

/// Provider proof for one required semantic type trait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraitProof {
    /// The exact type descriptor proves the trait.
    Proven,
    /// The exact type descriptor disproves the trait.
    Disproven,
    /// A provider or additional exact fact is unavailable.
    Indeterminate,
}

/// Type-owned facts required by lossy flow-policy resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowTypeFacts<'a> {
    /// Whether every value may be observably discarded.
    pub disposable: TraitProof,
    /// Known exact replacement relations, or `None` when unavailable.
    pub coalescers: Option<&'a [Id<'a>]>,
}

/// Stable flow-policy assessment reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowPolicyReason {
    /// No additional type fact is needed or every prerequisite is proven.
    Accepted,
    /// The coalescing relation is not declared by the type.
    CoalescingRelationUnavailable,
    /// The owning provider is required to enumerate coalescing relations.
    CoalescingProviderRequired,
    /// The type explicitly forbids disposable loss.
    TypeNotDisposable,
    /// The owning provider is required to prove disposability.
    DisposabilityProviderRequired,
}

impl FlowPolicyReason {
    /// Stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "flow-policy-accepted",
            Self::CoalescingRelationUnavailable => "coalescing-relation-unavailable",
            Self::CoalescingProviderRequired => "coalescing-provider-required",
            Self::TypeNotDisposable => "type-not-disposable",
            Self::DisposabilityProviderRequired => "disposability-provider-required",
        }
    }
}

/// Three-outcome policy/type resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowPolicyDecision {
    /// Compatible, incompatible, or indeterminate.
    pub outcome: CompatibilityOutcome,
    /// Stable explanation.
    pub reason: FlowPolicyReason,
}

impl FlowPolicyDecision {
    const fn compatible() -> Self {
        Self {
            outcome: CompatibilityOutcome::Compatible,
            reason: FlowPolicyReason::Accepted,
        }
    }

    const fn incompatible(reason: FlowPolicyReason) -> Self {
        Self {
            outcome: CompatibilityOutcome::Incompatible,
            reason,
        }
    }

    const fn indeterminate(reason: FlowPolicyReason) -> Self {
        Self {
            outcome: CompatibilityOutcome::Indeterminate,
            reason,
        }
    }
}

/// Queue lifecycle relevant to flow transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowQueueState {
    /// Accepting policy-governed offers.
    Active,
    /// Pressure policy disconnected the cord.
    Disconnected,
    /// Pressure policy failed the cord/run scope.
    Failed,
    /// Cancellation woke blocked participants.
    Cancelled,
}

/// One immutable flow observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowEvent {
    /// Monotonic sequence within this queue.
    pub sequence: u64,
    /// Exact transition or decision.
    pub kind: FlowEventKind,
    /// Resident item count after the transition.
    pub occupancy_items: u16,
    /// Accounted resident bytes after the transition.
    pub occupancy_bytes: u64,
}

/// Flow evidence kinds emitted by the reference queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowEventKind {
    /// High watermark or a byte/item limit was reached.
    PressureEntered,
    /// Occupancy fell to the low watermark.
    PressureCleared,
    /// An attempted value was rejected before acceptance.
    ValueRejected,
    /// A queued value was replaced under the named policy.
    ValueCoalesced {
        /// Logical queue index replaced, starting at the head.
        target: u16,
    },
    /// The exact sample schedule ignored an arrival.
    ValueSampledOut,
    /// A proven-disposable arrival was dropped.
    ValueDroppedDisposable,
    /// One waiting consumer can retry.
    ConsumerReady,
    /// One blocked producer can retry.
    ProducerReady,
    /// The pressure policy disconnected the cord.
    Disconnected,
    /// The pressure policy failed the cord/run.
    Failed,
    /// Cancellation woke all tracked endpoints.
    Cancelled {
        /// A producer was blocked.
        wake_producer: bool,
        /// A consumer was waiting.
        wake_consumer: bool,
    },
}

/// At most two evidence events emitted by one atomic queue transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowEvents {
    events: [Option<FlowEvent>; 2],
    len: u8,
}

impl FlowEvents {
    const fn new() -> Self {
        Self {
            events: [None, None],
            len: 0,
        }
    }

    fn push(&mut self, event: FlowEvent) {
        let index = usize::from(self.len);
        self.events[index] = Some(event);
        self.len += 1;
    }

    /// Iterates in exact emission order.
    pub fn iter(&self) -> impl Iterator<Item = &FlowEvent> {
        self.events[..usize::from(self.len)]
            .iter()
            .map(|event| event.as_ref().expect("occupied event prefix"))
    }
}

/// Per-arrival facts produced by the exact sampling/coalescing implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowOffer {
    /// Accounted bytes for this exact value.
    pub size_bytes: u32,
    /// Logical queued target selected by the declared coalescer.
    pub coalesce_target: Option<u16>,
}

/// Result of offering one value without hidden overflow storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OfferDisposition<T> {
    /// Value is resident in the bounded queue.
    Enqueued,
    /// Caller retains the value until the producer is woken.
    Pending(T),
    /// Caller retains a rejected value.
    Rejected(T),
    /// New value is resident and the replaced value is returned.
    Coalesced {
        /// Replaced queued value.
        replaced: T,
    },
    /// Incoming value was observably discarded.
    Dropped(T),
    /// Incoming value was not accepted because the cord disconnected.
    Disconnected(T),
    /// Incoming value was not accepted because the scope failed.
    Failed(T),
    /// Queue was already terminal.
    Terminated(T),
}

/// Atomic offer result and ordered evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferTransition<T> {
    /// Ownership and queue outcome.
    pub disposition: OfferDisposition<T>,
    /// Complete evidence from the transition.
    pub events: FlowEvents,
}

/// Atomic pop result and ordered evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopTransition<T> {
    /// Oldest queued value, when present.
    pub value: Option<T>,
    /// Complete evidence from the transition.
    pub events: FlowEvents,
}

/// Allocator-free reference queue over caller-provided slots.
pub struct BoundedFlowQueue<'s, 'p, T> {
    slots: &'s mut [Option<(T, u32)>],
    policy: FlowPolicy<'p>,
    head: usize,
    len: u16,
    queued_bytes: u64,
    arrival_sequence: u64,
    event_sequence: u64,
    pressured: bool,
    producer_waiting: bool,
    consumer_waiting: bool,
    state: FlowQueueState,
}

impl<'s, 'p, T> BoundedFlowQueue<'s, 'p, T> {
    /// Binds exact policy capacity to fixed caller storage.
    pub fn new(
        slots: &'s mut [Option<(T, u32)>],
        policy: FlowPolicy<'p>,
        type_facts: FlowTypeFacts<'_>,
    ) -> Result<Self, QueueError> {
        match policy.assess_type_facts(type_facts).outcome {
            CompatibilityOutcome::Compatible => {}
            CompatibilityOutcome::Incompatible => return Err(QueueError::PolicyIncompatible),
            CompatibilityOutcome::Indeterminate => return Err(QueueError::PolicyIndeterminate),
        }
        if slots.len() < usize::from(policy.capacity.items) {
            return Err(QueueError::StorageTooSmall);
        }
        for slot in &mut slots[..usize::from(policy.capacity.items)] {
            if slot.is_some() {
                return Err(QueueError::StorageNotEmpty);
            }
        }
        Ok(Self {
            slots,
            policy,
            head: 0,
            len: 0,
            queued_bytes: 0,
            arrival_sequence: 0,
            event_sequence: 0,
            pressured: false,
            producer_waiting: false,
            consumer_waiting: false,
            state: FlowQueueState::Active,
        })
    }

    /// Resident item count.
    #[must_use]
    pub const fn occupancy_items(&self) -> u16 {
        self.len
    }

    /// Accounted resident bytes.
    #[must_use]
    pub const fn occupancy_bytes(&self) -> u64 {
        self.queued_bytes
    }

    /// Current terminal/active state.
    #[must_use]
    pub const fn state(&self) -> FlowQueueState {
        self.state
    }

    /// Offers one value under the exact policy.
    pub fn offer(&mut self, value: T, offer: FlowOffer) -> OfferTransition<T> {
        let mut events = FlowEvents::new();
        if self.state != FlowQueueState::Active {
            return OfferTransition {
                disposition: OfferDisposition::Terminated(value),
                events,
            };
        }

        let arrival = self.arrival_sequence;
        self.arrival_sequence = self.arrival_sequence.wrapping_add(1);
        if let Pressure::Sample(schedule) = self.policy.pressure {
            if !schedule.selects(arrival) {
                self.emit(&mut events, FlowEventKind::ValueSampledOut);
                return OfferTransition {
                    disposition: OfferDisposition::Dropped(value),
                    events,
                };
            }
        }

        if offer.size_bytes > self.policy.capacity.max_value_bytes {
            self.emit(&mut events, FlowEventKind::ValueRejected);
            return OfferTransition {
                disposition: OfferDisposition::Rejected(value),
                events,
            };
        }

        let fits_items = self.len < self.policy.capacity.items;
        let fits_bytes = self
            .queued_bytes
            .checked_add(u64::from(offer.size_bytes))
            .is_some_and(|bytes| bytes <= self.policy.capacity.max_queued_bytes);
        if fits_items && fits_bytes {
            self.push_back(value, offer.size_bytes);
            if self.len >= self.policy.watermarks.high_items && !self.pressured {
                self.pressured = true;
                self.emit(&mut events, FlowEventKind::PressureEntered);
            }
            if self.consumer_waiting {
                self.consumer_waiting = false;
                self.emit(&mut events, FlowEventKind::ConsumerReady);
            }
            return OfferTransition {
                disposition: OfferDisposition::Enqueued,
                events,
            };
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
                let Some(target) = offer.coalesce_target else {
                    self.emit(&mut events, FlowEventKind::ValueRejected);
                    return OfferTransition {
                        disposition: OfferDisposition::Rejected(value),
                        events,
                    };
                };
                let target = usize::from(target);
                if target >= usize::from(self.len) {
                    self.emit(&mut events, FlowEventKind::ValueRejected);
                    return OfferTransition {
                        disposition: OfferDisposition::Rejected(value),
                        events,
                    };
                }
                let slot = (self.head + target) % usize::from(self.policy.capacity.items);
                let old_bytes = self.slots[slot]
                    .as_ref()
                    .map(|(_, bytes)| *bytes)
                    .expect("logical target is occupied");
                let new_bytes =
                    self.queued_bytes - u64::from(old_bytes) + u64::from(offer.size_bytes);
                if new_bytes > self.policy.capacity.max_queued_bytes {
                    self.emit(&mut events, FlowEventKind::ValueRejected);
                    return OfferTransition {
                        disposition: OfferDisposition::Rejected(value),
                        events,
                    };
                }
                let (replaced, _) = self.slots[slot]
                    .replace((value, offer.size_bytes))
                    .expect("logical target is occupied");
                self.queued_bytes = new_bytes;
                self.emit(
                    &mut events,
                    FlowEventKind::ValueCoalesced {
                        target: target as u16,
                    },
                );
                OfferDisposition::Coalesced { replaced }
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
        OfferTransition {
            disposition,
            events,
        }
    }

    /// Removes the oldest value and wakes a blocked producer when applicable.
    pub fn pop(&mut self) -> PopTransition<T> {
        let mut events = FlowEvents::new();
        if self.len == 0 {
            if self.state == FlowQueueState::Active {
                self.consumer_waiting = true;
            }
            return PopTransition {
                value: None,
                events,
            };
        }
        let (value, bytes) = self.slots[self.head]
            .take()
            .expect("queue head is occupied");
        self.head = (self.head + 1) % usize::from(self.policy.capacity.items);
        self.len -= 1;
        self.queued_bytes -= u64::from(bytes);
        if self.pressured && self.len <= self.policy.watermarks.low_items {
            self.pressured = false;
            self.emit(&mut events, FlowEventKind::PressureCleared);
        }
        if self.producer_waiting {
            self.producer_waiting = false;
            self.emit(&mut events, FlowEventKind::ProducerReady);
        }
        PopTransition {
            value: Some(value),
            events,
        }
    }

    /// Cancels the queue and returns explicit wake evidence.
    pub fn cancel(&mut self) -> FlowEvents {
        let mut events = FlowEvents::new();
        if self.state == FlowQueueState::Active {
            self.state = FlowQueueState::Cancelled;
            let kind = FlowEventKind::Cancelled {
                wake_producer: self.producer_waiting,
                wake_consumer: self.consumer_waiting,
            };
            self.producer_waiting = false;
            self.consumer_waiting = false;
            self.emit(&mut events, kind);
        }
        events
    }

    fn push_back(&mut self, value: T, bytes: u32) {
        let slot = (self.head + usize::from(self.len)) % usize::from(self.policy.capacity.items);
        debug_assert!(self.slots[slot].is_none());
        self.slots[slot] = Some((value, bytes));
        self.len += 1;
        self.queued_bytes += u64::from(bytes);
    }

    fn emit(&mut self, events: &mut FlowEvents, kind: FlowEventKind) {
        events.push(FlowEvent {
            sequence: self.event_sequence,
            kind,
            occupancy_items: self.len,
            occupancy_bytes: self.queued_bytes,
        });
        self.event_sequence = self.event_sequence.wrapping_add(1);
    }
}

/// Fixed-storage queue construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueError {
    /// Exact type facts disprove the selected policy.
    PolicyIncompatible,
    /// Exact type facts are unavailable for the selected policy.
    PolicyIndeterminate,
    /// Caller storage has fewer slots than exact plan capacity.
    StorageTooSmall,
    /// Caller storage was not empty.
    StorageNotEmpty,
}
