//! Actual local PTY framing/ownership fixture, without physical/Start claims.
use super::*;
use std::{
    ffi::CStr,
    fs::File,
    io::{Read, Write},
    os::fd::FromRawFd,
};
fn read_frame(file: &mut File) -> Vec<u8> {
    let mut length = [0; 2];
    file.read_exact(&mut length).unwrap();
    let mut bytes = vec![0; u16::from_be_bytes(length) as usize];
    file.read_exact(&mut bytes).unwrap();
    bytes
}
#[test]
fn consuming_transition_keeps_original_tty_and_decoder_after_raw_prelude_fixture() {
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
    // Own both descriptors immediately, including unwind paths.
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    use std::os::fd::AsRawFd;
    let mut name = [0 as libc::c_char; 128];
    assert_eq!(
        unsafe { libc::ttyname_r(slave.as_raw_fd(), name.as_mut_ptr(), name.len()) },
        0
    );
    let path = unsafe { CStr::from_ptr(name.as_ptr()) }.to_str().unwrap();
    let mut socket = PicoSpawnSocket {
        line: super::super::super::NativePathCdcLine::open_forwarded_tty(path, 4096).unwrap(),
    };
    socket
        .line
        .send_raw_stream_frame(b"raw-prelude-fixture", TIMEOUT)
        .unwrap();
    assert_eq!(read_frame(&mut master), b"raw-prelude-fixture");
    let prepared = PreparedForeCarriage::prepare(bindings(), bounds()).unwrap();
    let mut carriage = socket.into_fore_carriage(prepared);
    assert!(
        !carriage.is_active(),
        "owning a socket supplies no Ready, grant or Start"
    );
    carriage.send(0, Message::Hello, TIMEOUT).unwrap();
    let bytes = read_frame(&mut master);
    assert_eq!(
        decode_external_fore_session_frame(&bytes, 16, 128).unwrap(),
        bindings()[0].frame(Message::Hello)
    );
    let mut bytes = [0; 128];
    let n = encode_external_fore_session_frame_into(
        bindings()[0].frame(Message::Hello),
        &mut bytes,
        16,
        128,
    )
    .unwrap();
    // Existing CDC big-endian stream length, unchanged by the transition.
    master.write_all(&(n as u16).to_be_bytes()).unwrap();
    master.write_all(&bytes[..n]).unwrap();
    let mut output = [0; 128];
    let frame = carriage.receive(&mut output, TIMEOUT).unwrap();
    assert_eq!(frame.route, 0);
    assert_eq!(frame.frame.message, Message::Hello);
    assert!(!carriage.is_active());
    carriage.cancel();
    assert_eq!(
        carriage.send(0, Message::Message(SessionMessage::Ready), TIMEOUT),
        Err(ForeCarriageError::Retired)
    );
}
