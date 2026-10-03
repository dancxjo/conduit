//! Visible normal-product QEMU boot over one exact provisioned owner route.

use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::Path,
};

use crate::cli::GlobalOpts;

use super::{acceptance, demo, ConduitosError};

pub(super) struct PreparedOwnerBoot {
    pub artifact_sha256: String,
    pub build_id: String,
    pub source_identity: String,
    pub candidate_id: String,
    pub reachability: String,
    pub netdev: String,
}

pub(super) fn prepare(
    spore: &Path,
    candidate_id: &str,
    owner_forward: SocketAddr,
) -> Result<PreparedOwnerBoot, ConduitosError> {
    let SocketAddr::V4(owner_forward) = owner_forward else {
        return Err(ConduitosError::refusal(
            "owner-boot-forward-invalid",
            "the owner forward must be an explicit private IPv4 owner listener",
        ));
    };
    if !owner_forward.ip().is_private() || owner_forward.port() == 0 {
        return Err(ConduitosError::refusal(
            "owner-boot-forward-invalid",
            "the owner forward must be an explicit private IPv4 owner listener with a nonzero port",
        ));
    }
    let route = acceptance::owner_boot_route(spore, candidate_id)?;
    let (guest_address, guest_port) = guest_forward(&route.reachability)?;
    Ok(PreparedOwnerBoot {
        artifact_sha256: route.artifact_sha256,
        build_id: route.build_id,
        source_identity: route.source_identity,
        candidate_id: route.candidate_id,
        reachability: route.reachability,
        netdev: netdev(guest_address, guest_port, owner_forward),
    })
}

pub(super) fn execute(
    spore: &Path,
    candidate_id: &str,
    owner_forward: SocketAddr,
    qmp_socket: Option<&Path>,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let qmp = qmp_socket.map(qmp_arg).transpose()?;
    let route = prepare(spore, candidate_id, owner_forward)?;
    if !opts.quiet && !opts.json {
        println!("Provisioned owner candidate: {}", route.candidate_id);
        println!("Guest route: {}", route.reachability);
        println!("Forwarded to private owner listener: {owner_forward}");
        println!("The guest must validate the provisioned TLS identity and owner receipt.");
    }
    demo::boot_visible_image_with_network(
        spore,
        Some(&route.artifact_sha256),
        Some(&route.netdev),
        qmp.as_deref(),
        opts,
    )
}

#[cfg(unix)]
pub(super) fn qmp_arg(path: &Path) -> Result<String, ConduitosError> {
    use std::{io::ErrorKind, os::unix::fs::PermissionsExt};

    let value = path.to_str().ok_or_else(|| {
        ConduitosError::refusal("owner-boot-qmp-invalid", "QMP socket path must be UTF-8")
    })?;
    if !path.is_absolute()
        || value.len() > 96
        || value.bytes().any(|byte| byte == b',' || byte < b' ')
    {
        return Err(ConduitosError::refusal(
            "owner-boot-qmp-invalid",
            "QMP socket path must be absolute, short, and contain no QEMU separators",
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        ConduitosError::refusal("owner-boot-qmp-invalid", "QMP socket has no private parent")
    })?;
    let parent_meta = std::fs::metadata(parent).map_err(|error| {
        ConduitosError::refusal("owner-boot-qmp-invalid", format!("QMP parent: {error}"))
    })?;
    if !parent_meta.is_dir() || parent_meta.permissions().mode() & 0o077 != 0 {
        return Err(ConduitosError::refusal(
            "owner-boot-qmp-invalid",
            "QMP socket parent must be an existing private directory",
        ));
    }
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(ConduitosError::refusal(
                "owner-boot-qmp-invalid",
                "QMP socket path already exists",
            ));
        }
        Err(error) => {
            return Err(ConduitosError::refusal(
                "owner-boot-qmp-invalid",
                format!("QMP socket path: {error}"),
            ));
        }
    }
    Ok(format!("unix:{value},server=on,wait=off"))
}

#[cfg(not(unix))]
pub(super) fn qmp_arg(_path: &Path) -> Result<String, ConduitosError> {
    Err(ConduitosError::refusal(
        "owner-boot-qmp-unsupported",
        "this QMP socket capture is available only on Unix hosts",
    ))
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

    #[cfg(unix)]
    #[test]
    fn qmp_capture_requires_a_new_socket_in_a_private_directory() {
        use std::{os::unix::fs::PermissionsExt, time::SystemTime};

        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let private =
            Path::new("/tmp").join(format!("conduit-qmp-{}-{nonce:x}", std::process::id()));
        std::fs::create_dir(&private).unwrap();
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
        let socket = private.join("monitor.sock");
        assert_eq!(
            qmp_arg(&socket).unwrap(),
            format!("unix:{},server=on,wait=off", socket.display())
        );
        std::fs::write(&socket, b"occupied").unwrap();
        assert_eq!(
            qmp_arg(&socket).unwrap_err().reason,
            "owner-boot-qmp-invalid"
        );
        std::fs::remove_file(&socket).unwrap();
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            qmp_arg(&socket).unwrap_err().reason,
            "owner-boot-qmp-invalid"
        );
        std::fs::remove_dir(&private).unwrap();
        assert_eq!(
            qmp_arg(Path::new("relative.sock")).unwrap_err().reason,
            "owner-boot-qmp-invalid"
        );
    }

    #[test]
    fn forward_is_derived_from_exact_numeric_candidate_and_explicit_owner_listener() {
        let (guest, port) = guest_forward("wss://10.0.2.100:9000/conduit").unwrap();
        assert_eq!(
            netdev(guest, port, "172.17.0.1:19000".parse().unwrap()),
            "user,id=conduit-owner,restrict=on,guestfwd=tcp:10.0.2.100:9000-tcp:172.17.0.1:19000"
        );
    }

    #[test]
    fn owner_forward_requires_a_private_nonloopback_ipv4_listener() {
        for address in [
            "127.0.0.1:19000",
            "0.0.0.0:19000",
            "192.0.2.1:19000",
            "172.17.0.1:0",
            "[::1]:19000",
        ] {
            let address: SocketAddr = address.parse().unwrap();
            let refusal = execute(
                Path::new("absent.iso"),
                "candidate/owner",
                address,
                None,
                &GlobalOpts::default(),
            )
            .unwrap_err();
            assert_eq!(refusal.reason, "owner-boot-forward-invalid", "{address}");
        }
        let later_refusal = execute(
            Path::new("absent.iso"),
            "candidate/owner",
            "172.17.0.1:19000".parse().unwrap(),
            None,
            &GlobalOpts::default(),
        )
        .unwrap_err();
        assert_ne!(later_refusal.reason, "owner-boot-forward-invalid");
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
