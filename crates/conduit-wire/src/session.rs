use conduit_core::{
    bind_active_play, ActivePlayId, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId,
    FragmentId, HostId, KindId, LinkAvailability, LinkBinding, LinkBindingId, LinkEndpointId,
    LinkLimits, PlanId, PlannedConnection, PROTOCOL_VERSION,
};

use crate::{WireError, MAX_ID_BYTES};

const SESSION_MAGIC: [u8; 4] = *b"CNDS";
const SESSION_WIRE_VERSION: u8 = 2;
const COMMON_FIXED_BYTES: usize = 4 + 1 + 1 + 2 + 2 * 11 + 2 + 4 + 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBinding {
    pub protocol_version: u16,
    pub plan_id: PlanId,
    pub source_fragment_id: FragmentId,
    pub sink_fragment_id: FragmentId,
    pub source_active_play_id: ActivePlayId,
    pub sink_active_play_id: ActivePlayId,
    pub connection_id: ConnectionId,
    pub source: SessionEndpointIdentity,
    pub sink: SessionEndpointIdentity,
    pub value_kind: KindId,
    pub limits: SessionLimits,
    pub attachment: RouteAttachment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEndpointIdentity {
    pub host_id: HostId,
    pub boot_id: BootId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionLimits {
    pub maximum_in_flight_items: u16,
    pub maximum_payload_bytes: u32,
    pub maximum_buffered_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteAttachment {
    pub link_binding_id: LinkBindingId,
    pub base: ConnectionBase,
    pub base_instance_id: ConnectionBaseInstanceId,
    pub source_host_id: HostId,
    pub source_boot_id: BootId,
    pub source_endpoint_id: LinkEndpointId,
    pub sink_host_id: HostId,
    pub sink_boot_id: BootId,
    pub sink_endpoint_id: LinkEndpointId,
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
        Self::from_planned_connection_with_link(
            plan_id,
            source_fragment_id,
            sink_fragment_id,
            connection,
            link,
        )
    }

    /// Bind the same logical session to one exact currently-ready sealed route.
    pub fn from_planned_connection_with_link(
        plan_id: PlanId,
        source_fragment_id: FragmentId,
        sink_fragment_id: FragmentId,
        connection: &PlannedConnection,
        link: &LinkBinding,
    ) -> Result<Self, WireError> {
        if !link.base.supports_remote_session()
            || link.availability != LinkAvailability::Ready
            || (connection.route_candidates.is_empty() && link.base != connection.base)
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
            source: SessionEndpointIdentity {
                host_id: link.source.host_id.clone(),
                boot_id: link.source.boot_id.clone(),
            },
            sink: SessionEndpointIdentity {
                host_id: link.sink.host_id.clone(),
                boot_id: link.sink.boot_id.clone(),
            },
            value_kind: connection.value_kind.clone(),
            limits: SessionLimits {
                maximum_in_flight_items: connection.item_capacity,
                maximum_payload_bytes: connection.byte_capacity,
                maximum_buffered_bytes: connection.byte_capacity,
            },
            attachment: RouteAttachment {
                link_binding_id: link.binding_id.clone(),
                base: link.base,
                base_instance_id: link.base_instance_id.clone(),
                source_host_id: link.source.host_id.clone(),
                source_boot_id: link.source.boot_id.clone(),
                source_endpoint_id: link.source.endpoint_id.clone(),
                sink_host_id: link.sink.host_id.clone(),
                sink_boot_id: link.sink.boot_id.clone(),
                sink_endpoint_id: link.sink.endpoint_id.clone(),
                limits: link.limits,
            },
        };
        binding.validate()?;
        Ok(binding)
    }

    /// Materialize the boot-scoped session identity from an exact planned
    /// connection plus the two observed runtime boot facts. The matching route
    /// attachment boot facts change atomically; every other planner-owned
    /// identity remains unchanged.
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
        self.attachment.source_boot_id = self.source.boot_id.clone();
        self.attachment.sink_boot_id = self.sink.boot_id.clone();
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
        if !self.attachment.base.supports_remote_session() {
            return Err(WireError::InvalidBase);
        }
        let identities = [
            self.plan_id.as_str(),
            self.source_fragment_id.as_str(),
            self.sink_fragment_id.as_str(),
            self.source_active_play_id.as_str(),
            self.sink_active_play_id.as_str(),
            self.connection_id.as_str(),
            self.source.host_id.as_str(),
            self.source.boot_id.as_str(),
            self.sink.host_id.as_str(),
            self.sink.boot_id.as_str(),
            self.value_kind.as_str(),
            self.attachment.link_binding_id.as_str(),
            self.attachment.base_instance_id.as_str(),
            self.attachment.source_host_id.as_str(),
            self.attachment.source_boot_id.as_str(),
            self.attachment.source_endpoint_id.as_str(),
            self.attachment.sink_host_id.as_str(),
            self.attachment.sink_boot_id.as_str(),
            self.attachment.sink_endpoint_id.as_str(),
        ];
        if identities
            .iter()
            .any(|identity| identity.is_empty() || identity.len() > MAX_ID_BYTES)
            || self.source.host_id == self.sink.host_id
            || self.source.host_id != self.attachment.source_host_id
            || self.source.boot_id != self.attachment.source_boot_id
            || self.sink.host_id != self.attachment.sink_host_id
            || self.sink.boot_id != self.attachment.sink_boot_id
            || self.attachment.source_endpoint_id == self.attachment.sink_endpoint_id
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
            || self.limits.maximum_payload_bytes > self.limits.maximum_buffered_bytes
            || self.limits.maximum_in_flight_items > self.attachment.limits.maximum_in_flight_items
            || self.limits.maximum_payload_bytes > self.attachment.limits.maximum_payload_bytes
            || self.limits.maximum_buffered_bytes > self.attachment.limits.maximum_buffered_bytes
            || self.attachment.limits.maximum_frame_bytes == 0
        {
            return Err(WireError::InvalidLimits);
        }
        let maximum_frame = usize::try_from(self.attachment.limits.maximum_frame_bytes)
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
            source_host_id: self.source.host_id.as_str(),
            source_boot_id: self.source.boot_id.as_str(),
            sink_host_id: self.sink.host_id.as_str(),
            sink_boot_id: self.sink.boot_id.as_str(),
            value_kind: self.value_kind.as_str(),
            limits: self.limits,
        }
    }

    pub fn hello_frame(&self) -> SessionFrame<'_> {
        SessionFrame {
            identity: self.identity(),
            message: SessionMessage::Hello(SessionHello {
                link_binding_id: self.attachment.link_binding_id.as_str(),
                base: self.attachment.base,
                base_instance_id: self.attachment.base_instance_id.as_str(),
                source_endpoint_id: self.attachment.source_endpoint_id.as_str(),
                sink_endpoint_id: self.attachment.sink_endpoint_id.as_str(),
                limits: self.attachment.limits,
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
    pub source_host_id: &'a str,
    pub source_boot_id: &'a str,
    pub sink_host_id: &'a str,
    pub sink_boot_id: &'a str,
    pub value_kind: &'a str,
    pub limits: SessionLimits,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SessionHello<'a> {
    pub link_binding_id: &'a str,
    pub base: ConnectionBase,
    pub base_instance_id: &'a str,
    pub source_endpoint_id: &'a str,
    pub sink_endpoint_id: &'a str,
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

mod machine;
pub use machine::*;
mod reconciliation;
pub use reconciliation::*;
mod checkpoint_wire;
pub use checkpoint_wire::*;

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
        frame.identity.source_host_id,
        frame.identity.source_boot_id,
        frame.identity.sink_host_id,
        frame.identity.sink_boot_id,
        frame.identity.value_kind,
    ] {
        writer.text(identity)?;
    }
    writer.u16(frame.identity.limits.maximum_in_flight_items)?;
    writer.u32(frame.identity.limits.maximum_payload_bytes)?;
    writer.u32(frame.identity.limits.maximum_buffered_bytes)?;
    match frame.message {
        SessionMessage::Hello(hello) => {
            writer.text(hello.link_binding_id)?;
            writer.u8(base_code(hello.base)?)?;
            writer.text(hello.base_instance_id)?;
            writer.text(hello.source_endpoint_id)?;
            writer.text(hello.sink_endpoint_id)?;
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
        source_host_id: cursor.text()?,
        source_boot_id: cursor.text()?,
        sink_host_id: cursor.text()?,
        sink_boot_id: cursor.text()?,
        value_kind: cursor.text()?,
        limits: SessionLimits {
            maximum_in_flight_items: cursor.u16()?,
            maximum_payload_bytes: cursor.u32()?,
            maximum_buffered_bytes: cursor.u32()?,
        },
    };
    let message = match kind {
        1 => SessionMessage::Hello(SessionHello {
            link_binding_id: cursor.text()?,
            base: decode_base(cursor.u8()?)?,
            base_instance_id: cursor.text()?,
            source_endpoint_id: cursor.text()?,
            sink_endpoint_id: cursor.text()?,
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

fn identity_matches(binding: &SessionBinding, identity: SessionIdentity<'_>) -> bool {
    binding.identity() == identity
}

fn hello_matches(binding: &SessionBinding, hello: SessionHello<'_>) -> bool {
    hello.link_binding_id == binding.attachment.link_binding_id.as_str()
        && hello.base == binding.attachment.base
        && hello.base_instance_id == binding.attachment.base_instance_id.as_str()
        && hello.source_endpoint_id == binding.attachment.source_endpoint_id.as_str()
        && hello.sink_endpoint_id == binding.attachment.sink_endpoint_id.as_str()
        && hello.limits == binding.attachment.limits
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

fn base_code(base: ConnectionBase) -> Result<u8, WireError> {
    if !base.supports_remote_session() {
        return Err(WireError::InvalidBase);
    }
    Ok(base.canonical_code())
}

fn decode_base(code: u8) -> Result<ConnectionBase, WireError> {
    let base = ConnectionBase::from_canonical_code(code).ok_or(WireError::InvalidBase)?;
    if !base.supports_remote_session() {
        return Err(WireError::InvalidBase);
    }
    Ok(base)
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

fn hello_encoded_len(binding: &SessionBinding) -> Result<usize, WireError> {
    common_encoded_len(binding)?
        .checked_add(1 + 2 * 4 + 2 + 4 * 3)
        .and_then(|value| {
            value.checked_add(
                binding.attachment.link_binding_id.as_str().len()
                    + binding.attachment.base_instance_id.as_str().len()
                    + binding.attachment.source_endpoint_id.as_str().len()
                    + binding.attachment.sink_endpoint_id.as_str().len(),
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
        .and_then(|value| value.checked_add(binding.source.host_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.source.boot_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.sink.host_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.sink.boot_id.as_str().len()))
        .and_then(|value| value.checked_add(binding.value_kind.as_str().len()))
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
