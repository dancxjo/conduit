use super::*;
use std::{
    os::{fd::OwnedFd, unix::net::UnixStream},
    thread::JoinHandle,
};
fn expected() -> ExpectedDeviceIdentity {
    ExpectedDeviceIdentity {
        role: "conduit.environment-wifi-carrier@1".into(),
        firmware: [3; 32],
        configuration: [4; 32],
    }
}
fn status() -> Vec<u8> {
    format!("{{\"role\":\"conduit.environment-wifi-carrier@1\",\"build\":\"{}\",\"configuration\":\"sha256:{}\",\"samples\":10}}",String::from_utf8(hex([3;32]).to_vec()).unwrap(),String::from_utf8(hex([4;32]).to_vec()).unwrap()).into_bytes()
}
fn provider(
    mutate: Option<u8>,
    count: usize,
) -> (DeviceCarrier, HostAdvertisement, JoinHandle<()>) {
    let (client, mut server) = UnixStream::pair().unwrap();
    client.set_nonblocking(true).unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    server
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let fd: OwnedFd = client.into();
    let host = crate::StdHost::new().advertisement().clone();
    let binding = DeviceCarrierBinding {
        host: host.host_id.clone(),
        boot: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        acquisition: [1; 16],
    };
    let owner = DeviceCarrier::from_port(
        Port(File::from(fd)),
        binding,
        expected(),
        Duration::from_secs(1),
    );
    let task = std::thread::spawn(move || {
        for index in 0..count {
            let mut command = [0; HEADER];
            server.read_exact(&mut command).unwrap();
            let parsed = conduit_device_carrier_protocol::decode_header(&command).unwrap();
            assert!(matches!(parsed.kind, 3 | 6 | 7));
            assert_eq!(parsed.length, 0);
            assert_eq!(parsed.id, index as u32 + 1);
            let payload = if mutate == Some(2) {
                b"refused".to_vec()
            } else {
                status()
            };
            let mut response = header(
                if mutate == Some(2) { 130 } else { 131 },
                parsed.id + u32::from(mutate == Some(1)),
                payload.len(),
            )
            .unwrap()
            .to_vec();
            response.extend_from_slice(&payload);
            if mutate == Some(3) {
                let at = response.iter().position(|b| *b == b'3').unwrap();
                response[at] = b'5';
            }
            for chunk in response.chunks(7) {
                server.write_all(chunk).unwrap();
            }
        }
    });
    (owner, host, task)
}
#[test]
fn exclusive_owner_queries_only_bounded_read_only_commands_with_exact_identity() {
    let (mut owner, host, task) = provider(None, 3);
    for query in [
        ReadOnlyQuery::Status,
        ReadOnlyQuery::SensorStatus,
        ReadOnlyQuery::Diagnostics,
    ] {
        let info = owner.query(&host, query).unwrap();
        assert_eq!(info.payload, status());
        assert!(info.observed_at_host_boot_ms > 0);
    }
    task.join().unwrap();
}
#[test]
fn stale_correlation_and_error_poison_the_owner_instead_of_replaying() {
    for (mutation, expected_failure) in [
        (1, Failure::Protocol),
        (2, Failure::Refused),
        (3, Failure::Identity),
    ] {
        let (mut owner, host, task) = provider(Some(mutation), 1);
        assert_eq!(
            owner.query(&host, ReadOnlyQuery::Status).err(),
            Some(expected_failure)
        );
        assert_eq!(
            owner.query(&host, ReadOnlyQuery::Status).err(),
            Some(expected_failure)
        );
        task.join().unwrap();
    }
}
#[test]
fn changed_host_boot_or_offer_refuses_before_any_serial_write() {
    for field in 0..3 {
        let (client, mut server) = UnixStream::pair().unwrap();
        client.set_nonblocking(true).unwrap();
        server.set_nonblocking(true).unwrap();
        let fd: OwnedFd = client.into();
        let mut host = crate::StdHost::new().advertisement().clone();
        let binding = DeviceCarrierBinding {
            host: host.host_id.clone(),
            boot: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            acquisition: [1; 16],
        };
        let mut owner = DeviceCarrier::from_port(
            Port(File::from(fd)),
            binding,
            expected(),
            Duration::from_secs(1),
        );
        match field {
            0 => host.host_id = "host/other".into(),
            1 => host.boot_id = "boot/other".into(),
            _ => host.offer_generation.0 += 1,
        }
        assert_eq!(
            owner.query(&host, ReadOnlyQuery::Status).err(),
            Some(Failure::Scope)
        );
        let mut b = [0; 1];
        assert_eq!(
            server.read(&mut b).unwrap_err().kind(),
            ErrorKind::WouldBlock
        );
    }
}
#[test]
fn duplicate_identity_fields_and_wrong_checked_configuration_refuse() {
    let mut bytes = status();
    bytes.pop();
    bytes.extend_from_slice(b",\"build\":\"duplicated\"}");
    let identity = expected();
    let mut configuration = [0; 71];
    configuration[..7].copy_from_slice(b"sha256:");
    configuration[7..].copy_from_slice(&hex(identity.configuration));
    assert_eq!(
        verify_identity(&bytes, &identity, &hex(identity.firmware), &configuration),
        Err(Failure::Protocol)
    );
    let mut wrong = configuration;
    wrong[70] = b'1';
    assert_eq!(
        verify_identity(&status(), &identity, &hex(identity.firmware), &wrong),
        Err(Failure::Identity)
    );
}

#[test]
fn rejected_native_cooperative_lock_never_touches_the_existing_sessions_dtr() {
    use std::os::fd::FromRawFd;
    let fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    assert!(fd >= 0);
    let master = unsafe { File::from_raw_fd(fd) };
    assert_eq!(unsafe { libc::grantpt(master.as_raw_fd()) }, 0);
    assert_eq!(unsafe { libc::unlockpt(master.as_raw_fd()) }, 0);
    let mut name = [0u8; 128];
    assert_eq!(
        unsafe { libc::ptsname_r(master.as_raw_fd(), name.as_mut_ptr().cast(), name.len()) },
        0
    );
    let name = std::ffi::CStr::from_bytes_until_nul(&name)
        .unwrap()
        .to_str()
        .unwrap();
    let path = Path::new(name);
    let prior = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(prior.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let host = crate::StdHost::new().advertisement().clone();
    let before = DTR_RELEASE_ATTEMPTS.with(|count| count.get());
    assert_eq!(
        DeviceCarrier::acquire(path, &host, expected(), Duration::from_secs(1)).err(),
        Some(Failure::Lost)
    );
    // Linux PTYs do not model readable DTR modem state. Observe the actual native
    // cleanup call instead, while exercising real file-open/flock contention.
    assert_eq!(DTR_RELEASE_ATTEMPTS.with(|count| count.get()), before);
    let contender = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    assert_ne!(
        unsafe { libc::flock(contender.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
}
