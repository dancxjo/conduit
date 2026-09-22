//! Bounded authenticated remote emergency admission for the std Host.

use conduit_body::{
    BodyId, BodyMembership, EmergencyControl, EmergencyOutcome, EmergencyPolicy, EmergencyRefusal,
    EmergencyRequest, EmergencyTriggerClass, MembershipCredential, MembershipState, PartId,
    EMERGENCY_CONTROL_POLICY,
};
use conduit_core::{BootId, HostId};
use conduit_protected_line::{
    ProtectedLineError, ProtectedSession, SessionBinding, SessionDisposition,
};

const MAGIC: [u8; 4] = *b"CNDE";
const VERSION: u8 = 1;
const MAX_ID_BYTES: usize = 96;
pub const MAX_REMOTE_EMERGENCY_PAYLOAD_BYTES: usize = 320;
pub const MAX_REMOTE_PROPAGATION_TARGETS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteEmergencyRefusal {
    InvalidAuthority,
    StaleMembership,
    WrongSession,
    SessionClosed,
    Transport(ProtectedLineError),
    MalformedFrame,
    WrongBody,
    WrongCredential,
    StaleOrReplayed,
    Emergency(EmergencyRefusal),
    PropagationCapacity,
    DuplicatePropagationTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemotePropagationStatus {
    Delivered,
    Unreachable,
    Unauthorized,
    Refused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemotePropagationSummary {
    NotRequested,
    Complete,
    PartialFailure,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemotePropagationEvidence {
    pub host_id: HostId,
    pub status: RemotePropagationStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteEmergencyReceipt {
    pub local_outcome: EmergencyOutcome,
    pub authenticated_peer_host_id: HostId,
    pub authenticated_peer_boot_id: BootId,
    pub membership_credential_id: String,
    pub line_session_id: String,
    pub freshness: u64,
    pub propagation: Vec<RemotePropagationEvidence>,
}

impl RemoteEmergencyReceipt {
    pub fn record_propagation(
        &mut self,
        host_id: HostId,
        status: RemotePropagationStatus,
    ) -> Result<(), RemoteEmergencyRefusal> {
        if self.propagation.len() == MAX_REMOTE_PROPAGATION_TARGETS {
            return Err(RemoteEmergencyRefusal::PropagationCapacity);
        }
        if self
            .propagation
            .iter()
            .any(|entry| entry.host_id == host_id)
        {
            return Err(RemoteEmergencyRefusal::DuplicatePropagationTarget);
        }
        self.propagation
            .push(RemotePropagationEvidence { host_id, status });
        Ok(())
    }

    pub fn propagation_summary(&self) -> RemotePropagationSummary {
        let delivered = self
            .propagation
            .iter()
            .filter(|entry| entry.status == RemotePropagationStatus::Delivered)
            .count();
        match (delivered, self.propagation.len()) {
            (_, 0) => RemotePropagationSummary::NotRequested,
            (delivered, total) if delivered == total => RemotePropagationSummary::Complete,
            (0, _) => RemotePropagationSummary::Failed,
            _ => RemotePropagationSummary::PartialFailure,
        }
    }
}

pub struct RemoteEmergencyAdapter {
    body_id: BodyId,
    local_host_id: HostId,
    local_boot_id: BootId,
    peer_part_id: PartId,
    peer_host_id: HostId,
    peer_boot_id: BootId,
    credential_id: String,
    binding: SessionBinding,
    last_freshness: u64,
    control: EmergencyControl,
}

impl RemoteEmergencyAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        body_id: BodyId,
        local_host_id: HostId,
        local_boot_id: BootId,
        membership: &BodyMembership,
        peer_credential: &MembershipCredential,
        session: &ProtectedSession,
        policy: EmergencyPolicy,
    ) -> Result<Self, RemoteEmergencyRefusal> {
        if membership.body_id != body_id || peer_credential.body_id != body_id {
            return Err(RemoteEmergencyRefusal::InvalidAuthority);
        }
        validate_current_membership(membership, peer_credential)?;
        let evidence = session.evidence();
        if evidence.disposition != SessionDisposition::Open {
            return Err(RemoteEmergencyRefusal::SessionClosed);
        }
        validate_binding(
            evidence.binding,
            &local_host_id,
            &local_boot_id,
            &peer_credential.host_id,
            &peer_credential.boot_id,
        )?;
        let credential_id = peer_credential.credential_id.as_str();
        if credential_id.is_empty() || credential_id.len() > MAX_ID_BYTES {
            return Err(RemoteEmergencyRefusal::InvalidAuthority);
        }
        Ok(Self {
            body_id: body_id.clone(),
            local_host_id: local_host_id.clone(),
            local_boot_id: local_boot_id.clone(),
            peer_part_id: peer_credential.part_id.clone(),
            peer_host_id: peer_credential.host_id.clone(),
            peer_boot_id: peer_credential.boot_id.clone(),
            credential_id: credential_id.into(),
            binding: evidence.binding.clone(),
            last_freshness: 0,
            control: EmergencyControl::admit(body_id, local_host_id, local_boot_id, policy),
        })
    }

    pub fn receive_authenticated(
        &mut self,
        membership: &BodyMembership,
        session: &mut ProtectedSession,
        encrypted_frame: &[u8],
    ) -> Result<RemoteEmergencyReceipt, RemoteEmergencyRefusal> {
        self.revalidate(membership, session)?;
        let mut plaintext = [0_u8; MAX_REMOTE_EMERGENCY_PAYLOAD_BYTES];
        let length = session
            .open(encrypted_frame, &mut plaintext)
            .map_err(RemoteEmergencyRefusal::Transport)?;
        let request = decode_request(&plaintext[..length])?;
        if request.body_id != self.body_id.as_str() {
            return Err(RemoteEmergencyRefusal::WrongBody);
        }
        if request.credential_id != self.credential_id {
            return Err(RemoteEmergencyRefusal::WrongCredential);
        }
        if request.freshness == 0 || request.freshness <= self.last_freshness {
            return Err(RemoteEmergencyRefusal::StaleOrReplayed);
        }
        let outcome = self
            .control
            .inspect(&EmergencyRequest {
                request_id: request.request_id.into(),
                body_id: self.body_id.clone(),
                host_id: self.local_host_id.clone(),
                boot_id: self.local_boot_id.clone(),
                trigger: EmergencyTriggerClass::AuthenticatedRemoteEmergency,
                policy_id: EMERGENCY_CONTROL_POLICY.into(),
                freshness: request.freshness,
            })
            .map_err(RemoteEmergencyRefusal::Emergency)?;
        self.last_freshness = request.freshness;
        Ok(RemoteEmergencyReceipt {
            local_outcome: outcome,
            authenticated_peer_host_id: self.peer_host_id.clone(),
            authenticated_peer_boot_id: self.peer_boot_id.clone(),
            membership_credential_id: self.credential_id.clone(),
            line_session_id: self.binding.line_session_id.clone(),
            freshness: request.freshness,
            propagation: Vec::with_capacity(MAX_REMOTE_PROPAGATION_TARGETS),
        })
    }

    fn revalidate(
        &self,
        membership: &BodyMembership,
        session: &ProtectedSession,
    ) -> Result<(), RemoteEmergencyRefusal> {
        let part = membership
            .parts
            .iter()
            .find(|part| part.part_id == self.peer_part_id)
            .ok_or(RemoteEmergencyRefusal::StaleMembership)?;
        let current = part
            .current
            .as_ref()
            .filter(|_| part.state == MembershipState::Admitted)
            .ok_or(RemoteEmergencyRefusal::StaleMembership)?;
        if current.host_id != self.peer_host_id || current.boot_id != self.peer_boot_id {
            return Err(RemoteEmergencyRefusal::StaleMembership);
        }
        let evidence = session.evidence();
        if evidence.disposition != SessionDisposition::Open {
            return Err(RemoteEmergencyRefusal::SessionClosed);
        }
        if evidence.binding != &self.binding {
            return Err(RemoteEmergencyRefusal::WrongSession);
        }
        Ok(())
    }
}

pub fn encode_remote_emergency_request(
    body_id: &BodyId,
    credential_id: &str,
    freshness: u64,
    request_id: &str,
    output: &mut [u8],
) -> Result<usize, RemoteEmergencyRefusal> {
    if freshness == 0 {
        return Err(RemoteEmergencyRefusal::MalformedFrame);
    }
    let needed = 4
        + 1
        + 8
        + encoded_string_len(body_id.as_str())?
        + encoded_string_len(credential_id)?
        + encoded_string_len(request_id)?;
    if needed > MAX_REMOTE_EMERGENCY_PAYLOAD_BYTES || output.len() < needed {
        return Err(RemoteEmergencyRefusal::MalformedFrame);
    }
    output[..4].copy_from_slice(&MAGIC);
    output[4] = VERSION;
    output[5..13].copy_from_slice(&freshness.to_le_bytes());
    let mut offset = 13;
    offset += encode_string(body_id.as_str(), &mut output[offset..])?;
    offset += encode_string(credential_id, &mut output[offset..])?;
    offset += encode_string(request_id, &mut output[offset..])?;
    Ok(offset)
}

struct DecodedRequest<'a> {
    body_id: &'a str,
    credential_id: &'a str,
    request_id: &'a str,
    freshness: u64,
}

fn decode_request(input: &[u8]) -> Result<DecodedRequest<'_>, RemoteEmergencyRefusal> {
    if input.len() < 13 || input[..4] != MAGIC || input[4] != VERSION {
        return Err(RemoteEmergencyRefusal::MalformedFrame);
    }
    let freshness = u64::from_le_bytes(
        input[5..13]
            .try_into()
            .map_err(|_| RemoteEmergencyRefusal::MalformedFrame)?,
    );
    let mut offset = 13;
    let body_id = decode_string(input, &mut offset)?;
    let credential_id = decode_string(input, &mut offset)?;
    let request_id = decode_string(input, &mut offset)?;
    if offset != input.len() || freshness == 0 {
        return Err(RemoteEmergencyRefusal::MalformedFrame);
    }
    Ok(DecodedRequest {
        body_id,
        credential_id,
        request_id,
        freshness,
    })
}

fn validate_current_membership(
    membership: &BodyMembership,
    credential: &MembershipCredential,
) -> Result<(), RemoteEmergencyRefusal> {
    let part = membership
        .parts
        .iter()
        .find(|part| part.part_id == credential.part_id)
        .ok_or(RemoteEmergencyRefusal::StaleMembership)?;
    let current = part
        .current
        .as_ref()
        .filter(|_| part.state == MembershipState::Admitted)
        .ok_or(RemoteEmergencyRefusal::StaleMembership)?;
    if current.host_id != credential.host_id || current.boot_id != credential.boot_id {
        return Err(RemoteEmergencyRefusal::StaleMembership);
    }
    Ok(())
}

fn validate_binding(
    binding: &SessionBinding,
    local_host: &HostId,
    local_boot: &BootId,
    peer_host: &HostId,
    peer_boot: &BootId,
) -> Result<(), RemoteEmergencyRefusal> {
    if binding.line_session_id.is_empty() || binding.line_session_id.len() > MAX_ID_BYTES {
        return Err(RemoteEmergencyRefusal::WrongSession);
    }
    let endpoints = [&binding.initiator, &binding.responder];
    let local = endpoints.iter().any(|endpoint| {
        endpoint.host_id == local_host.as_str() && endpoint.boot_id == local_boot.as_str()
    });
    let peer = endpoints.iter().any(|endpoint| {
        endpoint.host_id == peer_host.as_str() && endpoint.boot_id == peer_boot.as_str()
    });
    if !local || !peer || local_host == peer_host || local_boot == peer_boot {
        return Err(RemoteEmergencyRefusal::WrongSession);
    }
    Ok(())
}

fn encoded_string_len(value: &str) -> Result<usize, RemoteEmergencyRefusal> {
    if value.is_empty() || value.len() > MAX_ID_BYTES {
        return Err(RemoteEmergencyRefusal::MalformedFrame);
    }
    Ok(1 + value.len())
}

fn encode_string(value: &str, output: &mut [u8]) -> Result<usize, RemoteEmergencyRefusal> {
    let length = encoded_string_len(value)?;
    if output.len() < length {
        return Err(RemoteEmergencyRefusal::MalformedFrame);
    }
    output[0] = value.len() as u8;
    output[1..length].copy_from_slice(value.as_bytes());
    Ok(length)
}

fn decode_string<'a>(
    input: &'a [u8],
    offset: &mut usize,
) -> Result<&'a str, RemoteEmergencyRefusal> {
    let length = usize::from(
        *input
            .get(*offset)
            .ok_or(RemoteEmergencyRefusal::MalformedFrame)?,
    );
    *offset += 1;
    if length == 0 || length > MAX_ID_BYTES || input.len().saturating_sub(*offset) < length {
        return Err(RemoteEmergencyRefusal::MalformedFrame);
    }
    let value = core::str::from_utf8(&input[*offset..*offset + length])
        .map_err(|_| RemoteEmergencyRefusal::MalformedFrame)?;
    *offset += length;
    Ok(value)
}

#[cfg(test)]
mod tests;
