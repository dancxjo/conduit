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
}
