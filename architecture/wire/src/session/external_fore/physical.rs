//! Descriptive identities for an independently verified physical installation.
//! No offer, readiness, admission, carrier possession or authority is created.
use crate::WireError;
use alloc::string::String;
use conduit_core::{
    AdmittedLine, AssignedForeDirection, AssignedForeRequirement, BaseImplementationId,
    BaseInstanceId, BootId, BoundLink, HostId, LineContract, LineDuplex, LineId, LineOrdering,
    LineReliability, LineScope, LineSecurity, LineTrafficShape, LinkAuthorityReference,
    LinkBindingId, LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits,
};
use sha2::{Digest, Sha256};

/// Existing request/result CNDF contract used by the native physical carrier.
/// Naming it creates no initialized transport, session or execution authority.
pub const PHYSICAL_FORE_SESSION_CONTRACT: &str = "conduit.wire/physical-fore-cndf@1";

/// Exact installation facts, supplied independently of received frame metadata.
#[derive(Clone, Copy)]
pub struct PhysicalForeInstallation<'a> {
    pub body_id: &'a str,
    pub configuration_id: &'a str,
    pub implementation_id: &'a str,
    pub session_contract: &'a str,
}
/// Both consumers use installed-device-local first, caller peer second.
#[derive(Clone, Copy)]
pub struct PhysicalForePeerFacts<'a> {
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub offer_generation: u64,
}
pub struct PhysicalForeLine {
    pub line: AdmittedLine,
    /// Common to all Fore legs on this exact installed carrier; excludes Fore.
    pub session_id: String,
}
/// Describe a verified installation; never discover or authorize one.
/// None/ProcessOwned are descriptive references, not a grant or provider proof.
/// The supplied physical/reliable/ordered contract must be established by the
/// caller independently. This helper cannot construct an unsafe native carrier
/// selection or its original physical custody ticket.
pub fn describe_physical_fore_line(
    installation: PhysicalForeInstallation<'_>,
    local: PhysicalForePeerFacts<'_>,
    peer: PhysicalForePeerFacts<'_>,
    fore: &AssignedForeRequirement,
    limits: LinkLimits,
    contract: LineContract,
) -> Result<PhysicalForeLine, WireError> {
    let texts = [
        installation.body_id,
        installation.configuration_id,
        installation.implementation_id,
        installation.session_contract,
        local.host_id,
        local.boot_id,
        peer.host_id,
        peer.boot_id,
    ];
    if texts.iter().any(|s| s.is_empty() || s.len() > 160)
        || local.host_id == peer.host_id
        || limits.maximum_in_flight_items == 0
        || limits.maximum_payload_bytes == 0
        || limits.maximum_payload_bytes > limits.maximum_buffered_bytes
        || limits
            .maximum_payload_bytes
            .checked_add(52)
            .is_none_or(|n| n > limits.maximum_frame_bytes)
        || contract.scope != LineScope::PointToPoint
        || contract.traffic_shape != LineTrafficShape::Message
        || contract.security != LineSecurity::PhysicalPossession
        || contract.ordering != LineOrdering::Ordered
        || contract.reliability != LineReliability::Reliable
        || contract.duplex != LineDuplex::FullDuplex
    {
        return Err(WireError::InvalidSession);
    }
    let mut hash = Sha256::new();
    hash.update(b"conduit.physical-fore-installation@1\0");
    for text in texts {
        hash.update((text.len() as u32).to_le_bytes());
        hash.update(text.as_bytes());
    }
    hash.update(local.offer_generation.to_le_bytes());
    hash.update(peer.offer_generation.to_le_bytes());
    hash.update(limits.maximum_in_flight_items.to_le_bytes());
    hash.update(limits.maximum_payload_bytes.to_le_bytes());
    hash.update(limits.maximum_buffered_bytes.to_le_bytes());
    hash.update(limits.maximum_frame_bytes.to_le_bytes());
    hash.update([
        contract.scope as u8,
        contract.traffic_shape as u8,
        contract.duplex as u8,
        contract.ordering as u8,
        contract.reliability as u8,
        contract.continuation as u8,
        contract.security as u8,
    ]);
    let installed: [u8; 32] = hash.finalize().into();
    let session_id = identity(b"session", &installed, &[]);
    let local_endpoint = endpoint(
        local,
        identity(b"local-endpoint", &installed, fore.payload()),
    );
    let peer_endpoint = endpoint(peer, identity(b"peer-endpoint", &installed, fore.payload()));
    let (source, sink) = match fore.direction() {
        AssignedForeDirection::Input => (peer_endpoint, local_endpoint),
        AssignedForeDirection::Output => (local_endpoint, peer_endpoint),
    };
    Ok(PhysicalForeLine {
        session_id,
        line: AdmittedLine {
            line_id: LineId::from(identity(b"line", &installed, fore.payload())),
            binding: BoundLink {
                binding_id: LinkBindingId::from(identity(b"binding", &installed, fore.payload())),
                source,
                sink,
                base: BaseImplementationId::from(installation.implementation_id),
                base_instance_id: BaseInstanceId::from(identity(b"base-instance", &installed, &[])),
                credential: LinkCredentialReference::None,
                authority: LinkAuthorityReference::ProcessOwned,
                limits,
            },
            contract,
        },
    })
}
fn endpoint(facts: PhysicalForePeerFacts<'_>, id: String) -> LinkEndpoint {
    LinkEndpoint {
        host_id: HostId::from(facts.host_id),
        boot_id: BootId::from(facts.boot_id),
        endpoint_id: LinkEndpointId::from(id),
    }
}
fn identity(domain: &[u8], installed: &[u8; 32], fore: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"conduit.physical-fore-identity@1\0");
    hash.update((domain.len() as u32).to_le_bytes());
    hash.update(domain);
    hash.update(installed);
    hash.update(fore);
    let digest: [u8; 32] = hash.finalize().into();
    let mut result = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}
#[cfg(test)]
mod tests;
