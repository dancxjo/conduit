//! Immutable execution-event envelopes and append-only stream validation.

use core::convert::Infallible;
use core::fmt;

use crate::{
    ArtifactDigest, CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition, Id,
    InstancePath, MapField, SemanticHash, Sensitivity, TerminalClass, TypeContractRef,
};

/// Exact event-envelope schema supported by the v1 validator.
pub const EXECUTION_EVENT_SCHEMA_VERSION: u32 = 1;

/// Maximum direct derivation inputs in the allocator-free v1 envelope.
pub const MAX_EVENT_DERIVATIONS: usize = 16;

/// Clock category. Its named basis is carried separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventTimeKind {
    Monotonic,
    Wall,
    Domain,
}

impl EventTimeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Monotonic => "monotonic",
            Self::Wall => "wall",
            Self::Domain => "domain",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "monotonic" => Some(Self::Monotonic),
            "wall" => Some(Self::Wall),
            "domain" => Some(Self::Domain),
            _ => None,
        }
    }
}

/// One explicitly based time observation. Time never implies causality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventTime<'a> {
    pub kind: EventTimeKind,
    pub basis: Id<'a>,
    pub tick: i64,
}

/// Stable core evidence families. Domain detail remains an open identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionEventKind {
    Lifecycle,
    CordOccupancy,
    Pressure,
    ValueAccepted,
    ValueRejected,
    ValueDropped,
    ValueCoalesced,
    Cancellation,
    Terminal,
    Resource,
    Authority,
    Checkpoint,
    Progress,
    Derivation,
    Domain,
    Correction,
    Retraction,
}

impl ExecutionEventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lifecycle => "lifecycle",
            Self::CordOccupancy => "cord-occupancy",
            Self::Pressure => "pressure",
            Self::ValueAccepted => "value-accepted",
            Self::ValueRejected => "value-rejected",
            Self::ValueDropped => "value-dropped",
            Self::ValueCoalesced => "value-coalesced",
            Self::Cancellation => "cancellation",
            Self::Terminal => "terminal",
            Self::Resource => "resource",
            Self::Authority => "authority",
            Self::Checkpoint => "checkpoint",
            Self::Progress => "progress",
            Self::Derivation => "derivation",
            Self::Domain => "domain",
            Self::Correction => "correction",
            Self::Retraction => "retraction",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "lifecycle" => Some(Self::Lifecycle),
            "cord-occupancy" => Some(Self::CordOccupancy),
            "pressure" => Some(Self::Pressure),
            "value-accepted" => Some(Self::ValueAccepted),
            "value-rejected" => Some(Self::ValueRejected),
            "value-dropped" => Some(Self::ValueDropped),
            "value-coalesced" => Some(Self::ValueCoalesced),
            "cancellation" => Some(Self::Cancellation),
            "terminal" => Some(Self::Terminal),
            "resource" => Some(Self::Resource),
            "authority" => Some(Self::Authority),
            "checkpoint" => Some(Self::Checkpoint),
            "progress" => Some(Self::Progress),
            "derivation" => Some(Self::Derivation),
            "domain" => Some(Self::Domain),
            "correction" => Some(Self::Correction),
            "retraction" => Some(Self::Retraction),
            _ => None,
        }
    }
}

/// Immutable correlation context propagated across execution boundaries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventCorrelation<'a> {
    pub request: Option<Id<'a>>,
    pub exchange: Option<Id<'a>>,
    pub session: Option<Id<'a>>,
    pub epoch: Option<u32>,
    pub work_unit: Option<Id<'a>>,
    pub attempt: Option<Id<'a>>,
    pub correlation: Option<Id<'a>>,
    pub idempotency: Option<Id<'a>>,
    pub checkpoint: Option<Id<'a>>,
    pub transport: Option<Id<'a>>,
}

/// Causal and append-only correction relationships.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventRelations<'a> {
    pub caused_by: Option<Id<'a>>,
    pub derived_from: &'a [Id<'a>],
    pub supersedes: Option<Id<'a>>,
    pub retracts: Option<Id<'a>>,
}

/// Optional shape metadata allowed after payload redaction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventPayloadShape {
    pub byte_length: Option<u64>,
    pub item_count: Option<u64>,
}

/// Typed payload material or a safe reference/redaction.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum EventPayload<'a> {
    None,
    InlinePublic {
        value_type: TypeContractRef<'a>,
        bytes: &'a [u8],
    },
    Reference {
        value_type: TypeContractRef<'a>,
        digest: ArtifactDigest,
        sensitivity: Sensitivity,
        shape: EventPayloadShape,
        recording_authority: Option<Id<'a>>,
    },
    Redacted {
        value_type: TypeContractRef<'a>,
        sensitivity: Sensitivity,
        shape: EventPayloadShape,
        reason: Id<'a>,
    },
}

impl fmt::Debug for EventPayload<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::InlinePublic { value_type, bytes } => formatter
                .debug_struct("InlinePublic")
                .field("value_type", value_type)
                .field("bytes", bytes)
                .finish(),
            Self::Reference {
                value_type,
                digest,
                sensitivity,
                shape,
                recording_authority,
            } => {
                let digest: &dyn fmt::Debug = if *sensitivity == Sensitivity::Public {
                    digest
                } else {
                    &"<redacted>"
                };
                formatter
                    .debug_struct("Reference")
                    .field("value_type", value_type)
                    .field("digest", digest)
                    .field("sensitivity", sensitivity)
                    .field("shape", shape)
                    .field("recording_authority", recording_authority)
                    .finish()
            }
            Self::Redacted {
                value_type,
                sensitivity,
                shape,
                reason,
            } => formatter
                .debug_struct("Redacted")
                .field("value_type", value_type)
                .field("sensitivity", sensitivity)
                .field("shape", shape)
                .field("reason", reason)
                .finish(),
        }
    }
}

/// Whether this event ends the exact run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventTerminality<'a> {
    NonTerminal,
    Terminal { class: TerminalClass, cause: Id<'a> },
}

/// Versioned immutable normative event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionEvent<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub event_id: Id<'a>,
    pub run_id: Id<'a>,
    pub plan_identity: SemanticHash,
    pub sequence: u64,
    pub recorder: Id<'a>,
    pub observer: Id<'a>,
    pub observer_sequence: u64,
    pub logical_template: Option<InstancePath<'a>>,
    pub subject: InstancePath<'a>,
    pub kind: ExecutionEventKind,
    pub detail: Id<'a>,
    pub observed_time: EventTime<'a>,
    pub domain_time: Option<EventTime<'a>>,
    pub correlation: EventCorrelation<'a>,
    pub relations: EventRelations<'a>,
    pub terminality: EventTerminality<'a>,
    pub payload: EventPayload<'a>,
}

/// Exact recorder policy relevant to portable event validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidencePolicy {
    pub max_inline_payload_bytes: u32,
    pub reveal_redacted_byte_length: bool,
    pub reveal_redacted_item_count: bool,
}

/// Stable event/stream validation reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceReason {
    UnsupportedVersion,
    InvalidDescriptor,
    IdentityMismatch,
    InlinePayloadTooLarge,
    ProtectedPayloadUnrecordable,
    InvalidRedaction,
    DerivationLimitExceeded,
    DuplicateIdentity,
    RunOrPlanMismatch,
    SequenceViolation,
    ObserverSequenceViolation,
    CausalReferenceMissing,
    CorrectionTargetInvalid,
    TerminalOrderViolation,
}

impl EvidenceReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-EVD-001",
            Self::InvalidDescriptor => "CND-EVD-002",
            Self::IdentityMismatch => "CND-EVD-003",
            Self::InlinePayloadTooLarge => "CND-EVD-004",
            Self::ProtectedPayloadUnrecordable | Self::InvalidRedaction => "CND-EVD-005",
            Self::DerivationLimitExceeded => "CND-EVD-006",
            Self::DuplicateIdentity => "CND-EVD-007",
            Self::RunOrPlanMismatch => "CND-EVD-008",
            Self::SequenceViolation | Self::ObserverSequenceViolation => "CND-EVD-009",
            Self::CausalReferenceMissing | Self::CorrectionTargetInvalid => "CND-EVD-010",
            Self::TerminalOrderViolation => "CND-EVD-011",
        }
    }
}

/// First deterministic evidence validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceError {
    pub reason: EvidenceReason,
    pub event_index: Option<u32>,
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.reason.code())?;
        if let Some(index) = self.event_index {
            write!(formatter, " at event {index}")?;
        }
        Ok(())
    }
}

impl ExecutionEvent<'_> {
    /// Canonical semantic identity. Encoding bytes and presentation are absent.
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        if self.relations.derived_from.len() > MAX_EVENT_DERIVATIONS {
            return Err(CanonicalError::LengthOverflow);
        }
        let mut derivations = [CanonicalValue::Null; MAX_EVENT_DERIVATIONS];
        for (index, event) in self.relations.derived_from.iter().enumerate() {
            derivations[index] = CanonicalValue::Identifier(*event);
        }
        let observed = time_fields(self.observed_time);
        let domain = self.domain_time.map(time_fields);
        let domain_value = match &domain {
            Some(fields) => CanonicalValue::Map(fields),
            None => CanonicalValue::Null,
        };
        let correlation = correlation_fields(self.correlation);
        let relations = [
            semantic("caused_by", optional_id(self.relations.caused_by)),
            semantic(
                "derived_from",
                CanonicalValue::Set(&derivations[..self.relations.derived_from.len()]),
            ),
            semantic("supersedes", optional_id(self.relations.supersedes)),
            semantic("retracts", optional_id(self.relations.retracts)),
        ];
        let terminality = terminality_fields(self.terminality);
        let payload = payload_fields(&self.payload);
        let fields = [
            semantic("event_id", CanonicalValue::Identifier(self.event_id)),
            semantic("run_id", CanonicalValue::Identifier(self.run_id)),
            semantic(
                "plan_identity",
                CanonicalValue::Bytes(self.plan_identity.as_bytes()),
            ),
            semantic(
                "sequence",
                CanonicalValue::Integer(i128::from(self.sequence)),
            ),
            semantic("recorder", CanonicalValue::Identifier(self.recorder)),
            semantic("observer", CanonicalValue::Identifier(self.observer)),
            semantic(
                "observer_sequence",
                CanonicalValue::Integer(i128::from(self.observer_sequence)),
            ),
            semantic(
                "logical_template",
                match self.logical_template {
                    Some(path) => CanonicalValue::Text(path.as_str()),
                    None => CanonicalValue::Null,
                },
            ),
            semantic("subject", CanonicalValue::Text(self.subject.as_str())),
            semantic("kind", CanonicalValue::Identifier(Id(self.kind.as_str()))),
            semantic("detail", CanonicalValue::Identifier(self.detail)),
            semantic("observed_time", CanonicalValue::Map(&observed)),
            semantic("domain_time", domain_value),
            semantic("correlation", CanonicalValue::Map(&correlation)),
            semantic("relations", CanonicalValue::Map(&relations)),
            semantic("terminality", CanonicalValue::Map(&terminality)),
            semantic("payload", CanonicalValue::Map(&payload)),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/execution-event"),
            schema_version: self.schema_version,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

/// Validate one event without consulting mutable state or presentation.
pub fn validate_execution_event(
    event: &ExecutionEvent<'_>,
    policy: EvidencePolicy,
) -> Result<(), EvidenceError> {
    if event.schema_version != EXECUTION_EVENT_SCHEMA_VERSION {
        return Err(failure(EvidenceReason::UnsupportedVersion, None));
    }
    if event.relations.derived_from.len() > MAX_EVENT_DERIVATIONS {
        return Err(failure(EvidenceReason::DerivationLimitExceeded, None));
    }
    if !valid_id(event.event_id)
        || !valid_id(event.run_id)
        || !valid_id(event.recorder)
        || !valid_id(event.observer)
        || !valid_id(event.detail)
        || !valid_path(event.subject)
        || event.logical_template.is_some_and(|template| {
            !valid_path(template) || !path_contains(template.as_str(), event.subject.as_str())
        })
        || !valid_time(event.observed_time)
        || event.observed_time.kind == EventTimeKind::Domain
        || event
            .domain_time
            .is_some_and(|time| !valid_time(time) || time.kind != EventTimeKind::Domain)
        || !valid_correlation(event.correlation)
        || !valid_relations(event.event_id, event.relations)
    {
        return Err(failure(EvidenceReason::InvalidDescriptor, None));
    }
    if !terminality_matches(event.kind, event.terminality) {
        return Err(failure(EvidenceReason::TerminalOrderViolation, None));
    }
    if event.relations.supersedes.is_some() != (event.kind == ExecutionEventKind::Correction)
        || event.relations.retracts.is_some() != (event.kind == ExecutionEventKind::Retraction)
    {
        return Err(failure(EvidenceReason::CorrectionTargetInvalid, None));
    }
    match event.payload {
        EventPayload::None => {}
        EventPayload::InlinePublic { value_type, bytes } => {
            if !valid_type(value_type) {
                return Err(failure(EvidenceReason::InvalidDescriptor, None));
            }
            if bytes.len() > policy.max_inline_payload_bytes as usize {
                return Err(failure(EvidenceReason::InlinePayloadTooLarge, None));
            }
        }
        EventPayload::Reference {
            value_type,
            sensitivity,
            recording_authority,
            ..
        } => {
            if !valid_type(value_type)
                || recording_authority.is_some_and(|authority| !valid_id(authority))
            {
                return Err(failure(EvidenceReason::InvalidDescriptor, None));
            }
            if sensitivity != Sensitivity::Public && recording_authority.is_none() {
                return Err(failure(EvidenceReason::ProtectedPayloadUnrecordable, None));
            }
        }
        EventPayload::Redacted {
            value_type,
            sensitivity,
            shape,
            reason,
        } => {
            if !valid_type(value_type)
                || sensitivity == Sensitivity::Public
                || !valid_id(reason)
                || (!policy.reveal_redacted_byte_length && shape.byte_length.is_some())
                || (!policy.reveal_redacted_item_count && shape.item_count.is_some())
            {
                return Err(failure(EvidenceReason::InvalidRedaction, None));
            }
        }
    }
    if event.semantic_hash().ok() != Some(event.identity) {
        return Err(failure(EvidenceReason::IdentityMismatch, None));
    }
    Ok(())
}

/// Validate a complete append-only replay without ordering by timestamps.
pub fn validate_event_stream(
    events: &[ExecutionEvent<'_>],
    policy: EvidencePolicy,
) -> Result<(), EvidenceError> {
    let Some(first) = events.first() else {
        return Ok(());
    };
    if first.sequence != 0 {
        return Err(indexed(EvidenceReason::SequenceViolation, 0));
    }
    for (index, event) in events.iter().enumerate() {
        validate_execution_event(event, policy)
            .map_err(|error| failure(error.reason, u32::try_from(index).ok()))?;
        if event.run_id != first.run_id
            || event.plan_identity != first.plan_identity
            || event.recorder != first.recorder
        {
            return Err(indexed(EvidenceReason::RunOrPlanMismatch, index));
        }
        if index > 0 && events[index - 1].sequence.checked_add(1) != Some(event.sequence) {
            return Err(indexed(EvidenceReason::SequenceViolation, index));
        }
        if events[..index]
            .iter()
            .any(|prior| prior.event_id == event.event_id || prior.identity == event.identity)
        {
            return Err(indexed(EvidenceReason::DuplicateIdentity, index));
        }
        let prior_observer = events[..index]
            .iter()
            .rev()
            .find(|prior| prior.observer == event.observer);
        match prior_observer {
            Some(prior)
                if prior.observer_sequence.checked_add(1) != Some(event.observer_sequence) =>
            {
                return Err(indexed(EvidenceReason::ObserverSequenceViolation, index));
            }
            None if event.observer_sequence != 0 => {
                return Err(indexed(EvidenceReason::ObserverSequenceViolation, index));
            }
            Some(_) | None => {}
        }
        if index + 1 < events.len()
            && matches!(event.terminality, EventTerminality::Terminal { .. })
        {
            return Err(indexed(EvidenceReason::TerminalOrderViolation, index));
        }
    }

    for (index, event) in events.iter().enumerate() {
        for reference in event
            .relations
            .caused_by
            .iter()
            .chain(event.relations.derived_from)
        {
            if !events
                .iter()
                .any(|candidate| candidate.event_id == *reference)
            {
                return Err(indexed(EvidenceReason::CausalReferenceMissing, index));
            }
        }
        for target in event
            .relations
            .supersedes
            .iter()
            .chain(event.relations.retracts.iter())
        {
            if !events[..index]
                .iter()
                .any(|candidate| candidate.event_id == *target)
            {
                return Err(indexed(EvidenceReason::CorrectionTargetInvalid, index));
            }
        }
    }
    Ok(())
}

fn terminality_matches(kind: ExecutionEventKind, terminality: EventTerminality<'_>) -> bool {
    match terminality {
        EventTerminality::NonTerminal => kind != ExecutionEventKind::Terminal,
        EventTerminality::Terminal { cause, .. } => {
            kind == ExecutionEventKind::Terminal && valid_id(cause)
        }
    }
}

fn valid_relations(event_id: Id<'_>, relations: EventRelations<'_>) -> bool {
    if relations.derived_from.len() > MAX_EVENT_DERIVATIONS
        || relations
            .derived_from
            .iter()
            .enumerate()
            .any(|(index, relation)| {
                !valid_id(*relation)
                    || *relation == event_id
                    || relations.derived_from[..index].contains(relation)
            })
    {
        return false;
    }
    for relation in [
        relations.caused_by,
        relations.supersedes,
        relations.retracts,
    ]
    .into_iter()
    .flatten()
    {
        if !valid_id(relation) || relation == event_id {
            return false;
        }
    }
    !(relations.supersedes.is_some() && relations.retracts.is_some())
}

fn valid_correlation(value: EventCorrelation<'_>) -> bool {
    let identities = [
        value.request,
        value.exchange,
        value.session,
        value.work_unit,
        value.attempt,
        value.correlation,
        value.idempotency,
        value.checkpoint,
        value.transport,
    ];
    if identities
        .into_iter()
        .flatten()
        .any(|identity| !valid_id(identity))
        || value.epoch.is_some() && value.session.is_none()
        || value.attempt.is_some() && value.work_unit.is_none()
    {
        return false;
    }
    for (index, identity) in identities.iter().enumerate() {
        let Some(identity) = identity else {
            continue;
        };
        if identities[..index]
            .iter()
            .flatten()
            .any(|prior| prior == identity)
        {
            return false;
        }
    }
    true
}

fn valid_time(time: EventTime<'_>) -> bool {
    valid_id(time.basis)
}

fn valid_type(value: TypeContractRef<'_>) -> bool {
    valid_id(value.contract_id) && value.schema_version > 0
}

fn valid_id(value: Id<'_>) -> bool {
    Id::new(value.as_str()).is_ok()
}

fn valid_path(value: InstancePath<'_>) -> bool {
    InstancePath::new(value.as_str()).is_ok()
}

fn path_contains(template: &str, subject: &str) -> bool {
    subject == template
        || subject
            .strip_prefix(template)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

const fn failure(reason: EvidenceReason, event_index: Option<u32>) -> EvidenceError {
    EvidenceError {
        reason,
        event_index,
    }
}

fn indexed(reason: EvidenceReason, index: usize) -> EvidenceError {
    failure(reason, u32::try_from(index).ok())
}

fn time_fields(value: EventTime<'_>) -> [MapField<'_>; 3] {
    [
        semantic("kind", CanonicalValue::Identifier(Id(value.kind.as_str()))),
        semantic("basis", CanonicalValue::Identifier(value.basis)),
        semantic("tick", CanonicalValue::Integer(i128::from(value.tick))),
    ]
}

fn correlation_fields(value: EventCorrelation<'_>) -> [MapField<'_>; 10] {
    [
        semantic("request", optional_id(value.request)),
        semantic("exchange", optional_id(value.exchange)),
        semantic("session", optional_id(value.session)),
        semantic(
            "epoch",
            value.epoch.map_or(CanonicalValue::Null, |epoch| {
                CanonicalValue::Integer(i128::from(epoch))
            }),
        ),
        semantic("work_unit", optional_id(value.work_unit)),
        semantic("attempt", optional_id(value.attempt)),
        semantic("correlation", optional_id(value.correlation)),
        semantic("idempotency", optional_id(value.idempotency)),
        semantic("checkpoint", optional_id(value.checkpoint)),
        semantic("transport", optional_id(value.transport)),
    ]
}

fn terminality_fields(value: EventTerminality<'_>) -> [MapField<'_>; 3] {
    match value {
        EventTerminality::NonTerminal => [
            semantic("terminal", CanonicalValue::Boolean(false)),
            semantic("class", CanonicalValue::Null),
            semantic("cause", CanonicalValue::Null),
        ],
        EventTerminality::Terminal { class, cause } => [
            semantic("terminal", CanonicalValue::Boolean(true)),
            semantic(
                "class",
                CanonicalValue::Identifier(Id(terminal_class_name(class))),
            ),
            semantic("cause", CanonicalValue::Identifier(cause)),
        ],
    }
}

const EMPTY_DIGEST: [u8; 32] = [0; 32];

fn payload_fields<'a>(value: &'a EventPayload<'a>) -> [MapField<'a>; 11] {
    match value {
        EventPayload::None => payload_map(
            "none",
            None,
            &EMPTY_DIGEST,
            None,
            EventPayloadShape::default(),
            None,
            None,
            &[],
        ),
        EventPayload::InlinePublic { value_type, bytes } => payload_map(
            "inline-public",
            Some(value_type),
            &EMPTY_DIGEST,
            Some(Sensitivity::Public),
            EventPayloadShape {
                byte_length: u64::try_from(bytes.len()).ok(),
                item_count: None,
            },
            None,
            None,
            bytes,
        ),
        EventPayload::Reference {
            value_type,
            digest,
            sensitivity,
            shape,
            recording_authority,
        } => payload_map(
            "reference",
            Some(value_type),
            digest.as_bytes(),
            Some(*sensitivity),
            *shape,
            *recording_authority,
            None,
            &[],
        ),
        EventPayload::Redacted {
            value_type,
            sensitivity,
            shape,
            reason,
        } => payload_map(
            "redacted",
            Some(value_type),
            &EMPTY_DIGEST,
            Some(*sensitivity),
            *shape,
            None,
            Some(*reason),
            &[],
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn payload_map<'a>(
    mode: &'a str,
    value_type: Option<&'a TypeContractRef<'a>>,
    digest: &'a [u8; 32],
    sensitivity: Option<Sensitivity>,
    shape: EventPayloadShape,
    authority: Option<Id<'a>>,
    reason: Option<Id<'a>>,
    bytes: &'a [u8],
) -> [MapField<'a>; 11] {
    [
        semantic("mode", CanonicalValue::Identifier(Id(mode))),
        semantic(
            "type_id",
            value_type.map_or(CanonicalValue::Null, |value| {
                CanonicalValue::Identifier(value.contract_id)
            }),
        ),
        semantic(
            "type_version",
            value_type.map_or(CanonicalValue::Null, |value| {
                CanonicalValue::Integer(i128::from(value.schema_version))
            }),
        ),
        semantic(
            "type_hash",
            value_type.map_or(CanonicalValue::Null, |value| {
                CanonicalValue::Bytes(value.semantic_hash.as_bytes())
            }),
        ),
        semantic("digest", CanonicalValue::Bytes(digest)),
        semantic(
            "sensitivity",
            sensitivity.map_or(CanonicalValue::Null, |value| {
                CanonicalValue::Identifier(Id(value.as_str()))
            }),
        ),
        semantic(
            "byte_length",
            shape.byte_length.map_or(CanonicalValue::Null, |value| {
                CanonicalValue::Integer(i128::from(value))
            }),
        ),
        semantic(
            "item_count",
            shape.item_count.map_or(CanonicalValue::Null, |value| {
                CanonicalValue::Integer(i128::from(value))
            }),
        ),
        semantic("recording_authority", optional_id(authority)),
        semantic("redaction_reason", optional_id(reason)),
        semantic("bytes", CanonicalValue::Bytes(bytes)),
    ]
}

fn optional_id(value: Option<Id<'_>>) -> CanonicalValue<'_> {
    value.map_or(CanonicalValue::Null, CanonicalValue::Identifier)
}

const fn terminal_class_name(value: TerminalClass) -> &'static str {
    match value {
        TerminalClass::Succeeded => "succeeded",
        TerminalClass::Disconnected => "disconnected",
        TerminalClass::Cancelled => "cancelled",
        TerminalClass::Failed => "failed",
    }
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
