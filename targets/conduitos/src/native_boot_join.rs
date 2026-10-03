//! Product boot's one bounded owner attempt for a provisioned x86 guest.
//!
//! The current native network profile uses QEMU user networking. A candidate
//! must therefore name a literal IPv4 owner; DNS and general network
//! configuration need separate admitted implementations. A verified receipt
//! is returned to the caller, never silently installed as Body membership.

use conduit_body::{
    PortableAdmissionReceipt, RendezvousCandidate, RendezvousLineFamily, RoutedAdmissionRequest,
};

use crate::{
    arch,
    cryptographic_entropy::CryptographicEntropyBase,
    identity::BootIdentities,
    native_guest_face::NativeGuestFace,
    native_owner_admission::{self, NativeOwnerFaceExchangeRefusal, OwnerRouteSeeds},
    native_owner_return::NativeOwnerReturnRoute,
    spore_join::{OwnerExchange, PendingNativeJoin},
    virtio_tcp::VirtioTcpEndpoint,
    wss_candidate_support::literal_ipv4_locator,
};

const MAXIMUM_ATTEMPT_POLLS: u32 = 1_000_000;

pub struct BootJoinOutcome {
    pub pending: PendingNativeJoin,
    /// An exact owner decision; a later product transition must install it.
    pub receipt: Option<PortableAdmissionReceipt>,
    /// A bounded owner Face request follows the verified receipt on the same
    /// authenticated Line. Its refusal never revokes the admitted Part.
    pub face: Option<Result<NativeGuestFace, NativeOwnerFaceExchangeRefusal>>,
    /// A separate finite bearer granted after admission, not the invitation.
    pub return_route: Option<NativeOwnerReturnRoute>,
    /// A return-route failure cannot revoke the already verified admission.
    pub return_refusal: Option<&'static str>,
}

pub fn attempt(
    mut pending: PendingNativeJoin,
    request: Option<RoutedAdmissionRequest>,
    identities: BootIdentities,
) -> BootJoinOutcome {
    let Some(route) = pending.rendezvous.as_ref() else {
        return BootJoinOutcome {
            pending,
            receipt: None,
            face: None,
            return_route: None,
            return_refusal: None,
        };
    };
    let result = request
        .as_ref()
        .ok_or("owner-admission-request-missing")
        .and_then(|request| exchange(route, &pending.route_certificates, request, identities));
    let (receipt, face, return_route, return_refusal) = match result {
        Ok((exchange, return_route, return_refusal)) => {
            pending.owner_exchange = OwnerExchange::ReceiptVerified;
            arch::early_write(b"CONDUIT_NATIVE_OWNER_ADMISSION {\"schema\":\"conduit.conduitos/native-owner-admission@1\",\"status\":\"receipt-verified\",\"membership_installed\":false,\"plan_created\":false,\"play_created\":false}\n");
            if let Some(reason) = return_refusal {
                arch::early_write(b"CONDUIT_NATIVE_OWNER_ROUTE {\"schema\":\"conduit.conduitos/native-owner-route@1\",\"status\":\"refused\",\"code\":\"");
                arch::early_write(reason.as_bytes());
                arch::early_write(b"\"}\n");
            }
            (
                Some(exchange.receipt),
                Some(exchange.face),
                return_route,
                return_refusal,
            )
        }
        Err(reason) => {
            pending.owner_exchange = OwnerExchange::Refused(reason);
            arch::early_write(b"CONDUIT_NATIVE_OWNER_ADMISSION {\"schema\":\"conduit.conduitos/native-owner-admission@1\",\"status\":\"refused\",\"reason\":\"");
            arch::early_write(reason.as_bytes());
            arch::early_write(b"\",\"membership_installed\":false,\"plan_created\":false,\"play_created\":false}\n");
            (None, None, None, None)
        }
    };
    BootJoinOutcome {
        pending,
        receipt,
        face,
        return_route,
        return_refusal,
    }
}

fn exchange(
    route: &conduit_body::SpawnRendezvousDescriptor,
    certificates: &[crate::spore_provision::RouteCertificate],
    request: &RoutedAdmissionRequest,
    identities: BootIdentities,
) -> Result<
    (
        native_owner_admission::NativeOwnerAdmissionExchange,
        Option<NativeOwnerReturnRoute>,
        Option<&'static str>,
    ),
    &'static str,
> {
    let candidate = route
        .candidates
        .iter()
        .find(|candidate| candidate.line_family == RendezvousLineFamily::AuthenticatedTlsStream)
        .ok_or("owner-route-line-unsupported")?;
    let certificate = certificates
        .iter()
        .find(|certificate| certificate.candidate_id == candidate.candidate_id)
        .ok_or("owner-route-certificate-missing")?;
    let endpoint = candidate_endpoint(candidate).ok_or("owner-route-ipv4-unavailable")?;
    let seeds = fresh_seeds()?;
    let device =
        arch::initialize_virtio_net(identities.boot, 1, crate::boot::executable_physical_address)
            .map_err(|error| error.as_str())?;
    // Reconstruct current boot truth independently of the signed request.
    let current = crate::mask_control::native_host_advertisement(
        &conduit_core::HostId::from(crate::identity::hex(&identities.host)),
        &conduit_core::BootId::from(crate::identity::hex(&identities.boot)),
        1,
    );
    let (mut exchange, _, device) = native_owner_admission::exchange_over_candidate(
        device,
        seeds,
        endpoint,
        MAXIMUM_ATTEMPT_POLLS,
        route,
        certificate,
        request,
        &current,
    )
    .map_err(|error| error.as_str())?;
    let (return_route, return_refusal) = match exchange.return_grant.take() {
        Some(grant) => match NativeOwnerReturnRoute::admit(
            grant,
            exchange.receipt.clone(),
            device,
            endpoint,
            candidate.authentication.server_identity.clone(),
            certificate.certificate_der.clone(),
        ) {
            Ok(route) => (Some(route), None),
            Err(reason) => (None, Some(reason)),
        },
        None => (None, None),
    };
    Ok((exchange, return_route, return_refusal))
}

pub(crate) fn fresh_seeds() -> Result<OwnerRouteSeeds, &'static str> {
    let source = arch::RdrandEntropy::detect(1).map_err(|error| error.as_str())?;
    let mut entropy =
        CryptographicEntropyBase::<_, 2>::admit(source).map_err(|error| error.as_str())?;
    let mut seeds = OwnerRouteSeeds {
        tcp: 0,
        tls: [0; 32],
        websocket: [0; 32],
    };
    entropy
        .with_secret::<40, _>(|secret, _| {
            let mut tcp = [0; 8];
            tcp.copy_from_slice(&secret[..8]);
            seeds.tcp = u64::from_le_bytes(tcp);
            seeds.tls.copy_from_slice(&secret[8..]);
        })
        .map_err(|error| error.as_str())?;
    entropy
        .with_secret::<32, _>(|secret, _| seeds.websocket.copy_from_slice(secret))
        .map_err(|error| error.as_str())?;
    Ok(seeds)
}

/// Resolve only an exact IPv4-literal WSS authority. The IPv4 address and port
/// come from the provisioned candidate; guest/gateway settings belong to the
/// current x86 QEMU user-network profile, not to invitation authority.
fn candidate_endpoint(candidate: &RendezvousCandidate) -> Option<VirtioTcpEndpoint> {
    let (bytes, port) = literal_ipv4_locator(&candidate.reachability)?;
    Some(VirtioTcpEndpoint {
        guest_address: [10, 0, 2, 15],
        prefix_length: 24,
        gateway: [10, 0, 2, 2],
        remote_address: bytes,
        remote_port: port,
        local_port: 49152,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(server: &str, reachability: &str) -> RendezvousCandidate {
        RendezvousCandidate {
            candidate_id: "candidate/owner".into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: reachability.into(),
            authentication: conduit_body::RendezvousAuthentication {
                server_identity: server.into(),
                transport_binding_sha256: [1; 32],
            },
            expires_at_millis: 10_000,
            maximum_attempts: 1,
            attempt_timeout_millis: 2_000,
        }
    }

    #[test]
    fn literal_owner_address_is_derived_from_exact_candidate() {
        let endpoint =
            candidate_endpoint(&candidate("owner.example", "wss://10.0.2.100:9000/conduit"))
                .unwrap();
        assert_eq!(endpoint.remote_address, [10, 0, 2, 100]);
        assert_eq!(endpoint.remote_port, 9000);
        assert_eq!(endpoint.guest_address, [10, 0, 2, 15]);
    }

    #[test]
    fn dns_and_mismatched_authority_cannot_become_an_inferred_endpoint() {
        for (server, reachability) in [
            ("owner.example", "wss://owner.example:9000/conduit"),
            ("10.0.2.100", "ws://10.0.2.100:9000/conduit"),
            ("10.0.2.100", "wss://10.0.2.100:9000/other"),
        ] {
            assert!(candidate_endpoint(&candidate(server, reachability)).is_none());
        }
    }
}
