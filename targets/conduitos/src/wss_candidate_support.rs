//! Exact binding between a portable WSS candidate and an admitted endpoint.

use sha2::{Digest, Sha256};

pub(crate) fn locator_matches(locator: &str, server_identity: &str, remote_port: u16) -> bool {
    let Some(rest) = locator.strip_prefix("wss://") else {
        return false;
    };
    let Some((authority, path)) = rest.split_once('/') else {
        return false;
    };
    if path != "conduit" {
        return false;
    }
    if remote_port == 443 && authority == server_identity {
        return true;
    }
    authority
        .strip_prefix(server_identity)
        .and_then(|suffix| suffix.strip_prefix(':'))
        .and_then(|value| value.parse::<u16>().ok())
        == Some(remote_port)
}

/// A provisioned numeric locator may name a transport IP while the separate
/// pinned TLS identity remains a DNS name. Return only exact IPv4 authority.
pub(crate) fn literal_ipv4_locator(locator: &str) -> Option<([u8; 4], u16)> {
    let rest = locator.strip_prefix("wss://")?;
    let (authority, path) = rest.split_once('/')?;
    if path != "conduit" {
        return None;
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().ok()?),
        None => (authority, 443),
    };
    if port == 0 {
        return None;
    }
    let mut bytes = [0; 4];
    let mut parts = host.split('.');
    for byte in &mut bytes {
        *byte = parts.next()?.parse().ok()?;
    }
    if parts.next().is_some() || bytes == [0; 4] {
        return None;
    }
    Some((bytes, port))
}

pub(crate) fn certificate_matches(expected_sha256: [u8; 32], certificate_der: &[u8]) -> bool {
    let observed: [u8; 32] = Sha256::digest(certificate_der).into();
    observed == expected_sha256
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locator_binding_is_exact_about_scheme_authority_port_and_path() {
        assert!(locator_matches(
            "wss://relay.example:8443/conduit",
            "relay.example",
            8443
        ));
        assert!(locator_matches(
            "wss://relay.example/conduit",
            "relay.example",
            443
        ));
        for invalid in [
            "ws://relay.example:8443/conduit",
            "wss://other.example:8443/conduit",
            "wss://relay.example:443/conduit",
            "wss://relay.example:8443/other",
        ] {
            assert!(!locator_matches(invalid, "relay.example", 8443));
        }
    }

    #[test]
    fn literal_locator_keeps_transport_address_separate_from_tls_identity() {
        assert_eq!(
            literal_ipv4_locator("wss://10.0.2.100:9000/conduit"),
            Some(([10, 0, 2, 100], 9000))
        );
        assert_eq!(
            literal_ipv4_locator("wss://10.0.2.100/conduit"),
            Some(([10, 0, 2, 100], 443))
        );
        for invalid in [
            "wss://owner.example:9000/conduit",
            "wss://10.0.2.100:0/conduit",
            "wss://10.0.2.100:9000/other",
            "ws://10.0.2.100:9000/conduit",
        ] {
            assert_eq!(literal_ipv4_locator(invalid), None);
        }
    }
}
