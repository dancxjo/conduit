//! Hosted NDJSON representation for immutable core execution events.

use std::fmt;

use conduit_core::{
    ArtifactDigest, EventCorrelation, EventPayload, EventPayloadShape, EventRelations,
    EventTerminality, EventTime, EventTimeKind, ExecutionEvent, ExecutionEventKind, Id,
    InstancePath, MAX_EVENT_DERIVATIONS, SemanticHash, Sensitivity, TerminalClass, TypeContractRef,
};
use serde::{Deserialize, Serialize};

/// Hosted allocation ceilings for one untrusted evidence stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceDecodeLimits {
    pub maximum_input_bytes: usize,
    pub maximum_record_bytes: usize,
    pub maximum_records: usize,
    pub maximum_inline_payload_bytes: usize,
    pub maximum_derivations: usize,
    pub maximum_string_bytes: usize,
}

impl Default for EvidenceDecodeLimits {
    fn default() -> Self {
        Self {
            maximum_input_bytes: 8 * 1024 * 1024,
            maximum_record_bytes: 1024 * 1024,
            maximum_records: 4096,
            maximum_inline_payload_bytes: 64 * 1024,
            maximum_derivations: MAX_EVENT_DERIVATIONS,
            maximum_string_bytes: 4096,
        }
    }
}

/// Stable evidence decoder limit category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NdjsonLimit {
    InputBytes,
    RecordBytes,
    Records,
    InlinePayloadBytes,
    Derivations,
    StringBytes,
}

/// Owned, encoding-oriented form. JSON bytes are not event identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedExecutionEvent {
    pub schema_version: u32,
    pub identity: String,
    pub event_id: String,
    pub run_id: String,
    pub plan_identity: String,
    pub sequence: u64,
    pub recorder: String,
    pub observer: String,
    pub observer_sequence: u64,
    pub logical_template: Option<String>,
    pub subject: String,
    pub kind: String,
    pub detail: String,
    pub observed_time: OwnedEventTime,
    pub domain_time: Option<OwnedEventTime>,
    pub correlation: OwnedEventCorrelation,
    pub relations: OwnedEventRelations,
    pub terminality: OwnedEventTerminality,
    pub payload: OwnedEventPayload,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedEventTime {
    pub kind: String,
    pub basis: String,
    pub tick: i64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedEventCorrelation {
    pub request: Option<String>,
    pub exchange: Option<String>,
    pub session: Option<String>,
    pub epoch: Option<u32>,
    pub work_unit: Option<String>,
    pub attempt: Option<String>,
    pub correlation: Option<String>,
    pub idempotency: Option<String>,
    pub checkpoint: Option<String>,
    pub transport: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedEventRelations {
    pub caused_by: Option<String>,
    pub derived_from: Vec<String>,
    pub supersedes: Option<String>,
    pub retracts: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OwnedEventTerminality {
    NonTerminal,
    Terminal { class: String, cause: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedPayloadShape {
    pub byte_length: Option<u64>,
    pub item_count: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedTypeRef {
    pub id: String,
    pub schema_version: u32,
    pub semantic_hash: String,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OwnedEventPayload {
    None,
    InlinePublic {
        value_type: OwnedTypeRef,
        bytes: Vec<u8>,
    },
    Reference {
        value_type: OwnedTypeRef,
        digest: String,
        sensitivity: String,
        shape: OwnedPayloadShape,
        recording_authority: Option<String>,
    },
    Redacted {
        value_type: OwnedTypeRef,
        sensitivity: String,
        shape: OwnedPayloadShape,
        reason: String,
    },
}

impl fmt::Debug for OwnedEventPayload {
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
                let digest: &dyn fmt::Debug = if sensitivity == "public" {
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

/// Hosted encoding/decoding or borrowed-view failure.
#[derive(Debug)]
pub enum NdjsonError {
    Json(serde_json::Error),
    EmptyLine { line: usize },
    InvalidField(&'static str),
    DerivationScratchTooSmall,
    LimitExceeded(NdjsonLimit),
}

impl fmt::Display for NdjsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "NDJSON parse failed: {error}"),
            Self::EmptyLine { line } => write!(formatter, "NDJSON line {line} is empty"),
            Self::InvalidField(field) => write!(formatter, "invalid NDJSON event field `{field}`"),
            Self::DerivationScratchTooSmall => {
                formatter.write_str("derived-from scratch storage is too small")
            }
            Self::LimitExceeded(limit) => write!(
                formatter,
                "evidence decode limit exceeded: {}",
                match limit {
                    NdjsonLimit::InputBytes => "input-bytes",
                    NdjsonLimit::RecordBytes => "record-bytes",
                    NdjsonLimit::Records => "records",
                    NdjsonLimit::InlinePayloadBytes => "inline-payload-bytes",
                    NdjsonLimit::Derivations => "derivations",
                    NdjsonLimit::StringBytes => "string-bytes",
                }
            ),
        }
    }
}

impl From<serde_json::Error> for NdjsonError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl OwnedExecutionEvent {
    #[must_use]
    pub fn from_event(event: &ExecutionEvent<'_>) -> Self {
        Self {
            schema_version: event.schema_version,
            identity: event.identity.to_string(),
            event_id: event.event_id.as_str().to_owned(),
            run_id: event.run_id.as_str().to_owned(),
            plan_identity: event.plan_identity.to_string(),
            sequence: event.sequence,
            recorder: event.recorder.as_str().to_owned(),
            observer: event.observer.as_str().to_owned(),
            observer_sequence: event.observer_sequence,
            logical_template: event.logical_template.map(|path| path.as_str().to_owned()),
            subject: event.subject.as_str().to_owned(),
            kind: event.kind.as_str().to_owned(),
            detail: event.detail.as_str().to_owned(),
            observed_time: OwnedEventTime::from_time(event.observed_time),
            domain_time: event.domain_time.map(OwnedEventTime::from_time),
            correlation: OwnedEventCorrelation::from_correlation(event.correlation),
            relations: OwnedEventRelations {
                caused_by: event.relations.caused_by.map(|id| id.as_str().to_owned()),
                derived_from: event
                    .relations
                    .derived_from
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
                supersedes: event.relations.supersedes.map(|id| id.as_str().to_owned()),
                retracts: event.relations.retracts.map(|id| id.as_str().to_owned()),
            },
            terminality: OwnedEventTerminality::from_terminality(event.terminality),
            payload: OwnedEventPayload::from_payload(event.payload),
        }
    }

    /// Borrow this owned record as the allocator-free core representation.
    pub fn as_event<'a>(
        &'a self,
        derivation_scratch: &'a mut [Id<'a>],
    ) -> Result<ExecutionEvent<'a>, NdjsonError> {
        if derivation_scratch.len() < self.relations.derived_from.len() {
            return Err(NdjsonError::DerivationScratchTooSmall);
        }
        for (slot, value) in derivation_scratch
            .iter_mut()
            .zip(&self.relations.derived_from)
        {
            *slot = checked_id(value, "relations.derived_from")?;
        }
        let derived_from = &derivation_scratch[..self.relations.derived_from.len()];
        Ok(ExecutionEvent {
            schema_version: self.schema_version,
            identity: parse_semantic_hash(&self.identity, "identity")?,
            event_id: checked_id(&self.event_id, "event_id")?,
            run_id: checked_id(&self.run_id, "run_id")?,
            plan_identity: parse_semantic_hash(&self.plan_identity, "plan_identity")?,
            sequence: self.sequence,
            recorder: checked_id(&self.recorder, "recorder")?,
            observer: checked_id(&self.observer, "observer")?,
            observer_sequence: self.observer_sequence,
            logical_template: self
                .logical_template
                .as_deref()
                .map(InstancePath::new)
                .transpose()
                .map_err(|_| NdjsonError::InvalidField("logical_template"))?,
            subject: InstancePath::new(&self.subject)
                .map_err(|_| NdjsonError::InvalidField("subject"))?,
            kind: ExecutionEventKind::parse(&self.kind).ok_or(NdjsonError::InvalidField("kind"))?,
            detail: checked_id(&self.detail, "detail")?,
            observed_time: self.observed_time.as_time()?,
            domain_time: self
                .domain_time
                .as_ref()
                .map(OwnedEventTime::as_time)
                .transpose()?,
            correlation: self.correlation.as_correlation()?,
            relations: EventRelations {
                caused_by: optional_id(self.relations.caused_by.as_deref(), "relations.caused_by")?,
                derived_from,
                supersedes: optional_id(
                    self.relations.supersedes.as_deref(),
                    "relations.supersedes",
                )?,
                retracts: optional_id(self.relations.retracts.as_deref(), "relations.retracts")?,
            },
            terminality: self.terminality.as_terminality()?,
            payload: self.payload.as_payload()?,
        })
    }
}

impl OwnedEventTime {
    fn from_time(value: EventTime<'_>) -> Self {
        Self {
            kind: value.kind.as_str().to_owned(),
            basis: value.basis.as_str().to_owned(),
            tick: value.tick,
        }
    }

    fn as_time(&self) -> Result<EventTime<'_>, NdjsonError> {
        Ok(EventTime {
            kind: EventTimeKind::parse(&self.kind).ok_or(NdjsonError::InvalidField("time.kind"))?,
            basis: checked_id(&self.basis, "time.basis")?,
            tick: self.tick,
        })
    }
}

impl OwnedEventCorrelation {
    fn from_correlation(value: EventCorrelation<'_>) -> Self {
        Self {
            request: owned_id(value.request),
            exchange: owned_id(value.exchange),
            session: owned_id(value.session),
            epoch: value.epoch,
            work_unit: owned_id(value.work_unit),
            attempt: owned_id(value.attempt),
            correlation: owned_id(value.correlation),
            idempotency: owned_id(value.idempotency),
            checkpoint: owned_id(value.checkpoint),
            transport: owned_id(value.transport),
        }
    }

    fn as_correlation(&self) -> Result<EventCorrelation<'_>, NdjsonError> {
        Ok(EventCorrelation {
            request: optional_id(self.request.as_deref(), "correlation.request")?,
            exchange: optional_id(self.exchange.as_deref(), "correlation.exchange")?,
            session: optional_id(self.session.as_deref(), "correlation.session")?,
            epoch: self.epoch,
            work_unit: optional_id(self.work_unit.as_deref(), "correlation.work_unit")?,
            attempt: optional_id(self.attempt.as_deref(), "correlation.attempt")?,
            correlation: optional_id(self.correlation.as_deref(), "correlation.correlation")?,
            idempotency: optional_id(self.idempotency.as_deref(), "correlation.idempotency")?,
            checkpoint: optional_id(self.checkpoint.as_deref(), "correlation.checkpoint")?,
            transport: optional_id(self.transport.as_deref(), "correlation.transport")?,
        })
    }
}

impl OwnedEventTerminality {
    fn from_terminality(value: EventTerminality<'_>) -> Self {
        match value {
            EventTerminality::NonTerminal => Self::NonTerminal,
            EventTerminality::Terminal { class, cause } => Self::Terminal {
                class: terminal_class_name(class).to_owned(),
                cause: cause.as_str().to_owned(),
            },
        }
    }

    fn as_terminality(&self) -> Result<EventTerminality<'_>, NdjsonError> {
        match self {
            Self::NonTerminal => Ok(EventTerminality::NonTerminal),
            Self::Terminal { class, cause } => Ok(EventTerminality::Terminal {
                class: parse_terminal_class(class)
                    .ok_or(NdjsonError::InvalidField("terminality.class"))?,
                cause: checked_id(cause, "terminality.cause")?,
            }),
        }
    }
}

impl OwnedEventPayload {
    fn from_payload(value: EventPayload<'_>) -> Self {
        match value {
            EventPayload::None => Self::None,
            EventPayload::InlinePublic { value_type, bytes } => Self::InlinePublic {
                value_type: OwnedTypeRef::from_type(value_type),
                bytes: bytes.to_vec(),
            },
            EventPayload::Reference {
                value_type,
                digest,
                sensitivity,
                shape,
                recording_authority,
            } => Self::Reference {
                value_type: OwnedTypeRef::from_type(value_type),
                digest: digest.to_string(),
                sensitivity: sensitivity.as_str().to_owned(),
                shape: OwnedPayloadShape::from_shape(shape),
                recording_authority: owned_id(recording_authority),
            },
            EventPayload::Redacted {
                value_type,
                sensitivity,
                shape,
                reason,
            } => Self::Redacted {
                value_type: OwnedTypeRef::from_type(value_type),
                sensitivity: sensitivity.as_str().to_owned(),
                shape: OwnedPayloadShape::from_shape(shape),
                reason: reason.as_str().to_owned(),
            },
        }
    }

    fn as_payload(&self) -> Result<EventPayload<'_>, NdjsonError> {
        match self {
            Self::None => Ok(EventPayload::None),
            Self::InlinePublic { value_type, bytes } => Ok(EventPayload::InlinePublic {
                value_type: value_type.as_type()?,
                bytes,
            }),
            Self::Reference {
                value_type,
                digest,
                sensitivity,
                shape,
                recording_authority,
            } => Ok(EventPayload::Reference {
                value_type: value_type.as_type()?,
                digest: ArtifactDigest::from_bytes(parse_sha256(digest, "payload.digest")?),
                sensitivity: parse_sensitivity(sensitivity)
                    .ok_or(NdjsonError::InvalidField("payload.sensitivity"))?,
                shape: shape.as_shape(),
                recording_authority: optional_id(
                    recording_authority.as_deref(),
                    "payload.recording_authority",
                )?,
            }),
            Self::Redacted {
                value_type,
                sensitivity,
                shape,
                reason,
            } => Ok(EventPayload::Redacted {
                value_type: value_type.as_type()?,
                sensitivity: parse_sensitivity(sensitivity)
                    .ok_or(NdjsonError::InvalidField("payload.sensitivity"))?,
                shape: shape.as_shape(),
                reason: checked_id(reason, "payload.reason")?,
            }),
        }
    }
}

impl OwnedTypeRef {
    fn from_type(value: TypeContractRef<'_>) -> Self {
        Self {
            id: value.contract_id.as_str().to_owned(),
            schema_version: value.schema_version,
            semantic_hash: value.semantic_hash.to_string(),
        }
    }

    fn as_type(&self) -> Result<TypeContractRef<'_>, NdjsonError> {
        Ok(TypeContractRef {
            contract_id: checked_id(&self.id, "payload.type.id")?,
            schema_version: self.schema_version,
            semantic_hash: parse_semantic_hash(&self.semantic_hash, "payload.type.semantic_hash")?,
        })
    }
}

impl OwnedPayloadShape {
    fn from_shape(value: EventPayloadShape) -> Self {
        Self {
            byte_length: value.byte_length,
            item_count: value.item_count,
        }
    }

    fn as_shape(&self) -> EventPayloadShape {
        EventPayloadShape {
            byte_length: self.byte_length,
            item_count: self.item_count,
        }
    }
}

/// Encode one JSON object per line with a final newline.
pub fn encode_event_ndjson(events: &[ExecutionEvent<'_>]) -> Result<String, NdjsonError> {
    let owned = events
        .iter()
        .map(|event| OwnedExecutionEvent::from_event(event))
        .collect::<Vec<_>>();
    encode_owned_event_ndjson(&owned)
}

/// Re-encode decoded owned records in the stable hosted field order.
pub fn encode_owned_event_ndjson(events: &[OwnedExecutionEvent]) -> Result<String, NdjsonError> {
    let mut output = String::new();
    for event in events {
        output.push_str(&serde_json::to_string(event)?);
        output.push('\n');
    }
    Ok(output)
}

/// Decode a complete NDJSON stream without accepting blank records.
pub fn decode_event_ndjson(input: &str) -> Result<Vec<OwnedExecutionEvent>, NdjsonError> {
    decode_event_ndjson_with_limits(input, EvidenceDecodeLimits::default())
}

/// Decode untrusted evidence only within explicit allocation and shape limits.
pub fn decode_event_ndjson_with_limits(
    input: &str,
    limits: EvidenceDecodeLimits,
) -> Result<Vec<OwnedExecutionEvent>, NdjsonError> {
    if input.len() > limits.maximum_input_bytes {
        return Err(NdjsonError::LimitExceeded(NdjsonLimit::InputBytes));
    }
    let initial_capacity = input
        .lines()
        .take(limits.maximum_records.saturating_add(1))
        .count()
        .min(limits.maximum_records);
    let mut events = Vec::with_capacity(initial_capacity);
    for (index, line) in input.lines().enumerate() {
        if index >= limits.maximum_records {
            return Err(NdjsonError::LimitExceeded(NdjsonLimit::Records));
        }
        if line.is_empty() {
            return Err(NdjsonError::EmptyLine { line: index + 1 });
        }
        if line.len() > limits.maximum_record_bytes {
            return Err(NdjsonError::LimitExceeded(NdjsonLimit::RecordBytes));
        }
        let event: OwnedExecutionEvent = serde_json::from_str(line)?;
        validate_owned_event_limits(&event, limits)?;
        events.push(event);
    }
    Ok(events)
}

fn validate_owned_event_limits(
    event: &OwnedExecutionEvent,
    limits: EvidenceDecodeLimits,
) -> Result<(), NdjsonError> {
    if event.relations.derived_from.len() > limits.maximum_derivations {
        return Err(NdjsonError::LimitExceeded(NdjsonLimit::Derivations));
    }
    if matches!(
        &event.payload,
        OwnedEventPayload::InlinePublic { bytes, .. }
            if bytes.len() > limits.maximum_inline_payload_bytes
    ) {
        return Err(NdjsonError::LimitExceeded(NdjsonLimit::InlinePayloadBytes));
    }
    let mut strings = [
        event.identity.as_str(),
        event.event_id.as_str(),
        event.run_id.as_str(),
        event.plan_identity.as_str(),
        event.recorder.as_str(),
        event.observer.as_str(),
        event.subject.as_str(),
        event.kind.as_str(),
        event.detail.as_str(),
        event.observed_time.kind.as_str(),
        event.observed_time.basis.as_str(),
    ]
    .into_iter()
    .chain(event.logical_template.as_deref())
    .chain(
        event
            .domain_time
            .iter()
            .flat_map(|time| [time.kind.as_str(), time.basis.as_str()]),
    )
    .chain(
        [
            event.correlation.request.as_deref(),
            event.correlation.exchange.as_deref(),
            event.correlation.session.as_deref(),
            event.correlation.work_unit.as_deref(),
            event.correlation.attempt.as_deref(),
            event.correlation.correlation.as_deref(),
            event.correlation.idempotency.as_deref(),
            event.correlation.checkpoint.as_deref(),
            event.correlation.transport.as_deref(),
            event.relations.caused_by.as_deref(),
            event.relations.supersedes.as_deref(),
            event.relations.retracts.as_deref(),
        ]
        .into_iter()
        .flatten(),
    )
    .chain(event.relations.derived_from.iter().map(String::as_str));
    if strings.any(|value| value.len() > limits.maximum_string_bytes) {
        return Err(NdjsonError::LimitExceeded(NdjsonLimit::StringBytes));
    }
    let terminal_string_too_long = match &event.terminality {
        OwnedEventTerminality::NonTerminal => false,
        OwnedEventTerminality::Terminal { class, cause } => {
            class.len() > limits.maximum_string_bytes || cause.len() > limits.maximum_string_bytes
        }
    };
    let payload_string_too_long = match &event.payload {
        OwnedEventPayload::None => false,
        OwnedEventPayload::InlinePublic { value_type, .. } => {
            owned_type_string_too_long(value_type, limits)
        }
        OwnedEventPayload::Reference {
            value_type,
            digest,
            sensitivity,
            recording_authority,
            ..
        } => {
            owned_type_string_too_long(value_type, limits)
                || digest.len() > limits.maximum_string_bytes
                || sensitivity.len() > limits.maximum_string_bytes
                || recording_authority
                    .as_ref()
                    .is_some_and(|value| value.len() > limits.maximum_string_bytes)
        }
        OwnedEventPayload::Redacted {
            value_type,
            sensitivity,
            reason,
            ..
        } => {
            owned_type_string_too_long(value_type, limits)
                || sensitivity.len() > limits.maximum_string_bytes
                || reason.len() > limits.maximum_string_bytes
        }
    };
    if terminal_string_too_long || payload_string_too_long {
        return Err(NdjsonError::LimitExceeded(NdjsonLimit::StringBytes));
    }
    Ok(())
}

fn owned_type_string_too_long(value: &OwnedTypeRef, limits: EvidenceDecodeLimits) -> bool {
    value.id.len() > limits.maximum_string_bytes
        || value.semantic_hash.len() > limits.maximum_string_bytes
}

fn checked_id<'a>(value: &'a str, field: &'static str) -> Result<Id<'a>, NdjsonError> {
    Id::new(value).map_err(|_| NdjsonError::InvalidField(field))
}

fn optional_id<'a>(
    value: Option<&'a str>,
    field: &'static str,
) -> Result<Option<Id<'a>>, NdjsonError> {
    value.map(|value| checked_id(value, field)).transpose()
}

fn owned_id(value: Option<Id<'_>>) -> Option<String> {
    value.map(|id| id.as_str().to_owned())
}

fn parse_semantic_hash(value: &str, field: &'static str) -> Result<SemanticHash, NdjsonError> {
    parse_sha256(value, field).map(SemanticHash::from_bytes)
}

fn parse_sha256(value: &str, field: &'static str) -> Result<[u8; 32], NdjsonError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(NdjsonError::InvalidField(field));
    };
    if hex.len() != 64 {
        return Err(NdjsonError::InvalidField(field));
    }
    if !hex
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(NdjsonError::InvalidField(field));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        let pair = core::str::from_utf8(pair).map_err(|_| NdjsonError::InvalidField(field))?;
        bytes[index] =
            u8::from_str_radix(pair, 16).map_err(|_| NdjsonError::InvalidField(field))?;
    }
    Ok(bytes)
}

fn parse_sensitivity(value: &str) -> Option<Sensitivity> {
    match value {
        "public" => Some(Sensitivity::Public),
        "restricted" => Some(Sensitivity::Restricted),
        "secret" => Some(Sensitivity::Secret),
        _ => None,
    }
}

fn parse_terminal_class(value: &str) -> Option<TerminalClass> {
    match value {
        "succeeded" => Some(TerminalClass::Succeeded),
        "disconnected" => Some(TerminalClass::Disconnected),
        "cancelled" => Some(TerminalClass::Cancelled),
        "failed" => Some(TerminalClass::Failed),
        _ => None,
    }
}

fn terminal_class_name(value: TerminalClass) -> &'static str {
    match value {
        TerminalClass::Succeeded => "succeeded",
        TerminalClass::Disconnected => "disconnected",
        TerminalClass::Cancelled => "cancelled",
        TerminalClass::Failed => "failed",
    }
}
