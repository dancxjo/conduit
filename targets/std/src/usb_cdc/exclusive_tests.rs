//! Real Linux PTY descriptors: OS exclusion, decoder custody, refusal and release.
use super::*;
use crate::usb_cdc::FdGuard;
use std::{fs::OpenOptions, io, os::fd::AsRawFd, os::unix::fs::OpenOptionsExt, time::Duration};
fn pty() -> (FdGuard, FdGuard, String) {
    let (mut master, mut slave) = (-1, -1);
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        },
        0
    );
    let mut name = [0 as libc::c_char; 128];
    assert_eq!(
        unsafe { libc::ttyname_r(slave, name.as_mut_ptr(), name.len()) },
        0
    );
    let path = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned();
    (FdGuard(master), FdGuard(slave), path)
}
fn open(path: &str) -> io::Result<std::fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(path)
}
fn write(fd: i32, bytes: &[u8]) {
    assert_eq!(
        unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) },
        bytes.len() as isize
    );
}
#[test]
fn same_descriptor_claim_preserves_partial_decoder_and_releases_on_drop() {
    let (master, _slave, path) = pty();
    let mut line = NativePathCdcLine::open_forwarded_tty(&path, 64).unwrap();
    assert_ne!(
        unsafe { libc::fcntl(line.fd.0, libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
    // A local PTY lock is not proof of a directly attached USB device.
    assert!(line.verify_local_usb_serial("fixture/serial").is_err());
    let mut output = [0; 64];
    write(master.0, b"\0\x05ab");
    assert!(line
        .receive_raw_stream_frame(&mut output, Duration::from_millis(5))
        .is_err());
    line.claim_local_exclusive().unwrap();
    line.claim_local_exclusive().unwrap();
    assert!(line.verify_local_usb_serial("fixture/serial").is_err());
    assert_eq!(open(&path).unwrap_err().raw_os_error(), Some(libc::EBUSY));
    write(master.0, b"cde");
    assert_eq!(
        line.receive_raw_stream_frame(&mut output, Duration::from_secs(1))
            .unwrap(),
        b"abcde"
    );
    drop(line);
    assert!(open(&path).is_ok());
}
#[test]
fn existing_cooperative_owner_refuses_claim_without_discarding_pending_bytes() {
    let (master, _slave, path) = pty();
    let other = open(&path).unwrap();
    assert_eq!(
        unsafe { libc::flock(other.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let mut line = NativePathCdcLine::open_forwarded_tty(&path, 64).unwrap();
    write(master.0, b"\0\x05abcde");
    assert!(line.claim_local_exclusive().is_err());
    assert!(!line.local_exclusive);
    let mut output = [0; 64];
    assert_eq!(
        line.receive_raw_stream_frame(&mut output, Duration::from_secs(1))
            .unwrap(),
        b"abcde"
    );
    drop(other);
    line.claim_local_exclusive().unwrap();
}
#[test]
fn provisioning_claims_local_endpoint_before_returning_socket() {
    let (_master, _slave, path) = pty();
    let socket = crate::pico_spawn::ProvisioningCarrier::ForwardedTty
        .open(&path)
        .unwrap();
    assert_eq!(open(&path).unwrap_err().raw_os_error(), Some(libc::EBUSY));
    drop(socket);
    assert!(open(&path).is_ok());
}

#[test]
fn provisioning_refuses_existing_lock_before_changing_terminal_configuration() {
    let (_master, slave, path) = pty();
    let other = open(&path).unwrap();
    assert_eq!(
        unsafe { libc::flock(other.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    fn attributes(fd: i32) -> libc::termios {
        let mut state = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::tcgetattr(fd, &mut state) }, 0);
        state
    }
    let before = attributes(slave.0);
    assert!(crate::pico_spawn::ProvisioningCarrier::ForwardedTty
        .open(&path)
        .is_err());
    let after = attributes(slave.0);
    assert_eq!(
        (
            before.c_iflag,
            before.c_oflag,
            before.c_cflag,
            before.c_lflag,
            before.c_cc
        ),
        (
            after.c_iflag,
            after.c_oflag,
            after.c_cflag,
            after.c_lflag,
            after.c_cc
        )
    );
}

#[test]
fn provisioning_preserves_unread_framed_bytes_already_queued_on_the_tty() {
    let (master, slave, path) = pty();
    crate::usb_cdc::configure_raw_termios(slave.0).unwrap();
    write(master.0, b"\0\x05abcde");
    let mut line = NativePathCdcLine::open_exclusive_tty(&path, 64, false).unwrap();
    let mut output = [0; 64];
    assert_eq!(
        line.receive_raw_stream_frame(&mut output, Duration::from_secs(1))
            .unwrap(),
        b"abcde"
    );
}
#[test]
fn failed_modem_configuration_releases_its_new_exclusion() {
    let (_master, _slave, path) = pty();
    // A PTY lacks USB modem control, so physical mode must refuse after claim.
    assert!(crate::pico_spawn::ProvisioningCarrier::UsbCdc
        .open(&path)
        .is_err());
    assert!(open(&path).is_ok());
}
