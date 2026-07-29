//! Transport-neutral, allocator-free distributed-cord contracts and state.
//!
//! Carrier authentication protects a byte channel. It never substitutes for
//! realm membership, workload delegation, or an exact authority grant.

use core::convert::Infallible;
use core::fmt;

use crate::{
    AuthorityTime, CanonicalDescriptor, CanonicalError, CanonicalValue, CredentialVerification,
    CredentialVerificationOutcome, FieldDisposition, FlowPolicy, Id, MapField, ObservedGrant,
    PassportStatusObservation, PinnedDescriptor, PlanAuthority, PlanResourceBudget, SemanticHash,
    TerminalClass, WorkloadDelegation, validate_authority_at_use, validate_credential_verification,
    validate_delegation, validate_passport_status,
};

/// Version of the distributed-cord session contract.
pub const DISTRIBUTED_CORD_PROTOCOL_VERSION: u16 = 1;

/// Delivery promise made by the Conduit session layer.
///
/// Exactly-once is intentionally absent: that claim requires an application
/// transaction boundary rather than a transport acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedDelivery {
    AtMostOnce,
    AtLeastOnce,
}

impl DistributedDelivery {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AtMostOnce => "at-most-once",
            Self::AtLeastOnce => "at-least-once",
        }
    }
}

/// Value acknowledgement mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcknowledgementMode {
    None,
    Cumulative,
}

impl AcknowledgementMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Cumulative => "cumulative",
        }
    }
}

/// Ordering promise exposed above a carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedOrdering {
    InOrder,
}

impl DistributedOrdering {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InOrder => "in-order",
        }
    }
}

/// Reconnect behavior. Same-epoch resume requires a bounded proof; a new
/// epoch makes any loss or duplication boundary explicit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconnectMode {
    Reject,
    ResumeSameEpoch,
    BeginNewEpoch,
}

impl ReconnectMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reject => "reject",
            Self::ResumeSameEpoch => "resume-same-epoch",
            Self::BeginNewEpoch => "begin-new-epoch",
        }
    }
}

/// Action taken when liveness expires or the carrier disconnects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisconnectPolicy {
    CancelCord,
    FailScope,
    AwaitReconnect,
}

impl DisconnectPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CancelCord => "cancel-cord",
            Self::FailScope => "fail-scope",
            Self::AwaitReconnect => "await-reconnect",
        }
    }
}

/// Exact finite transport-owned storage and time limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedCordBudget {
    pub send_items: u16,
    pub send_bytes: u64,
    pub receive_items: u16,
    pub receive_bytes: u64,
    pub retry_items: u16,
    pub retry_bytes: u64,
    pub reorder_items: u16,
    pub reorder_bytes: u64,
    pub dedup_items: u16,
    pub maximum_payload_bytes: u32,
    pub maximum_frame_bytes: u32,
    pub maximum_unacknowledged: u16,
    pub maximum_retries: u16,
    pub maximum_reconnect_attempts: u16,
    pub heartbeat_interval_ticks: u64,
    pub liveness_timeout_ticks: u64,
    pub reconnect_deadline_ticks: u64,
    pub maximum_evidence_events: u16,
    /// Exact plan reservation covering carrier-owned payload and metadata.
    pub allocated_memory_bytes: u64,
}

impl DistributedCordBudget {
    /// Minimum semantic payload reservation. Carrier-specific allocator and
    /// framing overhead must fit inside `allocated_memory_bytes` too.
    #[must_use]
    pub fn minimum_payload_reservation(self) -> Option<u64> {
        self.send_bytes
            .checked_add(self.receive_bytes)
            .and_then(|value| value.checked_add(self.retry_bytes))
            .and_then(|value| value.checked_add(self.reorder_bytes))
    }
}

/// Exact peer identity constraints selected into the plan.
///
/// Status, possession proof, and workload delegation are live handshake
/// observations and therefore remain outside immutable plan identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedPeerRequirement<'a> {
    pub node: crate::InstancePath<'a>,
    pub host_observation: Id<'a>,
    pub realm: Id<'a>,
    pub realm_identity: SemanticHash,
    pub entity: Id<'a>,
    pub passport: SemanticHash,
    pub passport_schema_version: u32,
    pub credential: Id<'a>,
    pub credential_epoch: u32,
    pub key: Id<'a>,
    pub key_epoch: u32,
    pub status_reporter: PinnedDescriptor<'a>,
    pub credential_verifier: PinnedDescriptor<'a>,
    pub audience: Id<'a>,
    pub grant_hash: SemanticHash,
}

/// Exact distributed binding for one ordinary plan cord.
///
/// This is a plan fact, not a socket, connection, discovery request, or
/// execution observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanDistributedCord<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub cord: Id<'a>,
    pub writer_port_contract_hash: SemanticHash,
    pub reader_port_contract_hash: SemanticHash,
    pub flow: FlowPolicy<'a>,
    pub session: Id<'a>,
    pub initial_session_epoch: u64,
    pub backend: PinnedDescriptor<'a>,
    pub carrier_security: PinnedDescriptor<'a>,
    /// Carrier-owned binding (for example a resolved topic or key
    /// expression), distinct from the semantic cord ID.
    pub carrier_binding: Id<'a>,
    pub delivery: DistributedDelivery,
    pub acknowledgement: AcknowledgementMode,
    pub ordering: DistributedOrdering,
    pub reconnect: ReconnectMode,
    pub disconnect: DisconnectPolicy,
    pub writer: DistributedPeerRequirement<'a>,
    pub reader: DistributedPeerRequirement<'a>,
    /// Required only for a cross-realm session.
    pub federation_policy: Option<PinnedDescriptor<'a>>,
    pub budget: DistributedCordBudget,
    pub allocation: PlanResourceBudget,
}

impl PlanDistributedCord<'_> {
    /// Canonical binding identity. It deliberately excludes the enclosing
    /// plan identity to avoid a self-referential hash.
    pub fn semantic_hash(&self) -> Result<SemanticHash, DistributedIdentityError> {
        let flow = flow_fields(self.flow);
        let writer_status = pin_fields(&self.writer.status_reporter);
        let writer_verifier = pin_fields(&self.writer.credential_verifier);
        let writer = peer_fields(&self.writer, &writer_status, &writer_verifier);
        let reader_status = pin_fields(&self.reader.status_reporter);
        let reader_verifier = pin_fields(&self.reader.credential_verifier);
        let reader = peer_fields(&self.reader, &reader_status, &reader_verifier);
        let backend = pin_fields(&self.backend);
        let carrier_security = pin_fields(&self.carrier_security);
        let federation = self.federation_policy.as_ref().map(pin_fields);
        let federation = federation
            .as_ref()
            .map_or(CanonicalValue::Null, |fields| CanonicalValue::Map(fields));
        let budget = distributed_budget_fields(self.budget);
        let allocation = resource_budget_fields(self.allocation);
        CanonicalDescriptor {
            kind: Id("conduit/distributed-cord-binding"),
            schema_version: self.schema_version,
            body: CanonicalValue::Map(&[
                field("cord", CanonicalValue::Identifier(self.cord)),
                field(
                    "writer_port_contract_hash",
                    CanonicalValue::Bytes(self.writer_port_contract_hash.as_bytes()),
                ),
                field(
                    "reader_port_contract_hash",
                    CanonicalValue::Bytes(self.reader_port_contract_hash.as_bytes()),
                ),
                field("flow", CanonicalValue::Map(&flow)),
                field("session", CanonicalValue::Identifier(self.session)),
                field(
                    "initial_session_epoch",
                    CanonicalValue::Integer(i128::from(self.initial_session_epoch)),
                ),
                field("backend", CanonicalValue::Map(&backend)),
                field("carrier_security", CanonicalValue::Map(&carrier_security)),
                field(
                    "carrier_binding",
                    CanonicalValue::Identifier(self.carrier_binding),
                ),
                field(
                    "delivery",
                    CanonicalValue::Identifier(Id(self.delivery.as_str())),
                ),
                field(
                    "acknowledgement",
                    CanonicalValue::Identifier(Id(self.acknowledgement.as_str())),
                ),
                field(
                    "ordering",
                    CanonicalValue::Identifier(Id(self.ordering.as_str())),
                ),
                field(
                    "reconnect",
                    CanonicalValue::Identifier(Id(self.reconnect.as_str())),
                ),
                field(
                    "disconnect",
                    CanonicalValue::Identifier(Id(self.disconnect.as_str())),
                ),
                field("writer", CanonicalValue::Map(&writer)),
                field("reader", CanonicalValue::Map(&reader)),
                field("federation_policy", federation),
                field("budget", CanonicalValue::Map(&budget)),
                field("allocation", CanonicalValue::Map(&allocation)),
            ]),
        }
        .semantic_hash()
        .map_err(DistributedIdentityError::Canonical)
    }
}

/// Fresh proof supplied by one peer during handshake.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedPeerProof<'a> {
    pub credential_epoch: u32,
    pub key: Id<'a>,
    pub key_epoch: u32,
    pub status: PassportStatusObservation<'a>,
    pub possession: CredentialVerification<'a>,
    pub delegation: WorkloadDelegation<'a>,
}

/// Transport-neutral session handshake. Exact plan and binding hashes are
/// joined only after the immutable plan has been sealed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedCordHandshake<'a> {
    pub protocol_version: u16,
    pub plan_identity: SemanticHash,
    pub binding_identity: SemanticHash,
    pub cord: Id<'a>,
    pub session: Id<'a>,
    pub session_epoch: u64,
    pub run: Id<'a>,
    pub run_epoch: u64,
    pub writer: DistributedPeerProof<'a>,
    pub reader: DistributedPeerProof<'a>,
}

/// Caller-supplied current time for validating live handshake observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedHandshakeContext<'a> {
    pub expected_plan_identity: SemanticHash,
    pub now: AuthorityTime<'a>,
}

/// Fresh authority observations supplied at every carrier effect boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedAuthorityContext<'a> {
    pub writer: PlanAuthority<'a>,
    pub writer_grant: ObservedGrant<'a>,
    pub reader: PlanAuthority<'a>,
    pub reader_grant: ObservedGrant<'a>,
    pub now: AuthorityTime<'a>,
}

/// Same-epoch resume proof. It never authorizes a new plan or epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResumeProof {
    pub plan_identity: SemanticHash,
    pub binding_identity: SemanticHash,
    pub session_epoch: u64,
    pub writer_next_sequence: u64,
    pub reader_next_sequence: u64,
    pub acknowledged_through: Option<u64>,
    pub receipt: SemanticHash,
}

/// Stable distributed-cord reason vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedReason {
    UnsupportedVersion,
    IdentityMismatch,
    InvalidBinding,
    FlowMismatch,
    UnboundedBudget,
    DeliveryMismatch,
    PeerMismatch,
    StalePeerStatus,
    CredentialRejected,
    DelegationDenied,
    AuthorityDenied,
    HandshakeMismatch,
    OversizedFrame,
    SequenceViolation,
    RetryExhausted,
    DedupWindowExhausted,
    ReconnectDenied,
    EpochMismatch,
    TerminalViolation,
    BufferFull,
    Partitioned,
    EvidenceFull,
}

impl DistributedReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-DST-001",
            Self::IdentityMismatch => "CND-DST-002",
            Self::InvalidBinding => "CND-DST-003",
            Self::FlowMismatch => "CND-DST-004",
            Self::UnboundedBudget => "CND-DST-005",
            Self::DeliveryMismatch => "CND-DST-006",
            Self::PeerMismatch => "CND-DST-007",
            Self::StalePeerStatus => "CND-DST-008",
            Self::CredentialRejected => "CND-DST-009",
            Self::DelegationDenied => "CND-DST-010",
            Self::AuthorityDenied => "CND-DST-011",
            Self::HandshakeMismatch => "CND-DST-012",
            Self::OversizedFrame => "CND-DST-013",
            Self::SequenceViolation => "CND-DST-014",
            Self::RetryExhausted => "CND-DST-015",
            Self::DedupWindowExhausted => "CND-DST-016",
            Self::ReconnectDenied => "CND-DST-017",
            Self::EpochMismatch => "CND-DST-018",
            Self::TerminalViolation => "CND-DST-019",
            Self::BufferFull => "CND-DST-020",
            Self::Partitioned => "CND-DST-021",
            Self::EvidenceFull => "CND-DST-022",
        }
    }
}

impl fmt::Display for DistributedReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Binding identity construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedIdentityError {
    Canonical(CanonicalError<Infallible>),
}

/// Validate one plan-owned distributed binding independently of a carrier.
pub fn validate_distributed_binding(
    binding: &PlanDistributedCord<'_>,
) -> Result<(), DistributedReason> {
    if binding.schema_version != u32::from(DISTRIBUTED_CORD_PROTOCOL_VERSION) {
        return Err(DistributedReason::UnsupportedVersion);
    }
    if !valid_id(binding.cord)
        || !valid_id(binding.session)
        || !valid_id(binding.carrier_binding)
        || !valid_pin(binding.backend)
        || !valid_pin(binding.carrier_security)
        || binding.initial_session_epoch == 0
        || !valid_peer(binding.writer)
        || !valid_peer(binding.reader)
        || binding.writer.host_observation == binding.reader.host_observation
    {
        return Err(DistributedReason::InvalidBinding);
    }
    if binding.semantic_hash().ok() != Some(binding.identity) {
        return Err(DistributedReason::IdentityMismatch);
    }
    let cross_realm = binding.writer.realm != binding.reader.realm;
    if cross_realm != binding.federation_policy.is_some()
        || binding.federation_policy.is_some_and(|pin| !valid_pin(pin))
    {
        return Err(DistributedReason::PeerMismatch);
    }
    validate_budget(binding)?;
    match (binding.delivery, binding.acknowledgement) {
        (DistributedDelivery::AtMostOnce, AcknowledgementMode::None) => {
            if binding.budget.retry_items != 0
                || binding.budget.retry_bytes != 0
                || binding.budget.dedup_items != 0
                || binding.budget.maximum_unacknowledged != 0
                || binding.budget.maximum_retries != 0
            {
                return Err(DistributedReason::DeliveryMismatch);
            }
        }
        (DistributedDelivery::AtLeastOnce, AcknowledgementMode::Cumulative) => {
            if binding.budget.retry_items == 0
                || binding.budget.retry_bytes == 0
                || binding.budget.maximum_unacknowledged == 0
                || binding.budget.maximum_retries == 0
                || binding.budget.retry_items < binding.budget.maximum_unacknowledged
            {
                return Err(DistributedReason::DeliveryMismatch);
            }
        }
        _ => return Err(DistributedReason::DeliveryMismatch),
    }
    let awaits_reconnect = matches!(binding.disconnect, DisconnectPolicy::AwaitReconnect);
    let rejects_reconnect = matches!(binding.reconnect, ReconnectMode::Reject);
    if (awaits_reconnect && rejects_reconnect) || (!awaits_reconnect && !rejects_reconnect) {
        return Err(DistributedReason::ReconnectDenied);
    }
    Ok(())
}

/// Validate exact plan/binding identity plus fresh realm, possession, and
/// workload proofs before values can flow.
pub fn validate_distributed_handshake(
    binding: &PlanDistributedCord<'_>,
    handshake: DistributedCordHandshake<'_>,
    context: DistributedHandshakeContext<'_>,
) -> Result<(), DistributedReason> {
    validate_distributed_binding(binding)?;
    if handshake.protocol_version != DISTRIBUTED_CORD_PROTOCOL_VERSION {
        return Err(DistributedReason::UnsupportedVersion);
    }
    if handshake.plan_identity != context.expected_plan_identity
        || handshake.binding_identity != binding.identity
        || handshake.cord != binding.cord
        || handshake.session != binding.session
        || handshake.session_epoch < binding.initial_session_epoch
        || !valid_id(handshake.run)
        || handshake.run_epoch == 0
    {
        return Err(DistributedReason::HandshakeMismatch);
    }
    validate_peer_proof(binding.writer, handshake.writer, handshake, context.now)?;
    validate_peer_proof(binding.reader, handshake.reader, handshake, context.now)?;
    Ok(())
}

/// Revalidate both endpoint grants at an actual send, receive, cancellation,
/// terminal, open, or reconnect boundary.
pub fn validate_distributed_authority_at_use(
    binding: &PlanDistributedCord<'_>,
    context: DistributedAuthorityContext<'_>,
) -> Result<(), DistributedReason> {
    validate_distributed_binding(binding)?;
    for (requirement, authority, observed) in [
        (binding.writer, context.writer, context.writer_grant),
        (binding.reader, context.reader, context.reader_grant),
    ] {
        if authority.node != requirement.node
            || authority.grant_hash != requirement.grant_hash
            || authority.effect.semantic_hash().ok() != Some(authority.effect_hash)
            || authority.grant.semantic_hash().ok() != Some(authority.grant_hash)
            || authority.grant.audience != requirement.audience
            || authority.grant != observed.grant
            || authority.binding.grant_id != authority.grant.id
            || validate_authority_at_use(
                authority.binding,
                authority.effect,
                context.now,
                authority.capability,
                observed,
            )
            .is_err()
        {
            return Err(DistributedReason::AuthorityDenied);
        }
    }
    Ok(())
}

fn validate_peer_proof(
    requirement: DistributedPeerRequirement<'_>,
    proof: DistributedPeerProof<'_>,
    handshake: DistributedCordHandshake<'_>,
    now: AuthorityTime<'_>,
) -> Result<(), DistributedReason> {
    if proof.credential_epoch != requirement.credential_epoch
        || proof.key != requirement.key
        || proof.key_epoch != requirement.key_epoch
        || proof.status.reporter != requirement.status_reporter
    {
        return Err(DistributedReason::PeerMismatch);
    }
    validate_passport_status(
        proof.status,
        requirement.passport,
        requirement.realm,
        requirement.entity,
        now.basis,
        now.tick,
    )
    .map_err(|_| DistributedReason::StalePeerStatus)?;
    if proof.possession.verifier != requirement.credential_verifier
        || proof.possession.challenge != handshake.session
    {
        return Err(DistributedReason::CredentialRejected);
    }
    validate_credential_verification(
        proof.possession,
        requirement.credential,
        requirement.passport,
        now.basis,
        now.tick,
    )
    .map_err(|_| DistributedReason::CredentialRejected)?;
    if proof.possession.outcome != CredentialVerificationOutcome::Verified {
        return Err(DistributedReason::CredentialRejected);
    }
    validate_delegation(
        proof.delegation,
        requirement.passport,
        requirement.realm,
        requirement.entity,
        handshake.run,
        handshake.run_epoch,
        now.tick,
    )
    .map_err(|_| DistributedReason::DelegationDenied)?;
    if proof.delegation.plan != handshake.plan_identity
        || proof.delegation.audience != requirement.audience
        || proof.delegation.receipt == SemanticHash::from_bytes([0; 32])
    {
        return Err(DistributedReason::DelegationDenied);
    }
    Ok(())
}

fn validate_budget(binding: &PlanDistributedCord<'_>) -> Result<(), DistributedReason> {
    let budget = binding.budget;
    let minimum = budget
        .minimum_payload_reservation()
        .ok_or(DistributedReason::UnboundedBudget)?;
    if budget.send_items == 0
        || budget.send_bytes == 0
        || budget.receive_items == 0
        || budget.receive_bytes == 0
        || budget.maximum_payload_bytes == 0
        || budget.maximum_frame_bytes < budget.maximum_payload_bytes
        || budget.maximum_evidence_events == 0
        || budget.allocated_memory_bytes < minimum
        || binding.allocation.memory_bytes < budget.allocated_memory_bytes
        || binding.allocation.transports == 0
        || binding.allocation.evidence_bytes < u64::from(budget.maximum_evidence_events)
        || budget.send_items > binding.flow.capacity.items()
        || budget.receive_items > binding.flow.capacity.items()
        || budget.maximum_payload_bytes != binding.flow.capacity.max_value_bytes()
        || budget.send_bytes > u64::from(budget.send_items) * u64::from(budget.maximum_frame_bytes)
        || budget.receive_bytes
            > u64::from(budget.receive_items) * u64::from(budget.maximum_frame_bytes)
        || budget.retry_bytes
            > u64::from(budget.retry_items) * u64::from(budget.maximum_frame_bytes)
        || budget.reorder_bytes
            > u64::from(budget.reorder_items) * u64::from(budget.maximum_frame_bytes)
        || budget.heartbeat_interval_ticks == 0
        || budget.liveness_timeout_ticks <= budget.heartbeat_interval_ticks
        || budget.reconnect_deadline_ticks < budget.liveness_timeout_ticks
    {
        return Err(DistributedReason::UnboundedBudget);
    }
    Ok(())
}

/// Portable session state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedSessionState {
    Handshaking,
    Open,
    Disconnected,
    CancelPending,
    TerminalPending,
    Closed,
    Failed,
}

/// Receiver outcome for one value sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveDisposition {
    Accepted,
    DuplicateSuppressed,
    DuplicateRedelivered,
    HeldForMissingSequence,
}

/// Transport control requiring an acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingControl {
    Cancellation,
    Terminal(TerminalClass),
}

/// Minimal allocator-free reference session machine. Payload and carrier
/// buffers remain caller-owned; the exact binding supplies their limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedSessionMachine {
    pub state: DistributedSessionState,
    pub session_epoch: u64,
    pub next_send_sequence: u64,
    pub next_receive_sequence: u64,
    pub acknowledged_through: Option<u64>,
    pub unacknowledged: u16,
    pub reconnect_attempts: u16,
    pub last_liveness_tick: u64,
    pub pending_control: Option<PendingControl>,
}

impl DistributedSessionMachine {
    #[must_use]
    pub const fn new(initial_session_epoch: u64) -> Self {
        Self {
            state: DistributedSessionState::Handshaking,
            session_epoch: initial_session_epoch,
            next_send_sequence: 0,
            next_receive_sequence: 0,
            acknowledged_through: None,
            unacknowledged: 0,
            reconnect_attempts: 0,
            last_liveness_tick: 0,
            pending_control: None,
        }
    }

    pub fn establish(&mut self, at_tick: u64) -> Result<(), DistributedReason> {
        if self.state != DistributedSessionState::Handshaking {
            return Err(DistributedReason::HandshakeMismatch);
        }
        self.state = DistributedSessionState::Open;
        self.last_liveness_tick = at_tick;
        Ok(())
    }

    /// Reserve the next value sequence before handing bytes to a backend.
    pub fn begin_send(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        payload_bytes: u32,
    ) -> Result<u64, DistributedReason> {
        if self.state != DistributedSessionState::Open {
            return Err(DistributedReason::TerminalViolation);
        }
        if payload_bytes > binding.budget.maximum_payload_bytes {
            return Err(DistributedReason::OversizedFrame);
        }
        if binding.acknowledgement == AcknowledgementMode::Cumulative
            && self.unacknowledged >= binding.budget.maximum_unacknowledged
        {
            return Err(DistributedReason::BufferFull);
        }
        let sequence = self.next_send_sequence;
        self.next_send_sequence = sequence
            .checked_add(1)
            .ok_or(DistributedReason::SequenceViolation)?;
        if binding.acknowledgement == AcknowledgementMode::Cumulative {
            self.unacknowledged = self
                .unacknowledged
                .checked_add(1)
                .ok_or(DistributedReason::BufferFull)?;
        }
        Ok(sequence)
    }

    /// Accept a cumulative acknowledgement without manufacturing progress.
    pub fn acknowledge(&mut self, through: u64) -> Result<u16, DistributedReason> {
        if self.state != DistributedSessionState::Open
            || through >= self.next_send_sequence
            || self
                .acknowledged_through
                .is_some_and(|prior| through < prior)
        {
            return Err(DistributedReason::SequenceViolation);
        }
        let prior = self
            .acknowledged_through
            .map_or(0, |value| value.saturating_add(1));
        let freed_u64 = through
            .checked_add(1)
            .and_then(|value| value.checked_sub(prior))
            .ok_or(DistributedReason::SequenceViolation)?;
        let freed = u16::try_from(freed_u64).map_err(|_| DistributedReason::SequenceViolation)?;
        if freed > self.unacknowledged {
            return Err(DistributedReason::SequenceViolation);
        }
        self.unacknowledged -= freed;
        self.acknowledged_through = Some(through);
        Ok(freed)
    }

    /// Validate a retry against the exact finite retransmission window.
    pub fn retry(
        &self,
        binding: &PlanDistributedCord<'_>,
        sequence: u64,
        attempt: u16,
    ) -> Result<(), DistributedReason> {
        if binding.delivery != DistributedDelivery::AtLeastOnce
            || self.state != DistributedSessionState::Open
            || sequence >= self.next_send_sequence
            || self
                .acknowledged_through
                .is_some_and(|through| sequence <= through)
        {
            return Err(DistributedReason::SequenceViolation);
        }
        if attempt == 0 || attempt > binding.budget.maximum_retries {
            return Err(DistributedReason::RetryExhausted);
        }
        let outstanding = self.next_send_sequence.saturating_sub(sequence);
        if outstanding > u64::from(binding.budget.retry_items) {
            return Err(DistributedReason::RetryExhausted);
        }
        Ok(())
    }

    /// Classify an incoming sequence under exact ordering and dedup bounds.
    pub fn receive(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        sequence: u64,
        payload_bytes: u32,
    ) -> Result<ReceiveDisposition, DistributedReason> {
        if self.state != DistributedSessionState::Open {
            return Err(DistributedReason::TerminalViolation);
        }
        if payload_bytes > binding.budget.maximum_payload_bytes {
            return Err(DistributedReason::OversizedFrame);
        }
        if sequence == self.next_receive_sequence {
            self.next_receive_sequence = sequence
                .checked_add(1)
                .ok_or(DistributedReason::SequenceViolation)?;
            return Ok(ReceiveDisposition::Accepted);
        }
        if sequence > self.next_receive_sequence {
            let distance = sequence - self.next_receive_sequence;
            if distance > u64::from(binding.budget.reorder_items) {
                return Err(DistributedReason::SequenceViolation);
            }
            return Ok(ReceiveDisposition::HeldForMissingSequence);
        }
        let age = self.next_receive_sequence - sequence;
        if binding.budget.dedup_items > 0 && age <= u64::from(binding.budget.dedup_items) {
            Ok(ReceiveDisposition::DuplicateSuppressed)
        } else if binding.delivery == DistributedDelivery::AtLeastOnce {
            Ok(ReceiveDisposition::DuplicateRedelivered)
        } else {
            Err(DistributedReason::DedupWindowExhausted)
        }
    }

    pub fn observe_liveness(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        at_tick: u64,
    ) -> Result<(), DistributedReason> {
        if at_tick < self.last_liveness_tick {
            return Err(DistributedReason::SequenceViolation);
        }
        if at_tick.saturating_sub(self.last_liveness_tick) >= binding.budget.liveness_timeout_ticks
        {
            self.disconnect(binding)?;
            return Err(DistributedReason::Partitioned);
        }
        self.last_liveness_tick = at_tick;
        Ok(())
    }

    pub fn disconnect(
        &mut self,
        binding: &PlanDistributedCord<'_>,
    ) -> Result<(), DistributedReason> {
        if self.state != DistributedSessionState::Open {
            return Err(DistributedReason::TerminalViolation);
        }
        self.state = match binding.disconnect {
            DisconnectPolicy::AwaitReconnect => DistributedSessionState::Disconnected,
            DisconnectPolicy::CancelCord => DistributedSessionState::CancelPending,
            DisconnectPolicy::FailScope => DistributedSessionState::Failed,
        };
        if binding.disconnect == DisconnectPolicy::CancelCord {
            self.pending_control = Some(PendingControl::Cancellation);
        }
        Ok(())
    }

    pub fn resume_same_epoch(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        plan_identity: SemanticHash,
        proof: ResumeProof,
    ) -> Result<(), DistributedReason> {
        if self.state != DistributedSessionState::Disconnected
            || binding.reconnect != ReconnectMode::ResumeSameEpoch
        {
            return Err(DistributedReason::ReconnectDenied);
        }
        self.reconnect_attempts = self
            .reconnect_attempts
            .checked_add(1)
            .ok_or(DistributedReason::ReconnectDenied)?;
        if self.reconnect_attempts > binding.budget.maximum_reconnect_attempts
            || proof.receipt == SemanticHash::from_bytes([0; 32])
            || proof.plan_identity != plan_identity
            || proof.binding_identity != binding.identity
            || proof.session_epoch != self.session_epoch
            || proof.writer_next_sequence != self.next_send_sequence
            || proof.reader_next_sequence != self.next_receive_sequence
            || proof.acknowledged_through != self.acknowledged_through
        {
            return Err(DistributedReason::EpochMismatch);
        }
        self.state = DistributedSessionState::Open;
        Ok(())
    }

    pub fn begin_new_epoch(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        next_epoch: u64,
    ) -> Result<(), DistributedReason> {
        if self.state != DistributedSessionState::Disconnected
            || binding.reconnect != ReconnectMode::BeginNewEpoch
            || self.session_epoch.checked_add(1) != Some(next_epoch)
        {
            return Err(DistributedReason::EpochMismatch);
        }
        self.reconnect_attempts = self
            .reconnect_attempts
            .checked_add(1)
            .ok_or(DistributedReason::ReconnectDenied)?;
        if self.reconnect_attempts > binding.budget.maximum_reconnect_attempts {
            return Err(DistributedReason::ReconnectDenied);
        }
        self.session_epoch = next_epoch;
        self.next_send_sequence = 0;
        self.next_receive_sequence = 0;
        self.acknowledged_through = None;
        self.unacknowledged = 0;
        self.state = DistributedSessionState::Open;
        Ok(())
    }

    pub fn request_cancel(&mut self) -> Result<(), DistributedReason> {
        if !matches!(
            self.state,
            DistributedSessionState::Open | DistributedSessionState::Disconnected
        ) {
            return Err(DistributedReason::TerminalViolation);
        }
        self.state = DistributedSessionState::CancelPending;
        self.pending_control = Some(PendingControl::Cancellation);
        Ok(())
    }

    pub fn request_terminal(&mut self, class: TerminalClass) -> Result<(), DistributedReason> {
        if self.state != DistributedSessionState::Open {
            return Err(DistributedReason::TerminalViolation);
        }
        self.state = DistributedSessionState::TerminalPending;
        self.pending_control = Some(PendingControl::Terminal(class));
        Ok(())
    }

    pub fn acknowledge_control(
        &mut self,
        control: PendingControl,
    ) -> Result<(), DistributedReason> {
        if self.pending_control != Some(control)
            || !matches!(
                self.state,
                DistributedSessionState::CancelPending | DistributedSessionState::TerminalPending
            )
        {
            return Err(DistributedReason::TerminalViolation);
        }
        self.pending_control = None;
        self.state = DistributedSessionState::Closed;
        Ok(())
    }
}

/// Transport evidence kind. This is a structured implementation observation,
/// not a semantic value or a durable event stream by implication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedEvidenceKind {
    HandshakeAccepted,
    HandshakeRejected,
    ValueSent,
    ValueReceived,
    Acknowledged,
    Retried,
    DuplicateSuppressed,
    DuplicateRedelivered,
    Reordered,
    Pressure,
    Heartbeat,
    FrameRejected,
    FrameDropped,
    Disconnected,
    Reconnected,
    Cancelled,
    Terminal,
}

/// Causally correlatable transport observation emitted by a backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedEvidence<'a> {
    pub plan_identity: SemanticHash,
    pub binding_identity: SemanticHash,
    pub cord: Id<'a>,
    pub session: Id<'a>,
    pub session_epoch: u64,
    pub local_host_observation: Id<'a>,
    pub remote_host_observation: Id<'a>,
    pub sequence: Option<u64>,
    pub attempt: Option<u16>,
    pub correlation: Option<Id<'a>>,
    pub kind: DistributedEvidenceKind,
    pub reason: Option<DistributedReason>,
}

fn valid_peer(value: DistributedPeerRequirement<'_>) -> bool {
    crate::InstancePath::new(value.node.as_str()).is_ok()
        && valid_id(value.host_observation)
        && valid_id(value.realm)
        && valid_id(value.entity)
        && valid_id(value.credential)
        && valid_id(value.key)
        && valid_id(value.audience)
        && value.passport_schema_version > 0
        && value.credential_epoch > 0
        && value.key_epoch > 0
        && valid_pin(value.status_reporter)
        && valid_pin(value.credential_verifier)
        && value.realm_identity != SemanticHash::from_bytes([0; 32])
        && value.passport != SemanticHash::from_bytes([0; 32])
        && value.grant_hash != SemanticHash::from_bytes([0; 32])
}

fn valid_id(value: Id<'_>) -> bool {
    Id::new(value.as_str()).is_ok()
}

fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    valid_id(value.id)
        && value.schema_version > 0
        && value.semantic_hash != SemanticHash::from_bytes([0; 32])
}

fn field<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn pin_fields<'a>(value: &'a PinnedDescriptor<'a>) -> [MapField<'a>; 3] {
    [
        field("id", CanonicalValue::Identifier(value.id)),
        field(
            "schema_version",
            CanonicalValue::Integer(i128::from(value.schema_version)),
        ),
        field(
            "semantic_hash",
            CanonicalValue::Bytes(value.semantic_hash.as_bytes()),
        ),
    ]
}

fn peer_fields<'a>(
    value: &'a DistributedPeerRequirement<'a>,
    status: &'a [MapField<'a>; 3],
    verifier: &'a [MapField<'a>; 3],
) -> [MapField<'a>; 15] {
    [
        field("node", CanonicalValue::Text(value.node.as_str())),
        field(
            "host_observation",
            CanonicalValue::Identifier(value.host_observation),
        ),
        field("realm", CanonicalValue::Identifier(value.realm)),
        field(
            "realm_identity",
            CanonicalValue::Bytes(value.realm_identity.as_bytes()),
        ),
        field("entity", CanonicalValue::Identifier(value.entity)),
        field("passport", CanonicalValue::Bytes(value.passport.as_bytes())),
        field(
            "passport_schema_version",
            CanonicalValue::Integer(i128::from(value.passport_schema_version)),
        ),
        field("credential", CanonicalValue::Identifier(value.credential)),
        field(
            "credential_epoch",
            CanonicalValue::Integer(i128::from(value.credential_epoch)),
        ),
        field("key", CanonicalValue::Identifier(value.key)),
        field(
            "key_epoch",
            CanonicalValue::Integer(i128::from(value.key_epoch)),
        ),
        field("status_reporter", CanonicalValue::Map(status)),
        field("credential_verifier", CanonicalValue::Map(verifier)),
        field("audience", CanonicalValue::Identifier(value.audience)),
        field(
            "grant_hash",
            CanonicalValue::Bytes(value.grant_hash.as_bytes()),
        ),
    ]
}

fn flow_fields(value: FlowPolicy<'_>) -> [MapField<'_>; 8] {
    let (parameter, sample_every, sample_offset) = match value.pressure {
        crate::Pressure::Block(_) => ("fifo", 0, 0),
        crate::Pressure::Coalesce { relation } => (relation.as_str(), 0, 0),
        crate::Pressure::Sample(schedule) => ("", schedule.every(), schedule.offset()),
        crate::Pressure::Reject
        | crate::Pressure::DropDisposable
        | crate::Pressure::Disconnect
        | crate::Pressure::Fail => ("", 0, 0),
    };
    [
        field(
            "capacity_items",
            CanonicalValue::Integer(i128::from(value.capacity.items())),
        ),
        field(
            "maximum_value_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.max_value_bytes())),
        ),
        field(
            "maximum_queued_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.max_queued_bytes())),
        ),
        field(
            "pressure",
            CanonicalValue::Identifier(Id(value.pressure.as_str())),
        ),
        field("pressure_parameter", CanonicalValue::Text(parameter)),
        field(
            "sample_every",
            CanonicalValue::Integer(i128::from(sample_every)),
        ),
        field(
            "sample_offset",
            CanonicalValue::Integer(i128::from(sample_offset)),
        ),
        field(
            "watermarks",
            CanonicalValue::Integer(
                (i128::from(value.watermarks.low_items()) << 16)
                    | i128::from(value.watermarks.high_items()),
            ),
        ),
    ]
}

fn distributed_budget_fields(value: DistributedCordBudget) -> [MapField<'static>; 19] {
    [
        field(
            "send_items",
            CanonicalValue::Integer(i128::from(value.send_items)),
        ),
        field(
            "send_bytes",
            CanonicalValue::Integer(i128::from(value.send_bytes)),
        ),
        field(
            "receive_items",
            CanonicalValue::Integer(i128::from(value.receive_items)),
        ),
        field(
            "receive_bytes",
            CanonicalValue::Integer(i128::from(value.receive_bytes)),
        ),
        field(
            "retry_items",
            CanonicalValue::Integer(i128::from(value.retry_items)),
        ),
        field(
            "retry_bytes",
            CanonicalValue::Integer(i128::from(value.retry_bytes)),
        ),
        field(
            "reorder_items",
            CanonicalValue::Integer(i128::from(value.reorder_items)),
        ),
        field(
            "reorder_bytes",
            CanonicalValue::Integer(i128::from(value.reorder_bytes)),
        ),
        field(
            "dedup_items",
            CanonicalValue::Integer(i128::from(value.dedup_items)),
        ),
        field(
            "maximum_payload_bytes",
            CanonicalValue::Integer(i128::from(value.maximum_payload_bytes)),
        ),
        field(
            "maximum_frame_bytes",
            CanonicalValue::Integer(i128::from(value.maximum_frame_bytes)),
        ),
        field(
            "maximum_unacknowledged",
            CanonicalValue::Integer(i128::from(value.maximum_unacknowledged)),
        ),
        field(
            "maximum_retries",
            CanonicalValue::Integer(i128::from(value.maximum_retries)),
        ),
        field(
            "maximum_reconnect_attempts",
            CanonicalValue::Integer(i128::from(value.maximum_reconnect_attempts)),
        ),
        field(
            "heartbeat_interval_ticks",
            CanonicalValue::Integer(i128::from(value.heartbeat_interval_ticks)),
        ),
        field(
            "liveness_timeout_ticks",
            CanonicalValue::Integer(i128::from(value.liveness_timeout_ticks)),
        ),
        field(
            "reconnect_deadline_ticks",
            CanonicalValue::Integer(i128::from(value.reconnect_deadline_ticks)),
        ),
        field(
            "maximum_evidence_events",
            CanonicalValue::Integer(i128::from(value.maximum_evidence_events)),
        ),
        field(
            "allocated_memory_bytes",
            CanonicalValue::Integer(i128::from(value.allocated_memory_bytes)),
        ),
    ]
}

fn resource_budget_fields(value: PlanResourceBudget) -> [MapField<'static>; 7] {
    [
        field(
            "memory_bytes",
            CanonicalValue::Integer(i128::from(value.memory_bytes)),
        ),
        field(
            "storage_bytes",
            CanonicalValue::Integer(i128::from(value.storage_bytes)),
        ),
        field(
            "cpu_units",
            CanonicalValue::Integer(i128::from(value.cpu_units)),
        ),
        field("timers", CanonicalValue::Integer(i128::from(value.timers))),
        field(
            "transports",
            CanonicalValue::Integer(i128::from(value.transports)),
        ),
        field(
            "checkpoints",
            CanonicalValue::Integer(i128::from(value.checkpoints)),
        ),
        field(
            "evidence_bytes",
            CanonicalValue::Integer(i128::from(value.evidence_bytes)),
        ),
    ]
}
