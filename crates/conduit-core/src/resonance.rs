//! Typed event-stream, retention, subscription, and replay contracts.

use core::fmt;

use crate::{
    ArtifactDigest, EventPayload, EvidencePolicy, ExecutionEvent, FlowPolicy, Id, InstancePath,
    PinnedDescriptor, SemanticHash, Sensitivity, TypeContractRef, validate_execution_event,
};

pub const RESONANCE_CONTRACT_VERSION: u32 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventClass {
    NormativeEvidence,
    Domain,
    Control,
}

impl EventClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NormativeEvidence => "normative-evidence",
            Self::Domain => "domain",
            Self::Control => "control",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionPolicy {
    Ephemeral,
    Ring {
        maximum_events: u16,
        maximum_bytes: u64,
    },
    CheckpointAssociated {
        maximum_events: u16,
        maximum_bytes: u64,
        maximum_checkpoints: u16,
    },
    DurableAppend {
        maximum_events: u64,
        maximum_bytes: u64,
        flush_ticks: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriberCoupling<'a> {
    Coupled(FlowPolicy<'a>),
    Isolated(FlowPolicy<'a>),
}

impl<'a> SubscriberCoupling<'a> {
    pub const fn flow(self) -> FlowPolicy<'a> {
        match self {
            Self::Coupled(flow) | Self::Isolated(flow) => flow,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayDelivery {
    AtMostOnce,
    AtLeastOnce,
}

impl ReplayDelivery {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AtMostOnce => "at-most-once",
            Self::AtLeastOnce => "at-least-once",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventStreamContract<'a> {
    pub id: Id<'a>,
    pub event_class: EventClass,
    pub payload_type: TypeContractRef<'a>,
    pub retention: RetentionPolicy,
    pub subscriber_coupling: SubscriberCoupling<'a>,
    pub delivery: ReplayDelivery,
    pub maximum_publishers: u16,
    pub maximum_subscribers: u16,
    pub maximum_pending_operations: u16,
    pub maximum_projection_bytes: u64,
    pub provider: PinnedDescriptor<'a>,
    pub recording_authority: Option<Id<'a>>,
    pub sensitivity: Sensitivity,
    pub terminal_evidence_required: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventProviderCapabilities {
    pub ephemeral: bool,
    pub retained: bool,
    pub durable: bool,
    pub checkpoint_cursor: bool,
    pub integrity: bool,
    pub redaction: bool,
    pub maximum_events: u64,
    pub maximum_bytes: u64,
    pub maximum_subscribers: u16,
    pub maximum_pending_operations: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventPayloadRef<'a> {
    None,
    InlinePublic { bytes: &'a [u8] },
    ContentAddressed { digest: ArtifactDigest, bytes: u64 },
    Redacted { original_bytes: u64, reason: Id<'a> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResonanceRelations<'a> {
    pub caused_by: Option<Id<'a>>,
    pub derived_from: &'a [Id<'a>],
    pub supersedes: Option<Id<'a>>,
    pub corrects: Option<Id<'a>>,
    pub retracts: Option<Id<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResonanceEnvelope<'a> {
    pub event: Id<'a>,
    pub stream: Id<'a>,
    pub run: Id<'a>,
    pub plan_epoch: SemanticHash,
    pub producer: InstancePath<'a>,
    pub subject: InstancePath<'a>,
    pub class: EventClass,
    pub sequence: u64,
    pub observer: Id<'a>,
    pub observer_sequence: u64,
    pub domain_time: Option<(Id<'a>, i64)>,
    pub correlation: Option<Id<'a>>,
    pub idempotency: Option<Id<'a>>,
    pub payload_type: TypeContractRef<'a>,
    pub payload: EventPayloadRef<'a>,
    pub relations: ResonanceRelations<'a>,
    pub provenance: Id<'a>,
    pub recording_authority: Option<Id<'a>>,
    pub sensitivity: Sensitivity,
    pub integrity: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceStreamExtension<'a> {
    pub stream: Id<'a>,
    pub producer: InstancePath<'a>,
    pub payload_type_when_none: TypeContractRef<'a>,
    pub provenance: Id<'a>,
    pub recording_authority: Option<Id<'a>>,
    pub integrity: SemanticHash,
}

/// Extend current ExecutionEvent v1 without changing its identity or semantics.
pub fn extend_execution_event<'a>(
    event: ExecutionEvent<'a>,
    policy: EvidencePolicy,
    extension: EvidenceStreamExtension<'a>,
) -> Result<ResonanceEnvelope<'a>, ResonanceError> {
    validate_execution_event(&event, policy).map_err(|_| ResonanceError::InvalidEnvelope)?;
    let (payload_type, payload, sensitivity, authority) = match event.payload {
        EventPayload::None => (
            extension.payload_type_when_none,
            EventPayloadRef::None,
            Sensitivity::Public,
            extension.recording_authority,
        ),
        EventPayload::InlinePublic { value_type, bytes } => (
            value_type,
            EventPayloadRef::InlinePublic { bytes },
            Sensitivity::Public,
            extension.recording_authority,
        ),
        EventPayload::Reference {
            value_type,
            digest,
            sensitivity,
            shape,
            recording_authority,
        } => (
            value_type,
            EventPayloadRef::ContentAddressed {
                digest,
                bytes: shape.byte_length.unwrap_or(0),
            },
            sensitivity,
            recording_authority,
        ),
        EventPayload::Redacted {
            value_type,
            sensitivity,
            shape,
            reason,
        } => (
            value_type,
            EventPayloadRef::Redacted {
                original_bytes: shape.byte_length.unwrap_or(0),
                reason,
            },
            sensitivity,
            extension.recording_authority,
        ),
    };
    let value = ResonanceEnvelope {
        event: event.event_id,
        stream: extension.stream,
        run: event.run_id,
        plan_epoch: event.plan_identity,
        producer: extension.producer,
        subject: event.subject,
        class: EventClass::NormativeEvidence,
        sequence: event.sequence,
        observer: event.observer,
        observer_sequence: event.observer_sequence,
        domain_time: event.domain_time.map(|time| (time.basis, time.tick)),
        correlation: event.correlation.correlation,
        idempotency: event.correlation.idempotency,
        payload_type,
        payload,
        relations: ResonanceRelations {
            caused_by: event.relations.caused_by,
            derived_from: event.relations.derived_from,
            supersedes: event.relations.supersedes,
            corrects: None,
            retracts: event.relations.retracts,
        },
        provenance: extension.provenance,
        recording_authority: authority,
        sensitivity,
        integrity: extension.integrity,
    };
    validate_envelope(&value)?;
    Ok(value)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayStart<'a> {
    Head,
    Tail,
    Cursor(u64),
    Checkpoint(Id<'a>),
    ProviderIndex(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubscriptionContract<'a> {
    pub id: Id<'a>,
    pub stream: Id<'a>,
    pub start: ReplayStart<'a>,
    pub queue: FlowPolicy<'a>,
    pub acknowledgement: bool,
    pub maximum_unacknowledged: u16,
    pub cancellation_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionContract<'a> {
    pub id: Id<'a>,
    pub stream: Id<'a>,
    pub logic: PinnedDescriptor<'a>,
    pub snapshot_contract: PinnedDescriptor<'a>,
    pub maximum_state_bytes: u64,
    pub maximum_rebuild_events: u64,
    pub gap_is_terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionSnapshot<'a> {
    pub projection: Id<'a>,
    pub stream: Id<'a>,
    pub logic_hash: SemanticHash,
    pub cursor: u64,
    pub digest: ArtifactDigest,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamEntry<'a> {
    pub cursor: u64,
    pub envelope: ResonanceEnvelope<'a>,
    pub accounted_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppendOutcome {
    Committed { cursor: u64 },
    WouldBlock,
    GapCreated { first_available: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadOutcome<'ring, 'event> {
    Event(&'ring StreamEntry<'event>),
    Gap { first_available: u64 },
    Pending,
    Sealed,
}

pub struct BoundedEventRing<'a, const N: usize> {
    slots: [Option<StreamEntry<'a>>; N],
    head: usize,
    len: usize,
    bytes: u64,
    maximum_bytes: u64,
    next_cursor: u64,
    first_available: u64,
    sealed: bool,
}

impl<'a, const N: usize> BoundedEventRing<'a, N> {
    pub const fn new(maximum_bytes: u64) -> Self {
        Self {
            slots: [None; N],
            head: 0,
            len: 0,
            bytes: 0,
            maximum_bytes,
            next_cursor: 0,
            first_available: 0,
            sealed: false,
        }
    }

    pub fn append(
        &mut self,
        envelope: ResonanceEnvelope<'a>,
        accounted_bytes: u64,
        isolated: bool,
    ) -> Result<AppendOutcome, ResonanceError> {
        validate_envelope(&envelope)?;
        if self.sealed {
            return Err(ResonanceError::Sealed);
        }
        if N == 0 || accounted_bytes == 0 || accounted_bytes > self.maximum_bytes {
            return Err(ResonanceError::Unbounded);
        }
        let mut gap = false;
        while self.len == N
            || self
                .bytes
                .checked_add(accounted_bytes)
                .is_none_or(|value| value > self.maximum_bytes)
        {
            if !isolated {
                return Ok(AppendOutcome::WouldBlock);
            }
            self.pop_oldest();
            gap = true;
        }
        let cursor = self.next_cursor;
        self.next_cursor = cursor.checked_add(1).ok_or(ResonanceError::Unbounded)?;
        let index = (self.head + self.len) % N;
        self.slots[index] = Some(StreamEntry {
            cursor,
            envelope,
            accounted_bytes,
        });
        self.len += 1;
        self.bytes += accounted_bytes;
        if gap {
            Ok(AppendOutcome::GapCreated {
                first_available: self.first_available,
            })
        } else {
            Ok(AppendOutcome::Committed { cursor })
        }
    }

    pub fn read(&self, cursor: u64) -> ReadOutcome<'_, 'a> {
        if cursor < self.first_available {
            return ReadOutcome::Gap {
                first_available: self.first_available,
            };
        }
        if cursor >= self.next_cursor {
            return if self.sealed {
                ReadOutcome::Sealed
            } else {
                ReadOutcome::Pending
            };
        }
        self.slots
            .iter()
            .flatten()
            .find(|entry| entry.cursor == cursor)
            .map_or(
                ReadOutcome::Gap {
                    first_available: self.first_available,
                },
                ReadOutcome::Event,
            )
    }

    pub const fn seal(&mut self) {
        self.sealed = true;
    }

    fn pop_oldest(&mut self) {
        let entry = self.slots[self.head].take().expect("nonempty ring");
        self.bytes -= entry.accounted_bytes;
        self.head = (self.head + 1) % N;
        self.len -= 1;
        self.first_available = entry.cursor + 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppendRecovery {
    DiscardPartial,
    ReplayCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppendCommit {
    prepared: bool,
    committed: bool,
}

impl AppendCommit {
    pub const fn prepare() -> Self {
        Self {
            prepared: true,
            committed: false,
        }
    }

    pub fn commit(&mut self) {
        self.committed = self.prepared;
    }

    pub const fn recover(self) -> AppendRecovery {
        if self.committed {
            AppendRecovery::ReplayCommitted
        } else {
            AppendRecovery::DiscardPartial
        }
    }
}

pub fn validate_stream_contract(
    contract: EventStreamContract<'_>,
    provider: EventProviderCapabilities,
) -> Result<(), ResonanceError> {
    let subscriber_flow = match contract.subscriber_coupling {
        SubscriberCoupling::Coupled(flow) | SubscriberCoupling::Isolated(flow) => flow,
    };
    let (events, bytes, retained, durable, checkpoint) = match contract.retention {
        RetentionPolicy::Ephemeral => (1, 1, false, false, false),
        RetentionPolicy::Ring {
            maximum_events,
            maximum_bytes,
        } => (u64::from(maximum_events), maximum_bytes, true, false, false),
        RetentionPolicy::CheckpointAssociated {
            maximum_events,
            maximum_bytes,
            maximum_checkpoints,
        } => {
            if maximum_checkpoints == 0 {
                return Err(ResonanceError::Unbounded);
            }
            (u64::from(maximum_events), maximum_bytes, true, false, true)
        }
        RetentionPolicy::DurableAppend {
            maximum_events,
            maximum_bytes,
            flush_ticks,
        } => {
            if flush_ticks == 0 {
                return Err(ResonanceError::Unbounded);
            }
            (maximum_events, maximum_bytes, true, true, false)
        }
    };
    if Id::new(contract.id.as_str()).is_err()
        || contract.payload_type.validate().is_err()
        || Id::new(contract.provider.id.as_str()).is_err()
        || contract.provider.schema_version != 0
        || contract
            .recording_authority
            .is_some_and(|id| Id::new(id.as_str()).is_err())
        || (matches!(contract.event_class, EventClass::Control)
            || contract.sensitivity != Sensitivity::Public)
            && contract.recording_authority.is_none()
        || (contract.event_class == EventClass::NormativeEvidence
            && contract.terminal_evidence_required
            && subscriber_flow.pressure.permits_loss())
        || contract.maximum_publishers == 0
        || contract.maximum_subscribers == 0
        || contract.maximum_pending_operations == 0
        || contract.maximum_projection_bytes == 0
        || events == 0
        || bytes == 0
        || provider.maximum_events < events
        || provider.maximum_bytes < bytes
        || provider.maximum_subscribers < contract.maximum_subscribers
        || provider.maximum_pending_operations < contract.maximum_pending_operations
        || (retained && !provider.retained)
        || (durable && (!provider.durable || !provider.integrity))
        || (checkpoint && !provider.checkpoint_cursor)
        || (contract.sensitivity != Sensitivity::Public && !provider.redaction)
        || (!retained && !provider.ephemeral)
    {
        return Err(ResonanceError::ProviderIncapable);
    }
    Ok(())
}

pub fn validate_subscription(value: SubscriptionContract<'_>) -> Result<(), ResonanceError> {
    let start_valid = match value.start {
        ReplayStart::Checkpoint(id) => Id::new(id.as_str()).is_ok(),
        ReplayStart::Head
        | ReplayStart::Tail
        | ReplayStart::Cursor(_)
        | ReplayStart::ProviderIndex(_) => true,
    };
    if Id::new(value.id.as_str()).is_err()
        || Id::new(value.stream.as_str()).is_err()
        || FlowPolicy::new(
            value.queue.capacity,
            value.queue.pressure,
            value.queue.watermarks,
        )
        .is_err()
        || !start_valid
        || value.cancellation_ticks == 0
        || (value.acknowledgement && value.maximum_unacknowledged == 0)
        || (!value.acknowledgement && value.maximum_unacknowledged != 0)
    {
        return Err(ResonanceError::Unbounded);
    }
    Ok(())
}

pub fn validate_projection(value: ProjectionContract<'_>) -> Result<(), ResonanceError> {
    if Id::new(value.id.as_str()).is_err()
        || Id::new(value.stream.as_str()).is_err()
        || Id::new(value.logic.id.as_str()).is_err()
        || value.logic.schema_version != 0
        || Id::new(value.snapshot_contract.id.as_str()).is_err()
        || value.snapshot_contract.schema_version != 0
        || value.maximum_state_bytes == 0
        || value.maximum_rebuild_events == 0
    {
        return Err(ResonanceError::Unbounded);
    }
    Ok(())
}

pub fn validate_projection_snapshot(
    contract: ProjectionContract<'_>,
    snapshot: ProjectionSnapshot<'_>,
) -> Result<(), ResonanceError> {
    validate_projection(contract)?;
    if snapshot.projection != contract.id
        || snapshot.stream != contract.stream
        || snapshot.logic_hash != contract.logic.semantic_hash
        || snapshot.bytes == 0
        || snapshot.bytes > contract.maximum_state_bytes
    {
        return Err(ResonanceError::InvalidEnvelope);
    }
    Ok(())
}

pub fn validate_envelope(value: &ResonanceEnvelope<'_>) -> Result<(), ResonanceError> {
    let revision_count = [
        value.relations.supersedes,
        value.relations.corrects,
        value.relations.retracts,
    ]
    .iter()
    .flatten()
    .count();
    if Id::new(value.event.as_str()).is_err()
        || Id::new(value.stream.as_str()).is_err()
        || Id::new(value.run.as_str()).is_err()
        || Id::new(value.observer.as_str()).is_err()
        || Id::new(value.provenance.as_str()).is_err()
        || value.payload_type.validate().is_err()
        || value
            .relations
            .derived_from
            .iter()
            .any(|id| Id::new(id.as_str()).is_err() || *id == value.event)
        || value
            .relations
            .caused_by
            .is_some_and(|id| id == value.event || Id::new(id.as_str()).is_err())
        || [
            value.relations.supersedes,
            value.relations.corrects,
            value.relations.retracts,
        ]
        .iter()
        .flatten()
        .any(|id| *id == value.event || Id::new(id.as_str()).is_err())
        || revision_count > 1
        || value
            .correlation
            .is_some_and(|id| Id::new(id.as_str()).is_err())
        || value
            .idempotency
            .is_some_and(|id| Id::new(id.as_str()).is_err())
        || value
            .recording_authority
            .is_some_and(|id| Id::new(id.as_str()).is_err())
        || (matches!(value.class, EventClass::Control) || value.sensitivity != Sensitivity::Public)
            && value.recording_authority.is_none()
        || value.integrity == SemanticHash::from_bytes([0; 32])
        || match value.payload {
            EventPayloadRef::None | EventPayloadRef::InlinePublic { .. } => false,
            EventPayloadRef::ContentAddressed { bytes, .. } => bytes == 0,
            EventPayloadRef::Redacted { reason, .. } => Id::new(reason.as_str()).is_err(),
        }
        || matches!(value.payload, EventPayloadRef::InlinePublic { .. })
            && value.sensitivity != Sensitivity::Public
    {
        return Err(ResonanceError::InvalidEnvelope);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResonanceError {
    InvalidEnvelope,
    Unbounded,
    ProviderIncapable,
    Sealed,
}

impl ResonanceError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidEnvelope => "CND-RSN-001",
            Self::Unbounded => "CND-RSN-002",
            Self::ProviderIncapable => "CND-RSN-003",
            Self::Sealed => "CND-RSN-004",
        }
    }
}

impl fmt::Display for ResonanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}
