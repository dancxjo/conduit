use alloc::string::String;

use crate::{
    ProtectedSessionPolicy, RelayEndpointRole, SessionBinding, RELAY_SERVICE_IMPLEMENTATION_ID,
};

pub const RELAY_CANDIDATE_SCHEMA: &str = "conduit.relay/endpoint-candidate@1";
const MAXIMUM_CANDIDATE_TEXT_BYTES: usize = 256;
const PROTECTED_FRAME_OVERHEAD_BYTES: u32 = 34;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayCandidateIdentity {
    pub schema: String,
    pub relay_implementation_id: String,
    pub relay_locator: String,
    pub relay_server_identity: String,
    pub certificate_binding_sha256: [u8; 32],
    pub negotiation_id: String,
    pub route_id: String,
    pub role: RelayEndpointRole,
    pub endpoint_binding: String,
    pub session_binding: SessionBinding,
    pub expires_at_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayCandidateBounds {
    pub maximum_protected_frame_bytes: u32,
    pub maximum_attempts: u8,
    pub attempt_timeout_millis: u32,
    pub protected_session: ProtectedSessionPolicy,
}

pub struct RelayCandidateSecrets {
    relay_capability: [u8; 32],
    protected_session_psk: [u8; 32],
}

impl RelayCandidateSecrets {
    pub fn new(relay_capability: [u8; 32], protected_session_psk: [u8; 32]) -> Self {
        Self {
            relay_capability,
            protected_session_psk,
        }
    }
}

impl core::fmt::Debug for RelayCandidateSecrets {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("RelayCandidateSecrets([REDACTED])")
    }
}

impl Drop for RelayCandidateSecrets {
    fn drop(&mut self) {
        self.relay_capability.fill(0);
        self.protected_session_psk.fill(0);
    }
}

pub struct RelayCandidateDescriptor {
    pub identity: RelayCandidateIdentity,
    pub bounds: RelayCandidateBounds,
    secrets: RelayCandidateSecrets,
}

impl RelayCandidateDescriptor {
    pub fn new(
        identity: RelayCandidateIdentity,
        bounds: RelayCandidateBounds,
        secrets: RelayCandidateSecrets,
        now_millis: u64,
    ) -> Result<Self, RelayCandidateError> {
        let candidate = Self {
            identity,
            bounds,
            secrets,
        };
        candidate.validate(now_millis)?;
        Ok(candidate)
    }

    pub fn validate(&self, now_millis: u64) -> Result<(), RelayCandidateError> {
        let identity = &self.identity;
        if identity.schema != RELAY_CANDIDATE_SCHEMA
            || identity.relay_implementation_id != RELAY_SERVICE_IMPLEMENTATION_ID
        {
            return Err(RelayCandidateError::WrongProtocol);
        }
        if !bounded(&identity.relay_locator)
            || !identity.relay_locator.starts_with("wss://")
            || !bounded(&identity.relay_server_identity)
            || identity.certificate_binding_sha256 == [0; 32]
        {
            return Err(RelayCandidateError::InvalidRelayIdentity);
        }
        if !bounded(&identity.negotiation_id)
            || !bounded(&identity.route_id)
            || !bounded(&identity.endpoint_binding)
            || identity.route_id.len() > 128
            || identity.endpoint_binding.len() > 128
            || identity.session_binding.negotiation_id != identity.negotiation_id
            || identity.session_binding.candidate_binding != identity.route_id
            || !valid_session_binding(&identity.session_binding)
        {
            return Err(RelayCandidateError::BindingMismatch);
        }
        if identity.expires_at_millis <= now_millis {
            return Err(RelayCandidateError::Expired);
        }
        self.bounds
            .protected_session
            .validate()
            .map_err(|_| RelayCandidateError::InvalidBounds)?;
        let required_frame_bytes = self
            .bounds
            .protected_session
            .traffic
            .maximum_payload_bytes
            .checked_add(PROTECTED_FRAME_OVERHEAD_BYTES)
            .ok_or(RelayCandidateError::InvalidBounds)?;
        if self.bounds.maximum_protected_frame_bytes < required_frame_bytes
            || self.bounds.maximum_protected_frame_bytes > 65_553
            || self.bounds.maximum_attempts == 0
            || self.bounds.maximum_attempts > 16
            || self.bounds.attempt_timeout_millis == 0
            || self.bounds.attempt_timeout_millis > 300_000
            || self.secrets.relay_capability == [0; 32]
            || self.secrets.protected_session_psk == [0; 32]
        {
            return Err(RelayCandidateError::InvalidBounds);
        }
        Ok(())
    }

    /// Copy into one admitted attempt; the caller must erase the returned key.
    pub fn copy_relay_capability_for_attempt(&self) -> [u8; 32] {
        self.secrets.relay_capability
    }

    /// Copy into one admitted handshake; the caller must erase the returned key.
    pub fn copy_protected_session_psk_for_attempt(&self) -> [u8; 32] {
        self.secrets.protected_session_psk
    }
}

impl core::fmt::Debug for RelayCandidateDescriptor {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RelayCandidateDescriptor")
            .field("identity", &self.identity)
            .field("bounds", &self.bounds)
            .field("secrets", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayCandidateError {
    WrongProtocol,
    InvalidRelayIdentity,
    BindingMismatch,
    Expired,
    InvalidBounds,
}

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_CANDIDATE_TEXT_BYTES
}

fn valid_session_binding(binding: &SessionBinding) -> bool {
    let values = [
        binding.initiator.host_id.as_str(),
        binding.initiator.boot_id.as_str(),
        binding.responder.host_id.as_str(),
        binding.responder.boot_id.as_str(),
        binding.negotiation_id.as_str(),
        binding.line_session_id.as_str(),
        binding.candidate_binding.as_str(),
        binding.transport_binding.as_str(),
    ];
    values.iter().all(|value| bounded(value))
        && values
            .iter()
            .try_fold(0_usize, |total, value| total.checked_add(value.len() + 2))
            .is_some_and(|total| total + 20 <= crate::MAXIMUM_BINDING_BYTES)
}
