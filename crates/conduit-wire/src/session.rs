use conduit_core::{
    bind_active_play, ActivePlayId, ConnectionId, ConnectionProvider, ConnectionProviderInstanceId,
    FragmentId, KindId, LinkBindingId, LinkEndpoint, LinkLimits, PlanId, PlannedConnection,
    PROTOCOL_VERSION,
};

use crate::{WireError, MAX_ID_BYTES};

const SESSION_MAGIC: [u8; 4] = *b"CNDS";
const SESSION_WIRE_VERSION: u8 = 1;
const COMMON_FIXED_BYTES: usize = 4 + 1 + 1 + 2 + 2 * 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBinding {
    pub protocol_version: u16,
    pub plan_id: PlanId,
    pub source_fragment_id: FragmentId,
    pub sink_fragment_id: FragmentId,
    pub source_active_play_id: ActivePlayId,
    pub sink_active_play_id: ActivePlayId,
    pub connection_id: ConnectionId,
    pub link_binding_id: LinkBindingId,
    pub provider: ConnectionProvider,
    pub provider_instance_id: ConnectionProviderInstanceId,
    pub source: LinkEndpoint,
    pub sink: LinkEndpoint,
    pub value_kind: KindId,
    pub limits: LinkLimits,
}

impl SessionBinding {
    /// Bind one directional wire session to an exact planned remote connection.
    pub fn from_planned_connection(
        plan_id: PlanId,
        source_fragment_id: FragmentId,
        sink_fragment_id: FragmentId,
        connection: &PlannedConnection,
    ) -> Result<Self, WireError> {
        let link = connection
            .link_binding
            .as_ref()
            .ok_or(WireError::InvalidSession)?;
        if !connection.provider.supports_remote_session()
            || link.provider != connection.provider
            || !connection.permits_bound_link(&link.bound_link())
            || connection.item_capacity > link.limits.maximum_in_flight_items
            || connection.byte_capacity > link.limits.maximum_payload_bytes
            || connection.byte_capacity > link.limits.maximum_buffered_bytes
        {
            return Err(WireError::InvalidSession);
        }
        let source_active_play_id =
            bind_active_play(&plan_id, &link.source.host_id, &link.source.boot_id, 0)
                .active_play_id;
        let sink_active_play_id =
            bind_active_play(&plan_id, &link.sink.host_id, &link.sink.boot_id, 0).active_play_id;
        let binding = Self {
            protocol_version: PROTOCOL_VERSION,
            plan_id,
            source_fragment_id,
            sink_fragment_id,
            source_active_play_id,
            sink_active_play_id,
            connection_id: connection.connection_id.clone(),
            link_binding_id: link.binding_id.clone(),
            provider: link.provider,
            provider_instance_id: link.provider_instance_id.clone(),
            source: link.source.clone(),
            sink: link.sink.clone(),
            value_kind: connection.value_kind.clone(),
            limits: link.limits,
        };
        binding.validate()?;
        Ok(binding)
    }

    /// Materialize the boot-scoped session identity from an exact planned
    /// connection plus the two observed runtime boot facts. All planner-owned
    /// host, endpoint, provider, instance, link, connection, kind, fragment,
    /// and limit identities remain unchanged.
    pub fn with_observed_boots(
        mut self,
        source_boot_id: conduit_core::BootId,
        sink_boot_id: conduit_core::BootId,
    ) -> Result<Self, WireError> {
        self.validate()?;
        if source_boot_id.as_str().is_empty() || sink_boot_id.as_str().is_empty() {
            return Err(WireError::InvalidSession);
        }
        self.source.boot_id = source_boot_id;
        self.sink.boot_id = sink_boot_id;
        self.source_active_play_id =
            bind_active_play(&self.plan_id, &self.source.host_id, &self.source.boot_id, 0)
                .active_play_id;
        self.sink_active_play_id =
            bind_active_play(&self.plan_id, &self.sink.host_id, &self.sink.boot_id, 0)
                .active_play_id;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), WireError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(WireError::WrongProtocolVersion);
        }
        if !self.provider.supports_remote_session() {
            return Err(WireError::InvalidProvider);
        }
        let identities = [
            self.plan_id.as_str(),
            self.source_fragment_id.as_str(),
            self.sink_fragment_id.as_str(),
            self.source_active_play_id.as_str(),
            self.sink_active_play_id.as_str(),
            self.connection_id.as_str(),
            self.link_binding_id.as_str(),
            self.provider_instance_id.as_str(),
            self.source.host_id.as_str(),
            self.source.boot_id.as_str(),
            self.source.endpoint_id.as_str(),
            self.sink.host_id.as_str(),
            self.sink.boot_id.as_str(),
            self.sink.endpoint_id.as_str(),
            self.value_kind.as_str(),
        ];
        if identities
            .iter()
            .any(|identity| identity.is_empty() || identity.len() > MAX_ID_BYTES)
            || self.source.host_id == self.sink.host_id
            || self.source.endpoint_id == self.sink.endpoint_id
        {
            return Err(WireError::InvalidSession);
        }
        if self.source_active_play_id
            != bind_active_play(&self.plan_id, &self.source.host_id, &self.source.boot_id, 0)
                .active_play_id
            || self.sink_active_play_id
                != bind_active_play(&self.plan_id, &self.sink.host_id, &self.sink.boot_id, 0)
                    .active_play_id
        {
            return Err(WireError::InvalidSession);
        }
        if self.limits.maximum_in_flight_items == 0
            || self.limits.maximum_payload_bytes == 0
            || self.limits.maximum_buffered_bytes == 0
            || self.limits.maximum_frame_bytes == 0
            || self.limits.maximum_payload_bytes > self.limits.maximum_buffered_bytes
        {
            return Err(WireError::InvalidLimits);
        }
        let maximum_frame = usize::try_from(self.limits.maximum_frame_bytes)
            .map_err(|_| WireError::InvalidLimits)?;
        let maximum_payload = usize::try_from(self.limits.maximum_payload_bytes)
            .map_err(|_| WireError::InvalidLimits)?;
        if hello_encoded_len(self)? > maximum_frame
            || offered_encoded_len(self, maximum_payload)? > maximum_frame
        {
            return Err(WireError::InvalidLimits);
        }
        Ok(())
    }

    pub fn identity(&self) -> SessionIdentity<'_> {
        SessionIdentity {
            protocol_version: self.protocol_version,
            plan_id: self.plan_id.as_str(),
            source_fragment_id: self.source_fragment_id.as_str(),
            sink_fragment_id: self.sink_fragment_id.as_str(),
            source_active_play_id: self.source_active_play_id.as_str(),
            sink_active_play_id: self.sink_active_play_id.as_str(),
            connection_id: self.connection_id.as_str(),
            link_binding_id: self.link_binding_id.as_str(),
            provider_instance_id: self.provider_instance_id.as_str(),
        }
    }

    pub fn hello_frame(&self) -> SessionFrame<'_> {
        SessionFrame {
            identity: self.identity(),
            message: SessionMessage::Hello(SessionHello {
                provider: self.provider,
                source: endpoint_ref(&self.source),
                sink: endpoint_ref(&self.sink),
                value_kind: self.value_kind.as_str(),
                limits: self.limits,
            }),
        }
    }

    pub fn frame<'a>(&'a self, message: SessionMessage<'a>) -> SessionFrame<'a> {
        SessionFrame {
            identity: self.identity(),
            message,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionIdentity<'a> {
    pub protocol_version: u16,
    pub plan_id: &'a str,
    pub source_fragment_id: &'a str,
    pub sink_fragment_id: &'a str,
    pub source_active_play_id: &'a str,
    pub sink_active_play_id: &'a str,
    pub connection_id: &'a str,
    pub link_binding_id: &'a str,
    pub provider_instance_id: &'a str,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionEndpoint<'a> {
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub endpoint_id: &'a str,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionHello<'a> {
    pub provider: ConnectionProvider,
    pub source: SessionEndpoint<'a>,
    pub sink: SessionEndpoint<'a>,
    pub value_kind: &'a str,
    pub limits: LinkLimits,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SessionTerminalDisposition {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SessionMessage<'a> {
    Hello(SessionHello<'a>),
    Ready,
    Offered {
        sequence: u64,
        payload: &'a [u8],
    },
    Pressure {
        sequence: u64,
    },
    Accepted {
        sequence: u64,
    },
    Delivered {
        sequence: u64,
    },
    InputClosed {
        final_sequence: u64,
    },
    Cancelled {
        code: u16,
    },
    Failed {
        code: u16,
    },
    Terminal {
        disposition: SessionTerminalDisposition,
        final_sequence: u64,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionFrame<'a> {
    pub identity: SessionIdentity<'a>,
    pub message: SessionMessage<'a>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SessionRole {
    Source,
    Sink,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TransferState {
    Offered(u64),
    Accepted(u64),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SessionFailureState {
    Cancelled(u16),
    Failed(u16),
}

/// Allocation-stable lifecycle verifier for one exact directional session.
/// It owns no socket and performs no retry; carriers may only submit inbound
/// and outbound frames in the order admitted here.
pub struct SessionMachine {
    binding: SessionBinding,
    role: SessionRole,
    local_hello: bool,
    peer_hello: bool,
    local_ready: bool,
    peer_ready: bool,
    transfer: Option<TransferState>,
    next_sequence: u64,
    input_closed: bool,
    local_failure: Option<SessionFailureState>,
    peer_failure: Option<SessionFailureState>,
    local_terminal: Option<SessionTerminalDisposition>,
    peer_terminal: Option<SessionTerminalDisposition>,
}

impl SessionMachine {
    pub fn new(binding: SessionBinding, role: SessionRole) -> Result<Self, WireError> {
        binding.validate()?;
        Ok(Self {
            binding,
            role,
            local_hello: false,
            peer_hello: false,
            local_ready: false,
            peer_ready: false,
            transfer: None,
            next_sequence: 0,
            input_closed: false,
            local_failure: None,
            peer_failure: None,
            local_terminal: None,
            peer_terminal: None,
        })
    }

    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }

    pub fn is_active(&self) -> bool {
        self.local_ready
            && self.peer_ready
            && self.local_failure.is_none()
            && self.peer_failure.is_none()
            && self.local_terminal.is_none()
            && self.peer_terminal.is_none()
    }

    pub fn is_terminal(&self) -> bool {
        self.local_terminal.is_some() && self.peer_terminal.is_some()
    }

    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn admit_outbound(&mut self, frame: SessionFrame<'_>) -> Result<(), WireError> {
        self.admit(FrameDirection::Outbound, frame)
    }

    pub fn admit_inbound(&mut self, frame: SessionFrame<'_>) -> Result<(), WireError> {
        self.admit(FrameDirection::Inbound, frame)
    }

    fn admit(
        &mut self,
        direction: FrameDirection,
        frame: SessionFrame<'_>,
    ) -> Result<(), WireError> {
        if !identity_matches(&self.binding, frame.identity) {
            return Err(WireError::InvalidSession);
        }
        if terminal_for(self, direction).is_some() {
            return Err(WireError::LateFrame);
        }
        match frame.message {
            SessionMessage::Hello(hello) => self.admit_hello(direction, hello),
            SessionMessage::Ready => self.admit_ready(direction),
            SessionMessage::Offered { sequence, payload } => {
                self.require_active()?;
                if self.transfer == Some(TransferState::Offered(sequence)) {
                    return Err(WireError::DuplicateFrame);
                }
                if !self.source_direction(direction)
                    || self.input_closed
                    || self.transfer.is_some()
                    || sequence != self.next_sequence
                {
                    return Err(WireError::ReorderedFrame);
                }
                if payload.len()
                    > usize::try_from(self.binding.limits.maximum_payload_bytes)
                        .map_err(|_| WireError::InvalidLimits)?
                {
                    return Err(WireError::OversizedPayload);
                }
                self.transfer = Some(TransferState::Offered(sequence));
                Ok(())
            }
            SessionMessage::Pressure { sequence } => {
                self.require_active()?;
                if !self.sink_direction(direction)
                    || self.transfer != Some(TransferState::Offered(sequence))
                {
                    return Err(WireError::ReorderedFrame);
                }
                self.transfer = None;
                Ok(())
            }
            SessionMessage::Accepted { sequence } => {
                self.require_active()?;
                if self.transfer == Some(TransferState::Accepted(sequence)) {
                    return Err(WireError::DuplicateFrame);
                }
                if !self.sink_direction(direction)
                    || self.transfer != Some(TransferState::Offered(sequence))
                {
                    return Err(WireError::ReorderedFrame);
                }
                self.transfer = Some(TransferState::Accepted(sequence));
                Ok(())
            }
            SessionMessage::Delivered { sequence } => {
                self.require_active()?;
                if self.transfer.is_none() && sequence.checked_add(1) == Some(self.next_sequence) {
                    return Err(WireError::DuplicateFrame);
                }
                if !self.sink_direction(direction)
                    || self.transfer != Some(TransferState::Accepted(sequence))
                {
                    return Err(WireError::ReorderedFrame);
                }
                self.next_sequence = sequence.checked_add(1).ok_or(WireError::ReorderedFrame)?;
                self.transfer = None;
                Ok(())
            }
            SessionMessage::InputClosed { final_sequence } => {
                self.require_active()?;
                if self.input_closed && final_sequence == self.next_sequence {
                    return Err(WireError::DuplicateFrame);
                }
                if !self.source_direction(direction)
                    || self.input_closed
                    || self.transfer.is_some()
                    || final_sequence != self.next_sequence
                {
                    return Err(WireError::ReorderedFrame);
                }
                self.input_closed = true;
                Ok(())
            }
            SessionMessage::Cancelled { code } => {
                self.admit_failure(direction, SessionFailureState::Cancelled(code), code)
            }
            SessionMessage::Failed { code } => {
                self.admit_failure(direction, SessionFailureState::Failed(code), code)
            }
            SessionMessage::Terminal {
                disposition,
                final_sequence,
            } => self.admit_terminal(direction, disposition, final_sequence),
        }
    }

    fn admit_hello(
        &mut self,
        direction: FrameDirection,
        hello: SessionHello<'_>,
    ) -> Result<(), WireError> {
        if self.local_hello || self.peer_hello {
            let already_seen = match direction {
                FrameDirection::Outbound => self.local_hello,
                FrameDirection::Inbound => self.peer_hello,
            };
            if already_seen {
                return Err(WireError::DuplicateFrame);
            }
        }
        if !hello_matches(&self.binding, hello) {
            return Err(WireError::InvalidSession);
        }
        match direction {
            FrameDirection::Outbound => self.local_hello = true,
            FrameDirection::Inbound => self.peer_hello = true,
        }
        Ok(())
    }

    fn admit_ready(&mut self, direction: FrameDirection) -> Result<(), WireError> {
        if !self.local_hello || !self.peer_hello {
            return Err(WireError::InvalidState);
        }
        let ready = match direction {
            FrameDirection::Outbound => &mut self.local_ready,
            FrameDirection::Inbound => &mut self.peer_ready,
        };
        if *ready {
            return Err(WireError::DuplicateFrame);
        }
        *ready = true;
        Ok(())
    }

    fn admit_failure(
        &mut self,
        direction: FrameDirection,
        failure: SessionFailureState,
        code: u16,
    ) -> Result<(), WireError> {
        if code == 0 {
            return Err(WireError::InvalidState);
        }
        let (current, counterpart) = match direction {
            FrameDirection::Outbound => (&mut self.local_failure, self.peer_failure),
            FrameDirection::Inbound => (&mut self.peer_failure, self.local_failure),
        };
        if current.is_some() {
            return Err(WireError::DuplicateFrame);
        }
        if counterpart.is_some_and(|counterpart| counterpart != failure) {
            return Err(WireError::InvalidState);
        }
        *current = Some(failure);
        self.transfer = None;
        Ok(())
    }

    fn admit_terminal(
        &mut self,
        direction: FrameDirection,
        disposition: SessionTerminalDisposition,
        final_sequence: u64,
    ) -> Result<(), WireError> {
        if final_sequence != self.next_sequence || self.transfer.is_some() {
            return Err(WireError::ReorderedFrame);
        }
        let failure = self.local_failure.or(self.peer_failure);
        let valid = match (disposition, failure) {
            (SessionTerminalDisposition::Completed, None) => self.input_closed,
            (SessionTerminalDisposition::Cancelled, Some(SessionFailureState::Cancelled(_))) => {
                true
            }
            (SessionTerminalDisposition::Failed, Some(SessionFailureState::Failed(_))) => true,
            _ => false,
        };
        if !valid {
            return Err(WireError::InvalidState);
        }
        let peer = terminal_for(self, direction.opposite());
        if peer.is_some_and(|peer| peer != disposition) {
            return Err(WireError::InvalidState);
        }
        *terminal_for_mut(self, direction) = Some(disposition);
        Ok(())
    }

    fn require_active(&self) -> Result<(), WireError> {
        if self.is_active() {
            Ok(())
        } else {
            Err(WireError::InvalidState)
        }
    }

    fn source_direction(&self, direction: FrameDirection) -> bool {
        matches!(
            (self.role, direction),
            (SessionRole::Source, FrameDirection::Outbound)
                | (SessionRole::Sink, FrameDirection::Inbound)
        )
    }

    fn sink_direction(&self, direction: FrameDirection) -> bool {
        !self.source_direction(direction)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FrameDirection {
    Outbound,
    Inbound,
}

impl FrameDirection {
    fn opposite(self) -> Self {
        match self {
            Self::Outbound => Self::Inbound,
            Self::Inbound => Self::Outbound,
        }
    }
}

pub fn encode_session_frame_into(
    frame: SessionFrame<'_>,
    output: &mut [u8],
    maximum_payload_bytes: u32,
    maximum_frame_bytes: u32,
) -> Result<usize, WireError> {
    if frame.identity.protocol_version != PROTOCOL_VERSION {
        return Err(WireError::WrongProtocolVersion);
    }
    let maximum_frame =
        usize::try_from(maximum_frame_bytes).map_err(|_| WireError::InvalidLimits)?;
    let mut writer = Writer::new(output, maximum_frame)?;
    writer.bytes(&SESSION_MAGIC)?;
    writer.u8(SESSION_WIRE_VERSION)?;
    writer.u8(message_kind(frame.message))?;
    writer.u16(frame.identity.protocol_version)?;
    for identity in [
        frame.identity.plan_id,
        frame.identity.source_fragment_id,
        frame.identity.sink_fragment_id,
        frame.identity.source_active_play_id,
        frame.identity.sink_active_play_id,
        frame.identity.connection_id,
        frame.identity.link_binding_id,
        frame.identity.provider_instance_id,
    ] {
        writer.text(identity)?;
    }
    match frame.message {
        SessionMessage::Hello(hello) => {
            writer.u8(provider_code(hello.provider)?)?;
            write_endpoint(&mut writer, hello.source)?;
            write_endpoint(&mut writer, hello.sink)?;
            writer.text(hello.value_kind)?;
            writer.u16(hello.limits.maximum_in_flight_items)?;
            writer.u32(hello.limits.maximum_payload_bytes)?;
            writer.u32(hello.limits.maximum_buffered_bytes)?;
            writer.u32(hello.limits.maximum_frame_bytes)?;
        }
        SessionMessage::Ready => {}
        SessionMessage::Offered { sequence, payload } => {
            if payload.len()
                > usize::try_from(maximum_payload_bytes).map_err(|_| WireError::OversizedPayload)?
            {
                return Err(WireError::OversizedPayload);
            }
            writer.u64(sequence)?;
            writer.byte_field(payload)?;
        }
        SessionMessage::Pressure { sequence } => writer.u64(sequence)?,
        SessionMessage::Accepted { sequence } | SessionMessage::Delivered { sequence } => {
            writer.u64(sequence)?;
        }
        SessionMessage::InputClosed { final_sequence } => writer.u64(final_sequence)?,
        SessionMessage::Cancelled { code } | SessionMessage::Failed { code } => writer.u16(code)?,
        SessionMessage::Terminal {
            disposition,
            final_sequence,
        } => {
            writer.u8(terminal_code(disposition))?;
            writer.u64(final_sequence)?;
        }
    }
    Ok(writer.len())
}

pub fn decode_session_frame(
    frame: &[u8],
    maximum_payload_bytes: u32,
    maximum_frame_bytes: u32,
) -> Result<SessionFrame<'_>, WireError> {
    if frame.len() > usize::try_from(maximum_frame_bytes).map_err(|_| WireError::InvalidLimits)? {
        return Err(WireError::OversizedFrame);
    }
    let mut cursor = Cursor::new(frame);
    if cursor.take(4)? != SESSION_MAGIC {
        return Err(WireError::InvalidMagic);
    }
    if cursor.u8()? != SESSION_WIRE_VERSION {
        return Err(WireError::UnsupportedWireFormat);
    }
    let kind = cursor.u8()?;
    let protocol_version = cursor.u16()?;
    if protocol_version != PROTOCOL_VERSION {
        return Err(WireError::WrongProtocolVersion);
    }
    let identity = SessionIdentity {
        protocol_version,
        plan_id: cursor.text()?,
        source_fragment_id: cursor.text()?,
        sink_fragment_id: cursor.text()?,
        source_active_play_id: cursor.text()?,
        sink_active_play_id: cursor.text()?,
        connection_id: cursor.text()?,
        link_binding_id: cursor.text()?,
        provider_instance_id: cursor.text()?,
    };
    let message = match kind {
        1 => SessionMessage::Hello(SessionHello {
            provider: decode_provider(cursor.u8()?)?,
            source: read_endpoint(&mut cursor)?,
            sink: read_endpoint(&mut cursor)?,
            value_kind: cursor.text()?,
            limits: LinkLimits {
                maximum_in_flight_items: cursor.u16()?,
                maximum_payload_bytes: cursor.u32()?,
                maximum_buffered_bytes: cursor.u32()?,
                maximum_frame_bytes: cursor.u32()?,
            },
        }),
        2 => SessionMessage::Ready,
        3 => {
            let sequence = cursor.u64()?;
            let payload = cursor.byte_field()?;
            if payload.len()
                > usize::try_from(maximum_payload_bytes).map_err(|_| WireError::OversizedPayload)?
            {
                return Err(WireError::OversizedPayload);
            }
            SessionMessage::Offered { sequence, payload }
        }
        10 => SessionMessage::Pressure {
            sequence: cursor.u64()?,
        },
        4 => SessionMessage::Accepted {
            sequence: cursor.u64()?,
        },
        5 => SessionMessage::Delivered {
            sequence: cursor.u64()?,
        },
        6 => SessionMessage::InputClosed {
            final_sequence: cursor.u64()?,
        },
        7 => SessionMessage::Cancelled {
            code: cursor.u16()?,
        },
        8 => SessionMessage::Failed {
            code: cursor.u16()?,
        },
        9 => SessionMessage::Terminal {
            disposition: decode_terminal(cursor.u8()?)?,
            final_sequence: cursor.u64()?,
        },
        _ => return Err(WireError::InvalidMessageKind),
    };
    if !cursor.is_empty() {
        return Err(WireError::TrailingGarbage);
    }
    Ok(SessionFrame { identity, message })
}

fn endpoint_ref(endpoint: &LinkEndpoint) -> SessionEndpoint<'_> {
    SessionEndpoint {
        host_id: endpoint.host_id.as_str(),
        boot_id: endpoint.boot_id.as_str(),
        endpoint_id: endpoint.endpoint_id.as_str(),
    }
}

fn identity_matches(binding: &SessionBinding, identity: SessionIdentity<'_>) -> bool {
    binding.identity() == identity
}

fn hello_matches(binding: &SessionBinding, hello: SessionHello<'_>) -> bool {
    hello.provider == binding.provider
        && hello.source == endpoint_ref(&binding.source)
        && hello.sink == endpoint_ref(&binding.sink)
        && hello.value_kind == binding.value_kind.as_str()
        && hello.limits == binding.limits
}

fn terminal_for(
    machine: &SessionMachine,
    direction: FrameDirection,
) -> Option<SessionTerminalDisposition> {
    match direction {
        FrameDirection::Outbound => machine.local_terminal,
        FrameDirection::Inbound => machine.peer_terminal,
    }
}

fn terminal_for_mut(
    machine: &mut SessionMachine,
    direction: FrameDirection,
) -> &mut Option<SessionTerminalDisposition> {
    match direction {
        FrameDirection::Outbound => &mut machine.local_terminal,
        FrameDirection::Inbound => &mut machine.peer_terminal,
    }
}

fn message_kind(message: SessionMessage<'_>) -> u8 {
    match message {
        SessionMessage::Hello(_) => 1,
        SessionMessage::Ready => 2,
        SessionMessage::Offered { .. } => 3,
        SessionMessage::Pressure { .. } => 10,
        SessionMessage::Accepted { .. } => 4,
        SessionMessage::Delivered { .. } => 5,
        SessionMessage::InputClosed { .. } => 6,
        SessionMessage::Cancelled { .. } => 7,
        SessionMessage::Failed { .. } => 8,
        SessionMessage::Terminal { .. } => 9,
    }
}

fn provider_code(provider: ConnectionProvider) -> Result<u8, WireError> {
    if !provider.supports_remote_session() {
        return Err(WireError::InvalidProvider);
    }
    Ok(provider.canonical_code())
}

fn decode_provider(code: u8) -> Result<ConnectionProvider, WireError> {
    let provider =
        ConnectionProvider::from_canonical_code(code).ok_or(WireError::InvalidProvider)?;
    if !provider.supports_remote_session() {
        return Err(WireError::InvalidProvider);
    }
    Ok(provider)
}

fn terminal_code(disposition: SessionTerminalDisposition) -> u8 {
    match disposition {
        SessionTerminalDisposition::Completed => 0,
        SessionTerminalDisposition::Cancelled => 1,
        SessionTerminalDisposition::Failed => 2,
    }
}

fn decode_terminal(code: u8) -> Result<SessionTerminalDisposition, WireError> {
    match code {
        0 => Ok(SessionTerminalDisposition::Completed),
        1 => Ok(SessionTerminalDisposition::Cancelled),
        2 => Ok(SessionTerminalDisposition::Failed),
        _ => Err(WireError::InvalidState),
    }
}

fn write_endpoint(writer: &mut Writer<'_>, endpoint: SessionEndpoint<'_>) -> Result<(), WireError> {
    writer.text(endpoint.host_id)?;
    writer.text(endpoint.boot_id)?;
    writer.text(endpoint.endpoint_id)
}

fn read_endpoint<'a>(cursor: &mut Cursor<'a>) -> Result<SessionEndpoint<'a>, WireError> {
    Ok(SessionEndpoint {
        host_id: cursor.text()?,
        boot_id: cursor.text()?,
        endpoint_id: cursor.text()?,
    })
}

fn hello_encoded_len(binding: &SessionBinding) -> Result<usize, WireError> {
    common_encoded_len(binding)?
        .checked_add(1 + 2 * 6 + 2 + 2 + 4 * 3)
        .and_then(|value| {
            value.checked_add(
                binding.source.host_id.as_str().len()
                    + binding.source.boot_id.as_str().len()
                    + binding.source.endpoint_id.as_str().len()
                    + binding.sink.host_id.as_str().len()
                    + binding.sink.boot_id.as_str().len()
                    + binding.sink.endpoint_id.as_str().len()
                    + binding.value_kind.as_str().len(),
            )
        })
        .ok_or(WireError::InvalidLimits)
}

fn offered_encoded_len(binding: &SessionBinding, payload: usize) -> Result<usize, WireError> {
    common_encoded_len(binding)?
        .checked_add(8 + 4)
        .and_then(|value| value.checked_add(payload))
        .ok_or(WireError::InvalidLimits)
}

fn common_encoded_len(binding: &SessionBinding) -> Result<usize, WireError> {
    COMMON_FIXED_BYTES
        .checked_add(binding.plan_id.as_str().len())
        .and_then(|value| value.checked_add(binding.source_fragment_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.sink_fragment_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.source_active_play_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.sink_active_play_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.connection_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.link_binding_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.provider_instance_id.as_str().len()))
        .ok_or(WireError::InvalidLimits)
}

struct Writer<'a> {
    output: &'a mut [u8],
    limit: usize,
    offset: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8], maximum: usize) -> Result<Self, WireError> {
        if maximum == 0 || output.len() < maximum {
            return Err(WireError::OutputTooSmall);
        }
        Ok(Self {
            output,
            limit: maximum,
            offset: 0,
        })
    }

    fn len(&self) -> usize {
        self.offset
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), WireError> {
        let end = self
            .offset
            .checked_add(value.len())
            .filter(|end| *end <= self.limit)
            .ok_or(WireError::OversizedFrame)?;
        self.output[self.offset..end].copy_from_slice(value);
        self.offset = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), WireError> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), WireError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), WireError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), WireError> {
        self.bytes(&value.to_le_bytes())
    }

    fn text(&mut self, value: &str) -> Result<(), WireError> {
        if value.len() > MAX_ID_BYTES || value.len() > usize::from(u16::MAX) {
            return Err(WireError::IdentifierTooLong);
        }
        self.u16(value.len() as u16)?;
        self.bytes(value.as_bytes())
    }

    fn byte_field(&mut self, value: &[u8]) -> Result<(), WireError> {
        let length = u32::try_from(value.len()).map_err(|_| WireError::OversizedPayload)?;
        self.u32(length)?;
        self.bytes(value)
    }
}

struct Cursor<'a> {
    remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn new(frame: &'a [u8]) -> Self {
        Self { remaining: frame }
    }

    fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WireError> {
        if self.remaining.len() < length {
            return Err(WireError::TruncatedFrame);
        }
        let (value, rest) = self.remaining.split_at(length);
        self.remaining = rest;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, WireError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> Result<u64, WireError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn text(&mut self) -> Result<&'a str, WireError> {
        let length = usize::from(self.u16()?);
        if length > MAX_ID_BYTES {
            return Err(WireError::IdentifierTooLong);
        }
        core::str::from_utf8(self.take(length)?).map_err(|_| WireError::InvalidIdentifierEncoding)
    }

    fn byte_field(&mut self) -> Result<&'a [u8], WireError> {
        let length = usize::try_from(self.u32()?).map_err(|_| WireError::OversizedPayload)?;
        self.take(length)
    }
}

#[cfg(test)]
mod tests;
