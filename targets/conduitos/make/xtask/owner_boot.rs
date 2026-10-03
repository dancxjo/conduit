//! Visible normal-product QEMU boot over one exact provisioned owner route.

use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::Path,
};

use crate::cli::GlobalOpts;

use super::{acceptance, demo, ConduitosError};

pub(super) fn execute(
    spore: &Path,
    candidate_id: &str,
    owner_forward: SocketAddr,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let SocketAddr::V4(owner_forward) = owner_forward else {
        return Err(ConduitosError::refusal(
            "owner-boot-forward-invalid",
            "the owner forward must be an explicit IPv4 loopback listener",
        ));
    };
    if !owner_forward.ip().is_loopback() || owner_forward.port() == 0 {
        return Err(ConduitosError::refusal(
            "owner-boot-forward-invalid",
            "the owner forward must be an explicit IPv4 loopback listener with a nonzero port",
        ));
    }
    let route = acceptance::owner_boot_route(spore, candidate_id)?;
    let (guest_address, guest_port) = guest_forward(&route.reachability)?;
    let netdev = netdev(guest_address, guest_port, owner_forward);
    if !opts.quiet && !opts.json {
        println!("Provisioned owner candidate: {}", route.candidate_id);
        println!("Guest route: {}", route.reachability);
        println!("Forwarded to local owner: {owner_forward}");
        println!("The guest must validate the provisioned TLS identity and owner receipt.");
    }
    demo::boot_visible_image_with_network(spore, Some(&route.artifact_sha256), Some(&netdev), opts)
}

fn guest_forward(reachability: &str) -> Result<(Ipv4Addr, u16), ConduitosError> {
    let authority = reachability
        .strip_prefix("wss://")
        .and_then(|remainder| remainder.strip_suffix("/conduit"))
        .ok_or_else(|| {
            ConduitosError::refusal(
                "owner-boot-route-unsupported",
                "candidate must have an exact IPv4-literal wss:// address and /conduit path",
            )
        })?;
    let (host, port) = authority.split_once(':').ok_or_else(|| {
        ConduitosError::refusal(
            "owner-boot-route-unsupported",
            "candidate must specify its guest port explicitly",
        )
    })?;
    let guest_address: Ipv4Addr = host.parse().map_err(|_| {
        ConduitosError::refusal(
            "owner-boot-route-unsupported",
            "candidate must name a literal IPv4 guestfwd address",
        )
    })?;
    let guest_port: u16 = port.parse().map_err(|_| {
        ConduitosError::refusal(
            "owner-boot-route-unsupported",
            "candidate guestfwd port is invalid",
        )
    })?;
    if guest_address.octets()[..3] != [10, 0, 2]
        || matches!(guest_address.octets()[3], 0 | 2 | 15 | 255)
        || guest_port == 0
    {
        return Err(ConduitosError::refusal(
            "owner-boot-route-unsupported",
            "candidate must use an unreserved 10.0.2.x guestfwd address and a nonzero port for this QEMU profile",
        ));
    }
    Ok((guest_address, guest_port))
}

fn netdev(guest_address: Ipv4Addr, guest_port: u16, owner: SocketAddrV4) -> String {
    format!(
        "user,id=conduit-owner,restrict=on,guestfwd=tcp:{guest_address}:{guest_port}-tcp:{owner}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_is_derived_from_exact_numeric_candidate_and_explicit_owner_listener() {
        let (guest, port) = guest_forward("wss://10.0.2.100:9000/conduit").unwrap();
        assert_eq!(
            netdev(guest, port, "127.0.0.1:19000".parse().unwrap()),
            "user,id=conduit-owner,restrict=on,guestfwd=tcp:10.0.2.100:9000-tcp:127.0.0.1:19000"
        );
    }

    #[test]
    fn unsupported_candidate_address_never_becomes_a_forward() {
        for route in [
            "wss://owner.example:9000/conduit",
            "ws://10.0.2.100:9000/conduit",
            "wss://10.0.2.100:9000/other",
            "wss://10.0.2.15:9000/conduit",
            "wss://10.0.3.100:9000/conduit",
            "wss://10.0.2.100:0/conduit",
        ] {
            assert!(guest_forward(route).is_err(), "{route}");
        }
    }
}
