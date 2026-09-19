//! Outbound ordinary Host rendezvous above one protected relay Line.

use conduit_protected_line::{
    establish_protected_session, EndpointBinding, ProtectedCarrier, ProtectedSessionPolicy,
    RelayCandidateBounds, RelayCandidateDescriptor, RelayCandidateIdentity, RelayCandidateSecrets,
    RelayEndpointRole, Role, SessionBinding, SessionLimits,
};
use conduit_std_host::relay_client::{HostedRelayCarrier, RelayClientDescriptor};
use serde::Deserialize;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{run_session, RendezvousLine};

const DESCRIPTOR_SCHEMA: &str = "conduit.relay/host-endpoint@1";
const MAXIMUM_DESCRIPTOR_BYTES: u64 = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostRelayDescriptor {
    schema: String,
    address: SocketAddr,
    candidate: SerializedRelayCandidate,
    rendezvous_session_secret: [u8; 32],
}

impl Drop for HostRelayDescriptor {
    fn drop(&mut self) {
        self.candidate.relay_capability.fill(0);
        self.candidate.protected_session_psk.fill(0);
        self.rendezvous_session_secret.fill(0);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SerializedRelayCandidate {
    schema: String,
    relay_implementation_id: String,
    relay_locator: String,
    relay_server_identity: String,
    certificate_binding_sha256: [u8; 32],
    negotiation_id: String,
    route_id: String,
    role: DescriptorRole,
    endpoint_binding: String,
    session_binding: DescriptorSessionBinding,
    expires_at_millis: u64,
    relay_capability: [u8; 32],
    protected_session_psk: [u8; 32],
    bounds: DescriptorLimits,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DescriptorRole {
    Initiator,
    Responder,
}

impl DescriptorRole {
    fn protected(&self) -> Role {
        match self {
            Self::Initiator => Role::Initiator,
            Self::Responder => Role::Responder,
        }
    }

    fn relay(&self) -> RelayEndpointRole {
        match self {
            Self::Initiator => RelayEndpointRole::First,
            Self::Responder => RelayEndpointRole::Second,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorEndpointBinding {
    host_id: String,
    boot_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorSessionBinding {
    initiator: DescriptorEndpointBinding,
    responder: DescriptorEndpointBinding,
    negotiation_id: String,
    line_session_id: String,
    candidate_binding: String,
    transport_binding: String,
}

impl From<&DescriptorSessionBinding> for SessionBinding {
    fn from(binding: &DescriptorSessionBinding) -> Self {
        Self {
            initiator: EndpointBinding {
                host_id: binding.initiator.host_id.clone(),
                boot_id: binding.initiator.boot_id.clone(),
            },
            responder: EndpointBinding {
                host_id: binding.responder.host_id.clone(),
                boot_id: binding.responder.boot_id.clone(),
            },
            negotiation_id: binding.negotiation_id.clone(),
            line_session_id: binding.line_session_id.clone(),
            candidate_binding: binding.candidate_binding.clone(),
            transport_binding: binding.transport_binding.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorLimits {
    maximum_protected_frame_bytes: u32,
    maximum_attempts: u8,
    attempt_timeout_millis: u32,
    maximum_payload_bytes: u32,
    maximum_frames_per_direction: u64,
    maximum_bytes_per_direction: u64,
    handshake_timeout_millis: u32,
    idle_timeout_millis: u32,
}

pub(super) fn connect(state_dir: &Path, descriptor_path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(descriptor_path)
        .map_err(|error| format!("inspect relay endpoint descriptor: {error}"))?;
    if metadata.len() == 0 || metadata.len() > MAXIMUM_DESCRIPTOR_BYTES {
        return Err("relay endpoint descriptor violates its finite byte bound".into());
    }
    let mut bytes = fs::read(descriptor_path)
        .map_err(|error| format!("read relay endpoint descriptor: {error}"))?;
    let decoded = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode relay endpoint descriptor: {error}"));
    bytes.fill(0);
    let mut descriptor: HostRelayDescriptor = decoded?;
    if descriptor.schema != DESCRIPTOR_SCHEMA {
        return Err("relay endpoint descriptor used the wrong protocol".into());
    }
    let now_millis: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock precedes Unix epoch".to_string())?
        .as_millis()
        .try_into()
        .map_err(|_| "system clock exceeds relay representation".to_string())?;
    let policy = ProtectedSessionPolicy {
        traffic: SessionLimits {
            maximum_payload_bytes: descriptor.candidate.bounds.maximum_payload_bytes,
            maximum_frames_per_direction: descriptor.candidate.bounds.maximum_frames_per_direction,
            maximum_bytes_per_direction: descriptor.candidate.bounds.maximum_bytes_per_direction,
        },
        maximum_simultaneous_sessions: 1,
        maximum_pending_frames_per_session: 1,
        handshake_work_units: 2,
        handshake_timeout_millis: descriptor.candidate.bounds.handshake_timeout_millis,
        idle_timeout_millis: descriptor.candidate.bounds.idle_timeout_millis,
    };
    let role = descriptor.candidate.role.protected();
    let relay_role = descriptor.candidate.role.relay();
    let binding: SessionBinding = (&descriptor.candidate.session_binding).into();
    let candidate = RelayCandidateDescriptor::new(
        RelayCandidateIdentity {
            schema: descriptor.candidate.schema.clone(),
            relay_implementation_id: descriptor.candidate.relay_implementation_id.clone(),
            relay_locator: descriptor.candidate.relay_locator.clone(),
            relay_server_identity: descriptor.candidate.relay_server_identity.clone(),
            certificate_binding_sha256: descriptor.candidate.certificate_binding_sha256,
            negotiation_id: descriptor.candidate.negotiation_id.clone(),
            route_id: descriptor.candidate.route_id.clone(),
            role: relay_role,
            endpoint_binding: descriptor.candidate.endpoint_binding.clone(),
            session_binding: binding.clone(),
            expires_at_millis: descriptor.candidate.expires_at_millis,
        },
        RelayCandidateBounds {
            maximum_protected_frame_bytes: descriptor
                .candidate
                .bounds
                .maximum_protected_frame_bytes,
            maximum_attempts: descriptor.candidate.bounds.maximum_attempts,
            attempt_timeout_millis: descriptor.candidate.bounds.attempt_timeout_millis,
            protected_session: policy,
        },
        RelayCandidateSecrets::new(
            descriptor.candidate.relay_capability,
            descriptor.candidate.protected_session_psk,
        ),
        now_millis,
    )
    .map_err(|error| format!("validate portable relay candidate: {error:?}"))?;
    descriptor.candidate.relay_capability.fill(0);
    descriptor.candidate.protected_session_psk.fill(0);
    let client = RelayClientDescriptor {
        address: descriptor.address,
        public_url: candidate.identity.relay_locator.clone(),
        server_identity: candidate.identity.relay_server_identity.clone(),
        certificate_binding_sha256: candidate.identity.certificate_binding_sha256,
        route_id: candidate.identity.route_id.clone(),
        role: candidate.identity.role,
        endpoint_binding: candidate.identity.endpoint_binding.clone(),
        capability: candidate.copy_relay_capability_for_attempt(),
        maximum_protected_frame_bytes: candidate.bounds.maximum_protected_frame_bytes,
        timeout_millis: candidate.bounds.attempt_timeout_millis,
    };
    let mut carrier = HostedRelayCarrier::connect(client)
        .map_err(|error| format!("connect user-operated relay: {error:?}"))?;
    let mut ephemeral_private_key = [0_u8; 32];
    getrandom::fill(&mut ephemeral_private_key)
        .map_err(|error| format!("create protected relay ephemeral key: {error}"))?;
    if ephemeral_private_key == [0; 32] {
        return Err("system randomness returned a weak relay ephemeral key".into());
    }
    let mut protected_session_psk = candidate.copy_protected_session_psk_for_attempt();
    let session = establish_protected_session(
        &mut carrier,
        role,
        &binding,
        policy,
        protected_session_psk,
        ephemeral_private_key,
    )
    .map_err(|error| format!("establish end-to-end protected relay Line: {error:?}"));
    protected_session_psk.fill(0);
    ephemeral_private_key.fill(0);
    let session = session?;
    let mut line = ProtectedCarrier::new(carrier, session, policy)
        .map_err(|error| format!("activate end-to-end protected relay Line: {error:?}"))?;
    let result = run_session(
        &mut line,
        state_dir,
        &descriptor.rendezvous_session_secret,
        "conduit-line/user-operated-protected-relay@1",
    );
    descriptor.rendezvous_session_secret.fill(0);
    result
}

impl RendezvousLine for ProtectedCarrier<HostedRelayCarrier> {
    fn receive(&mut self) -> Result<Vec<u8>, String> {
        self.receive()
            .map(<[u8]>::to_vec)
            .map_err(|error| format!("receive protected relay session frame: {error:?}"))
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send(bytes)
            .map_err(|error| format!("send protected relay session frame: {error:?}"))
    }

    fn close(&mut self) -> Result<(), String> {
        self.close()
            .map_err(|error| format!("close protected relay Line: {error:?}"))
    }
}
