//! Carrier-neutral hosted transport selection and envelope codec.
//!
//! Carrier crates consume these types without moving their configuration,
//! endpoint, or library types into `conduit-core`.

pub use conduit_core::CarrierSecurityMode;
use conduit_core::{
    Id, PinnedDescriptor, PlanArtifact, PlanDistributedCord, SemanticHash, TerminalClass,
};

use crate::{DistributedFrameKind, OutboundDistributedFrame, ResolvedPlacementBinding};

pub const DISTRIBUTED_ENVELOPE_VERSION: u16 = 1;
pub const DISTRIBUTED_ENVELOPE_FIXED_BYTES: usize = 132;

const MAGIC: [u8; 4] = *b"CNDT";
const FLAG_SEQUENCE: u16 = 1 << 0;
const FLAG_ATTEMPT: u16 = 1 << 1;
const FLAG_CORRELATION: u16 = 1 << 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierSecurityCapabilities {
    pub plaintext: bool,
    pub tls: bool,
    pub mutual_tls: bool,
}

impl CarrierSecurityCapabilities {
    #[must_use]
    pub const fn supports(self, mode: CarrierSecurityMode) -> bool {
        match mode {
            CarrierSecurityMode::Plaintext => self.plaintext,
            CarrierSecurityMode::Tls => self.tls,
            CarrierSecurityMode::MutualTls => self.mutual_tls,
        }
    }
}

/// Complete backend-stack ceilings relevant to one distributed session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportCapabilities {
    pub protocol_version: u16,
    pub publish_subscribe: bool,
    pub query_reply: bool,
    pub reconnect: bool,
    pub deterministic_faults: bool,
    pub security: CarrierSecurityCapabilities,
    pub maximum_frame_bytes: u32,
    pub adapter_send_items: u16,
    pub adapter_receive_items: u16,
    pub adapter_evidence_items: u16,
    pub carrier_queue_items: u16,
    pub carrier_queue_bytes: u64,
    pub receive_buffer_bytes: u64,
    pub defragmentation_bytes: u64,
    pub socket_send_bytes: u64,
    pub socket_receive_bytes: u64,
    pub session_state_bytes: u64,
    pub discovery_state_bytes: u64,
    pub pending_operation_bytes: u64,
    pub retained_payload_bytes: u64,
    pub timer_state_bytes: u64,
    pub worker_stack_bytes: u64,
    pub pending_links: u16,
    pub maximum_links: u16,
    pub maximum_sessions: u16,
    pub retry_timers: u16,
    /// False means at least one library/kernel bound is only observed.
    pub complete_stack_hard_bounded: bool,
}

impl TransportCapabilities {
    #[must_use]
    pub fn accounted_memory_bytes(self) -> Option<u64> {
        self.carrier_queue_bytes
            .checked_add(self.receive_buffer_bytes)?
            .checked_add(self.defragmentation_bytes)?
            .checked_add(self.socket_send_bytes)?
            .checked_add(self.socket_receive_bytes)?
            .checked_add(self.session_state_bytes)?
            .checked_add(self.discovery_state_bytes)?
            .checked_add(self.pending_operation_bytes)?
            .checked_add(self.retained_payload_bytes)?
            .checked_add(self.timer_state_bytes)?
            .checked_add(self.worker_stack_bytes)
    }
}

/// Exact host-resolved transport selection above the portable plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedTransportSelection<'a> {
    pub backend: PinnedDescriptor<'a>,
    pub artifact: PlanArtifact<'a>,
    pub execution_profile: PinnedDescriptor<'a>,
    pub endpoint: &'a str,
    pub carrier_binding: Id<'a>,
    pub security_descriptor: PinnedDescriptor<'a>,
    pub security_mode: CarrierSecurityMode,
    pub capabilities: TransportCapabilities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportReason {
    UnsupportedProtocol,
    BindingMismatch,
    ImplementationMismatch,
    ArtifactMismatch,
    ProfileMismatch,
    EndpointMismatch,
    UnsupportedSecurity,
    ResourceUnderaccounted,
    EnvelopeMalformed,
    EnvelopeIdentityMismatch,
    EnvelopeTooLarge,
    QueueFull,
    CarrierFailure,
    Disconnected,
    SecretHandleMissing,
    FfiBoundaryMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportTransition {
    Unchanged,
    NewSessionEpoch,
}

impl TransportReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedProtocol => "CND-TRN-001",
            Self::BindingMismatch => "CND-TRN-002",
            Self::ImplementationMismatch => "CND-TRN-003",
            Self::ArtifactMismatch => "CND-TRN-004",
            Self::ProfileMismatch => "CND-TRN-005",
            Self::EndpointMismatch => "CND-TRN-006",
            Self::UnsupportedSecurity => "CND-TRN-007",
            Self::ResourceUnderaccounted => "CND-TRN-008",
            Self::EnvelopeMalformed => "CND-TRN-009",
            Self::EnvelopeIdentityMismatch => "CND-TRN-010",
            Self::EnvelopeTooLarge => "CND-TRN-011",
            Self::QueueFull => "CND-TRN-012",
            Self::CarrierFailure => "CND-TRN-013",
            Self::Disconnected => "CND-TRN-014",
            Self::SecretHandleMissing => "CND-TRN-015",
            Self::FfiBoundaryMismatch => "CND-TRN-016",
        }
    }
}

/// Prove that a carrier selection is exactly the one retained by both the
/// host resolver result and the schema-2 distributed binding.
pub fn validate_transport_selection(
    binding: &PlanDistributedCord<'_>,
    placement: &ResolvedPlacementBinding,
    selected: ResolvedTransportSelection<'_>,
) -> Result<(), TransportReason> {
    conduit_core::validate_distributed_binding(binding)
        .map_err(|_| TransportReason::BindingMismatch)?;
    let planned_artifact = binding
        .backend_artifact
        .ok_or(TransportReason::ArtifactMismatch)?;
    let planned_profile = binding
        .backend_profile
        .ok_or(TransportReason::ProfileMismatch)?;
    let planned_endpoint = binding
        .carrier_endpoint
        .ok_or(TransportReason::EndpointMismatch)?;
    if selected.capabilities.protocol_version != DISTRIBUTED_ENVELOPE_VERSION {
        return Err(TransportReason::UnsupportedProtocol);
    }
    if binding.backend != selected.backend
        || placement.implementation_id != selected.backend.id.as_str()
        || placement.implementation_identity != selected.backend.semantic_hash
    {
        return Err(TransportReason::ImplementationMismatch);
    }
    if planned_artifact != selected.artifact
        || !placement.artifacts.iter().any(|(id, digest)| {
            id == selected.artifact.id.as_str() && *digest == selected.artifact.digest
        })
    {
        return Err(TransportReason::ArtifactMismatch);
    }
    if planned_profile != selected.execution_profile {
        return Err(TransportReason::ProfileMismatch);
    }
    if planned_endpoint != selected.endpoint
        || binding.carrier_binding != selected.carrier_binding
        || binding.carrier_security != selected.security_descriptor
    {
        return Err(TransportReason::EndpointMismatch);
    }
    if binding.carrier_security_mode != Some(selected.security_mode) {
        return Err(TransportReason::UnsupportedSecurity);
    }
    if !selected
        .capabilities
        .security
        .supports(selected.security_mode)
    {
        return Err(TransportReason::UnsupportedSecurity);
    }
    let required_memory = selected
        .capabilities
        .accounted_memory_bytes()
        .ok_or(TransportReason::ResourceUnderaccounted)?;
    if selected.capabilities.maximum_frame_bytes < binding.budget.maximum_frame_bytes
        || selected.capabilities.adapter_send_items > binding.budget.send_items
        || selected.capabilities.adapter_receive_items > binding.budget.receive_items
        || selected.capabilities.adapter_evidence_items > binding.budget.maximum_evidence_events
        || selected.capabilities.carrier_queue_items == 0
        || selected.capabilities.pending_links == 0
        || selected.capabilities.maximum_links == 0
        || selected.capabilities.maximum_sessions == 0
        || selected.capabilities.retry_timers > binding.allocation.timers
        || required_memory > binding.allocation.memory_bytes
    {
        return Err(TransportReason::ResourceUnderaccounted);
    }
    Ok(())
}

/// Validate the carrier-neutral part of a transport replacement.
///
/// This does not authorize a transition or mutate an active session. It only
/// prevents a replacement plan from silently weakening the cord contract or
/// carrier protection, and requires a new session epoch for every changed
/// exact binding.
pub fn validate_transport_transition(
    current: &PlanDistributedCord<'_>,
    current_security: CarrierSecurityMode,
    next: &PlanDistributedCord<'_>,
    next_security: CarrierSecurityMode,
) -> Result<TransportTransition, TransportReason> {
    conduit_core::validate_distributed_binding(current)
        .map_err(|_| TransportReason::BindingMismatch)?;
    conduit_core::validate_distributed_binding(next)
        .map_err(|_| TransportReason::BindingMismatch)?;
    if current.cord != next.cord
        || current.writer_port_contract_hash != next.writer_port_contract_hash
        || current.reader_port_contract_hash != next.reader_port_contract_hash
        || current.flow != next.flow
        || current.delivery != next.delivery
        || current.acknowledgement != next.acknowledgement
        || current.ordering != next.ordering
        || current.reconnect != next.reconnect
        || current.disconnect != next.disconnect
        || current.budget.maximum_payload_bytes != next.budget.maximum_payload_bytes
    {
        return Err(TransportReason::BindingMismatch);
    }
    if security_strength(next_security) < security_strength(current_security) {
        return Err(TransportReason::UnsupportedSecurity);
    }
    if current.identity == next.identity {
        return Ok(TransportTransition::Unchanged);
    }
    if next.initial_session_epoch <= current.initial_session_epoch {
        return Err(TransportReason::BindingMismatch);
    }
    Ok(TransportTransition::NewSessionEpoch)
}

const fn security_strength(mode: CarrierSecurityMode) -> u8 {
    match mode {
        CarrierSecurityMode::Plaintext => 0,
        CarrierSecurityMode::Tls => 1,
        CarrierSecurityMode::MutualTls => 2,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedDistributedEnvelope<'a> {
    pub plan_identity: SemanticHash,
    pub binding_identity: SemanticHash,
    pub cord: &'a str,
    pub session: &'a str,
    pub frame: OutboundDistributedFrame<'a>,
}

pub fn encode_distributed_envelope(
    plan_identity: SemanticHash,
    binding: &PlanDistributedCord<'_>,
    frame: OutboundDistributedFrame<'_>,
    destination: &mut [u8],
) -> Result<usize, TransportReason> {
    if frame.session_epoch == 0
        || frame.payload.len() > binding.budget.maximum_payload_bytes as usize
    {
        return Err(TransportReason::EnvelopeTooLarge);
    }
    let cord = binding.cord.as_str().as_bytes();
    let session = binding.session.as_str().as_bytes();
    let cord_len = u16::try_from(cord.len()).map_err(|_| TransportReason::EnvelopeTooLarge)?;
    let session_len =
        u16::try_from(session.len()).map_err(|_| TransportReason::EnvelopeTooLarge)?;
    let payload_len =
        u32::try_from(frame.payload.len()).map_err(|_| TransportReason::EnvelopeTooLarge)?;
    let total = DISTRIBUTED_ENVELOPE_FIXED_BYTES
        .checked_add(cord.len())
        .and_then(|value| value.checked_add(session.len()))
        .and_then(|value| value.checked_add(frame.payload.len()))
        .ok_or(TransportReason::EnvelopeTooLarge)?;
    if total > binding.budget.maximum_frame_bytes as usize || destination.len() < total {
        return Err(TransportReason::EnvelopeTooLarge);
    }
    destination[..4].copy_from_slice(&MAGIC);
    destination[4..6].copy_from_slice(&DISTRIBUTED_ENVELOPE_VERSION.to_be_bytes());
    let (kind, terminal) = encode_kind(frame.kind);
    destination[6] = kind;
    destination[7] = terminal;
    destination[8..16].copy_from_slice(&frame.session_epoch.to_be_bytes());
    destination[16..24].copy_from_slice(&frame.sequence.unwrap_or(0).to_be_bytes());
    destination[24..26].copy_from_slice(&frame.attempt.unwrap_or(0).to_be_bytes());
    let mut flags = 0_u16;
    flags |= u16::from(frame.sequence.is_some()) * FLAG_SEQUENCE;
    flags |= u16::from(frame.attempt.is_some()) * FLAG_ATTEMPT;
    flags |= u16::from(frame.correlation.is_some()) * FLAG_CORRELATION;
    destination[26..28].copy_from_slice(&flags.to_be_bytes());
    destination[28..60].fill(0);
    if let Some(correlation) = frame.correlation {
        destination[28..60].copy_from_slice(correlation.as_bytes());
    }
    destination[60..92].copy_from_slice(plan_identity.as_bytes());
    destination[92..124].copy_from_slice(binding.identity.as_bytes());
    destination[124..126].copy_from_slice(&cord_len.to_be_bytes());
    destination[126..128].copy_from_slice(&session_len.to_be_bytes());
    destination[128..132].copy_from_slice(&payload_len.to_be_bytes());
    let mut cursor = DISTRIBUTED_ENVELOPE_FIXED_BYTES;
    destination[cursor..cursor + cord.len()].copy_from_slice(cord);
    cursor += cord.len();
    destination[cursor..cursor + session.len()].copy_from_slice(session);
    cursor += session.len();
    destination[cursor..cursor + frame.payload.len()].copy_from_slice(frame.payload);
    Ok(total)
}

pub fn decode_distributed_envelope<'a>(
    input: &'a [u8],
    expected_plan_identity: SemanticHash,
    binding: &PlanDistributedCord<'_>,
) -> Result<DecodedDistributedEnvelope<'a>, TransportReason> {
    if input.len() < DISTRIBUTED_ENVELOPE_FIXED_BYTES
        || input.len() > binding.budget.maximum_frame_bytes as usize
        || input[..4] != MAGIC
    {
        return Err(TransportReason::EnvelopeMalformed);
    }
    if read_u16(input, 4)? != DISTRIBUTED_ENVELOPE_VERSION {
        return Err(TransportReason::UnsupportedProtocol);
    }
    let kind = decode_kind(input[6], input[7])?;
    let session_epoch = read_u64(input, 8)?;
    let flags = read_u16(input, 26)?;
    if flags & !(FLAG_SEQUENCE | FLAG_ATTEMPT | FLAG_CORRELATION) != 0 || session_epoch == 0 {
        return Err(TransportReason::EnvelopeMalformed);
    }
    let sequence = (flags & FLAG_SEQUENCE != 0)
        .then(|| read_u64(input, 16))
        .transpose()?;
    let attempt = (flags & FLAG_ATTEMPT != 0)
        .then(|| read_u16(input, 24))
        .transpose()?;
    let correlation = if flags & FLAG_CORRELATION != 0 {
        Some(SemanticHash::from_bytes(
            input[28..60]
                .try_into()
                .map_err(|_| TransportReason::EnvelopeMalformed)?,
        ))
    } else {
        if input[28..60].iter().any(|byte| *byte != 0) {
            return Err(TransportReason::EnvelopeMalformed);
        }
        None
    };
    let plan_identity = SemanticHash::from_bytes(
        input[60..92]
            .try_into()
            .map_err(|_| TransportReason::EnvelopeMalformed)?,
    );
    let binding_identity = SemanticHash::from_bytes(
        input[92..124]
            .try_into()
            .map_err(|_| TransportReason::EnvelopeMalformed)?,
    );
    if plan_identity != expected_plan_identity || binding_identity != binding.identity {
        return Err(TransportReason::EnvelopeIdentityMismatch);
    }
    let cord_len = usize::from(read_u16(input, 124)?);
    let session_len = usize::from(read_u16(input, 126)?);
    let payload_len =
        usize::try_from(read_u32(input, 128)?).map_err(|_| TransportReason::EnvelopeTooLarge)?;
    if payload_len > binding.budget.maximum_payload_bytes as usize {
        return Err(TransportReason::EnvelopeTooLarge);
    }
    let expected = DISTRIBUTED_ENVELOPE_FIXED_BYTES
        .checked_add(cord_len)
        .and_then(|value| value.checked_add(session_len))
        .and_then(|value| value.checked_add(payload_len))
        .ok_or(TransportReason::EnvelopeTooLarge)?;
    if expected != input.len() {
        return Err(TransportReason::EnvelopeMalformed);
    }
    let cord_start = DISTRIBUTED_ENVELOPE_FIXED_BYTES;
    let session_start = cord_start + cord_len;
    let payload_start = session_start + session_len;
    let cord = core::str::from_utf8(&input[cord_start..session_start])
        .map_err(|_| TransportReason::EnvelopeMalformed)?;
    let session = core::str::from_utf8(&input[session_start..payload_start])
        .map_err(|_| TransportReason::EnvelopeMalformed)?;
    if cord != binding.cord.as_str()
        || session != binding.session.as_str()
        || session_epoch < binding.initial_session_epoch
    {
        return Err(TransportReason::EnvelopeIdentityMismatch);
    }
    Ok(DecodedDistributedEnvelope {
        plan_identity,
        binding_identity,
        cord,
        session,
        frame: OutboundDistributedFrame {
            kind,
            session_epoch,
            sequence,
            attempt,
            correlation,
            payload: &input[payload_start..],
        },
    })
}

fn encode_kind(kind: DistributedFrameKind) -> (u8, u8) {
    match kind {
        DistributedFrameKind::Value => (0, 0),
        DistributedFrameKind::Acknowledgement => (1, 0),
        DistributedFrameKind::Heartbeat => (2, 0),
        DistributedFrameKind::Cancellation => (3, 0),
        DistributedFrameKind::CancellationAcknowledgement => (4, 0),
        DistributedFrameKind::Terminal(class) => {
            let terminal = match class {
                TerminalClass::Succeeded => 0,
                TerminalClass::Disconnected => 1,
                TerminalClass::Cancelled => 2,
                TerminalClass::Failed => 3,
            };
            (5, terminal)
        }
        DistributedFrameKind::TerminalAcknowledgement => (6, 0),
    }
}

fn decode_kind(kind: u8, terminal: u8) -> Result<DistributedFrameKind, TransportReason> {
    let value = match kind {
        0 if terminal == 0 => DistributedFrameKind::Value,
        1 if terminal == 0 => DistributedFrameKind::Acknowledgement,
        2 if terminal == 0 => DistributedFrameKind::Heartbeat,
        3 if terminal == 0 => DistributedFrameKind::Cancellation,
        4 if terminal == 0 => DistributedFrameKind::CancellationAcknowledgement,
        5 => DistributedFrameKind::Terminal(match terminal {
            0 => TerminalClass::Succeeded,
            1 => TerminalClass::Disconnected,
            2 => TerminalClass::Cancelled,
            3 => TerminalClass::Failed,
            _ => return Err(TransportReason::EnvelopeMalformed),
        }),
        6 if terminal == 0 => DistributedFrameKind::TerminalAcknowledgement,
        _ => return Err(TransportReason::EnvelopeMalformed),
    };
    Ok(value)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, TransportReason> {
    input
        .get(offset..offset + 2)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_be_bytes)
        .ok_or(TransportReason::EnvelopeMalformed)
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, TransportReason> {
    input
        .get(offset..offset + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or(TransportReason::EnvelopeMalformed)
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, TransportReason> {
    input
        .get(offset..offset + 8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_be_bytes)
        .ok_or(TransportReason::EnvelopeMalformed)
}
