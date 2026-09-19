use alloc::{collections::VecDeque, string::String, vec::Vec};

use crate::{decode_relay_envelope, RelayEnvelopeError, MAXIMUM_RELAY_ROUTE_BYTES};

pub const RELAY_SERVICE_IMPLEMENTATION_ID: &str = "conduit.relay/opaque-two-endpoint@1";
pub const MAXIMUM_RELAY_ENDPOINT_BINDING_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayServiceLimits {
    pub maximum_slots: u16,
    pub maximum_protected_frame_bytes: u32,
    pub maximum_queued_frames_per_direction: u16,
    pub maximum_queued_bytes_per_direction: u32,
    pub maximum_attachment_attempts_per_slot: u8,
    pub maximum_idle_millis: u32,
    pub maximum_active_millis: u32,
}

impl RelayServiceLimits {
    pub fn validate(self) -> Result<Self, RelayServiceError> {
        if self.maximum_slots == 0
            || self.maximum_slots > 1_024
            || self.maximum_protected_frame_bytes == 0
            || self.maximum_protected_frame_bytes > 65_553
            || self.maximum_queued_frames_per_direction == 0
            || self.maximum_queued_frames_per_direction > 64
            || self.maximum_queued_bytes_per_direction < self.maximum_protected_frame_bytes
            || self.maximum_queued_bytes_per_direction > 4 * 1024 * 1024
            || self.maximum_attachment_attempts_per_slot == 0
            || self.maximum_attachment_attempts_per_slot > 16
            || self.maximum_idle_millis == 0
            || self.maximum_active_millis < self.maximum_idle_millis
            || self.maximum_active_millis > 86_400_000
        {
            return Err(RelayServiceError::InvalidLimits);
        }
        Ok(self)
    }
}

pub struct RelayCapability([u8; 32]);

impl RelayCapability {
    pub fn new(bytes: [u8; 32]) -> Result<Self, RelayServiceError> {
        if bytes == [0; 32] {
            return Err(RelayServiceError::WeakCapability);
        }
        Ok(Self(bytes))
    }

    fn authenticates(&self, offered: &mut [u8]) -> bool {
        let mut difference = offered.len() ^ self.0.len();
        for (index, expected) in self.0.iter().copied().enumerate() {
            difference |= usize::from(offered.get(index).copied().unwrap_or(0) ^ expected);
        }
        offered.fill(0);
        difference == 0
    }
}

impl core::fmt::Debug for RelayCapability {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("RelayCapability([REDACTED])")
    }
}

impl Drop for RelayCapability {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Debug)]
pub struct RelaySlotDescriptor {
    pub route_id: String,
    pub negotiation_id: String,
    pub first_endpoint_binding: String,
    pub second_endpoint_binding: String,
    pub expires_at_millis: u64,
    capabilities: [RelayCapability; 2],
}

impl RelaySlotDescriptor {
    pub fn new(
        route_id: String,
        negotiation_id: String,
        first_endpoint_binding: String,
        second_endpoint_binding: String,
        expires_at_millis: u64,
        first_capability: [u8; 32],
        second_capability: [u8; 32],
    ) -> Result<Self, RelayServiceError> {
        for value in [
            route_id.as_str(),
            negotiation_id.as_str(),
            first_endpoint_binding.as_str(),
            second_endpoint_binding.as_str(),
        ] {
            if value.is_empty() || value.len() > MAXIMUM_RELAY_ENDPOINT_BINDING_BYTES {
                return Err(RelayServiceError::InvalidIdentity);
            }
        }
        if route_id.len() > MAXIMUM_RELAY_ROUTE_BYTES || expires_at_millis == 0 {
            return Err(RelayServiceError::InvalidIdentity);
        }
        Ok(Self {
            route_id,
            negotiation_id,
            first_endpoint_binding,
            second_endpoint_binding,
            expires_at_millis,
            capabilities: [
                RelayCapability::new(first_capability)?,
                RelayCapability::new(second_capability)?,
            ],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayEndpointRole {
    First,
    Second,
}

impl RelayEndpointRole {
    fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }

    fn peer(self) -> usize {
        self.index() ^ 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayAttachmentDisposition {
    WaitingForPeer,
    Paired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayServiceError {
    InvalidLimits,
    WeakCapability,
    InvalidIdentity,
    SlotCapacity,
    DuplicateRoute,
    UnknownRoute,
    ExpiredCapability,
    AuthenticationFailed,
    AttachmentAttemptsExhausted,
    WrongEndpointBinding,
    EndpointAlreadyAttached,
    ConnectionMismatch,
    PeerWaiting,
    QueuePressure,
    RelayEnvelope(RelayEnvelopeError),
    ExplicitlyClosed,
    RelayConnectionLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelaySlotDisposition {
    Waiting,
    Paired,
    Closed,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaySlotEvidence {
    pub implementation_id: &'static str,
    pub route_id: String,
    pub negotiation_id: String,
    pub attached_endpoints: u8,
    pub forwarded_frames: u64,
    pub forwarded_bytes: u64,
    pub pressure_refusals: u64,
    pub disposition: RelaySlotDisposition,
}

struct RelayEndpoint {
    binding: String,
    connection_id: Option<u64>,
    queue: VecDeque<Vec<u8>>,
    queued_bytes: usize,
}

struct RelaySlot {
    descriptor: RelaySlotDescriptor,
    created_at_millis: u64,
    endpoints: [RelayEndpoint; 2],
    attachment_attempts: u8,
    paired_at_millis: Option<u64>,
    forwarded_frames: u64,
    forwarded_bytes: u64,
    pressure_refusals: u64,
    disposition: RelaySlotDisposition,
}

pub struct OpaqueRelayService {
    limits: RelayServiceLimits,
    slots: Vec<RelaySlot>,
}

impl OpaqueRelayService {
    pub fn new(limits: RelayServiceLimits) -> Result<Self, RelayServiceError> {
        let limits = limits.validate()?;
        Ok(Self {
            limits,
            slots: Vec::with_capacity(usize::from(limits.maximum_slots)),
        })
    }

    pub fn install_slot(
        &mut self,
        descriptor: RelaySlotDescriptor,
        now_millis: u64,
    ) -> Result<(), RelayServiceError> {
        if descriptor.expires_at_millis <= now_millis {
            return Err(RelayServiceError::ExpiredCapability);
        }
        if self.slots.len() == usize::from(self.limits.maximum_slots) {
            return Err(RelayServiceError::SlotCapacity);
        }
        if self
            .slots
            .iter()
            .any(|slot| slot.descriptor.route_id == descriptor.route_id)
        {
            return Err(RelayServiceError::DuplicateRoute);
        }
        let endpoints = [
            RelayEndpoint {
                binding: descriptor.first_endpoint_binding.clone(),
                connection_id: None,
                queue: VecDeque::with_capacity(usize::from(
                    self.limits.maximum_queued_frames_per_direction,
                )),
                queued_bytes: 0,
            },
            RelayEndpoint {
                binding: descriptor.second_endpoint_binding.clone(),
                connection_id: None,
                queue: VecDeque::with_capacity(usize::from(
                    self.limits.maximum_queued_frames_per_direction,
                )),
                queued_bytes: 0,
            },
        ];
        self.slots.push(RelaySlot {
            descriptor,
            created_at_millis: now_millis,
            endpoints,
            attachment_attempts: 0,
            paired_at_millis: None,
            forwarded_frames: 0,
            forwarded_bytes: 0,
            pressure_refusals: 0,
            disposition: RelaySlotDisposition::Waiting,
        });
        Ok(())
    }

    pub fn attach(
        &mut self,
        route_id: &str,
        role: RelayEndpointRole,
        endpoint_binding: &str,
        mut capability: Vec<u8>,
        connection_id: u64,
        now_millis: u64,
    ) -> Result<RelayAttachmentDisposition, RelayServiceError> {
        let limits = self.limits;
        let slot = self.slot_mut(route_id)?;
        Self::ensure_live(slot, limits, now_millis)?;
        if slot.attachment_attempts == limits.maximum_attachment_attempts_per_slot {
            capability.fill(0);
            return Err(RelayServiceError::AttachmentAttemptsExhausted);
        }
        slot.attachment_attempts += 1;
        if !slot.descriptor.capabilities[role.index()].authenticates(&mut capability) {
            return Err(RelayServiceError::AuthenticationFailed);
        }
        let endpoint = &mut slot.endpoints[role.index()];
        if endpoint.binding != endpoint_binding {
            return Err(RelayServiceError::WrongEndpointBinding);
        }
        if endpoint.connection_id.is_some() {
            return Err(RelayServiceError::EndpointAlreadyAttached);
        }
        endpoint.connection_id = Some(connection_id);
        if slot.endpoints[role.peer()].connection_id.is_some() {
            slot.paired_at_millis = Some(now_millis);
            slot.disposition = RelaySlotDisposition::Paired;
            Ok(RelayAttachmentDisposition::Paired)
        } else {
            Ok(RelayAttachmentDisposition::WaitingForPeer)
        }
    }

    pub fn forward(
        &mut self,
        route_id: &str,
        role: RelayEndpointRole,
        connection_id: u64,
        envelope: &[u8],
        now_millis: u64,
    ) -> Result<(), RelayServiceError> {
        let limits = self.limits;
        let slot = self.slot_mut(route_id)?;
        Self::ensure_live(slot, limits, now_millis)?;
        if slot.endpoints[role.index()].connection_id != Some(connection_id) {
            return Err(RelayServiceError::ConnectionMismatch);
        }
        if slot.endpoints[role.peer()].connection_id.is_none() {
            return Err(RelayServiceError::PeerWaiting);
        }
        let decoded =
            decode_relay_envelope(envelope, limits.maximum_protected_frame_bytes as usize)
                .map_err(RelayServiceError::RelayEnvelope)?;
        if decoded.route_id != route_id {
            return Err(RelayServiceError::UnknownRoute);
        }
        let peer = &mut slot.endpoints[role.peer()];
        let next_bytes = peer
            .queued_bytes
            .checked_add(envelope.len())
            .ok_or(RelayServiceError::QueuePressure)?;
        if peer.queue.len() == usize::from(limits.maximum_queued_frames_per_direction)
            || next_bytes > limits.maximum_queued_bytes_per_direction as usize
        {
            slot.pressure_refusals += 1;
            return Err(RelayServiceError::QueuePressure);
        }
        peer.queue.push_back(envelope.to_vec());
        peer.queued_bytes = next_bytes;
        slot.forwarded_frames += 1;
        slot.forwarded_bytes += decoded.protected_frame.len() as u64;
        Ok(())
    }

    pub fn attachment_status(
        &mut self,
        route_id: &str,
        role: RelayEndpointRole,
        connection_id: u64,
        now_millis: u64,
    ) -> Result<RelayAttachmentDisposition, RelayServiceError> {
        let limits = self.limits;
        let slot = self.slot_mut(route_id)?;
        Self::ensure_live(slot, limits, now_millis)?;
        if slot.endpoints[role.index()].connection_id != Some(connection_id) {
            return Err(RelayServiceError::ConnectionMismatch);
        }
        if slot.endpoints[role.peer()].connection_id.is_some() {
            Ok(RelayAttachmentDisposition::Paired)
        } else {
            Ok(RelayAttachmentDisposition::WaitingForPeer)
        }
    }

    pub fn receive(
        &mut self,
        route_id: &str,
        role: RelayEndpointRole,
        connection_id: u64,
        now_millis: u64,
    ) -> Result<Option<Vec<u8>>, RelayServiceError> {
        let limits = self.limits;
        let slot = self.slot_mut(route_id)?;
        Self::ensure_live(slot, limits, now_millis)?;
        let endpoint = &mut slot.endpoints[role.index()];
        if endpoint.connection_id != Some(connection_id) {
            return Err(RelayServiceError::ConnectionMismatch);
        }
        let Some(frame) = endpoint.queue.pop_front() else {
            return Ok(None);
        };
        endpoint.queued_bytes -= frame.len();
        Ok(Some(frame))
    }

    pub fn close(
        &mut self,
        route_id: &str,
        disposition: RelaySlotDisposition,
    ) -> Result<RelaySlotEvidence, RelayServiceError> {
        if !matches!(
            disposition,
            RelaySlotDisposition::Closed | RelaySlotDisposition::Lost
        ) {
            return Err(RelayServiceError::InvalidIdentity);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.descriptor.route_id == route_id)
            .ok_or(RelayServiceError::UnknownRoute)?;
        let mut slot = self.slots.swap_remove(index);
        slot.disposition = disposition;
        Ok(slot.evidence())
    }

    fn slot_mut(&mut self, route_id: &str) -> Result<&mut RelaySlot, RelayServiceError> {
        self.slots
            .iter_mut()
            .find(|slot| slot.descriptor.route_id == route_id)
            .ok_or(RelayServiceError::UnknownRoute)
    }

    fn ensure_live(
        slot: &RelaySlot,
        limits: RelayServiceLimits,
        now_millis: u64,
    ) -> Result<(), RelayServiceError> {
        if now_millis >= slot.descriptor.expires_at_millis {
            return Err(RelayServiceError::ExpiredCapability);
        }
        if slot.paired_at_millis.is_none()
            && now_millis.saturating_sub(slot.created_at_millis)
                > u64::from(limits.maximum_idle_millis)
        {
            return Err(RelayServiceError::ExpiredCapability);
        }
        if let Some(paired_at) = slot.paired_at_millis {
            if now_millis.saturating_sub(paired_at) > u64::from(limits.maximum_active_millis) {
                return Err(RelayServiceError::RelayConnectionLost);
            }
        }
        Ok(())
    }
}

impl RelaySlot {
    fn evidence(&self) -> RelaySlotEvidence {
        RelaySlotEvidence {
            implementation_id: RELAY_SERVICE_IMPLEMENTATION_ID,
            route_id: self.descriptor.route_id.clone(),
            negotiation_id: self.descriptor.negotiation_id.clone(),
            attached_endpoints: self
                .endpoints
                .iter()
                .filter(|endpoint| endpoint.connection_id.is_some())
                .count() as u8,
            forwarded_frames: self.forwarded_frames,
            forwarded_bytes: self.forwarded_bytes,
            pressure_refusals: self.pressure_refusals,
            disposition: self.disposition,
        }
    }
}
