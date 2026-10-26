//! The authenticated Root framing is identical on the alternate byte carrier.
use super::Line;
use std::{
    io::{Read, Write},
    net::TcpListener,
};
#[test]
fn tcp_carrier_preserves_fragmented_events_and_binary_frames() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut command = [0; 6];
        socket.read_exact(&mut command).unwrap();
        assert_eq!(&command, b"offer\n");
        for bytes in [b"{\"event\":".as_slice(), b"\"offer\",\"ok\":true}\n"] {
            socket.write_all(bytes).unwrap();
        }
        socket.write_all(&[0, 255, 10, 13]).unwrap();
        socket
            .write_all(b"{\"event\":\"native-owner-ingress-credit\"}\n")
            .unwrap();
        let mut payload = [0; 4];
        socket.read_exact(&mut payload).unwrap();
        assert_eq!(payload, [3, 0, 255, 4]);
    });
    let mut line = Line::open(&format!("tcp://{address}")).unwrap();
    assert_eq!(line.command("offer", "offer").unwrap()["ok"], true);
    assert_eq!(line.exact(4).unwrap(), [0, 255, 10, 13]);
    line.credited(&[3, 0, 255, 4]).unwrap();
    peer.join().unwrap();
}

#[test]
fn tcp_reopen_discards_partial_framing_without_replaying_the_lost_command() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let (mut first, _) = listener.accept().unwrap();
        let mut bytes = [0; 8];
        first.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"inspect\n");
        first.write_all(b"{\"event\":\"lost").unwrap();
        drop(first);
        let (mut second, _) = listener.accept().unwrap();
        let mut bytes = [0; 6];
        second.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"offer\n");
        second
            .write_all(b"{\"event\":\"offer\",\"fresh\":true}\n")
            .unwrap();
    });
    let mut line = Line::open(&format!("tcp://{address}")).unwrap();
    let error = line.command("inspect", "inspection").unwrap_err();
    assert!(super::is_recoverable(error.as_ref()));
    line.reconnect().unwrap();
    assert_eq!(line.command("offer", "offer").unwrap()["fresh"], true);
    peer.join().unwrap();
}

#[test]
fn filesystem_or_protocol_failures_do_not_become_carrier_retries() {
    let filesystem: Box<dyn std::error::Error> =
        std::io::Error::new(std::io::ErrorKind::TimedOut, "filesystem timeout").into();
    assert!(!super::is_recoverable(filesystem.as_ref()));
    let protocol: Box<dyn std::error::Error> = "invalid Root signature".into();
    assert!(!super::is_recoverable(protocol.as_ref()));
    let timeout = super::error::timeout();
    let mut context = super::fault::RecentFaults::default();
    assert!(super::is_recoverable(
        context.describe("inspect", timeout).as_ref()
    ));
    context.remember(b"panicked at source.rs:4: memory allocation of");
    assert!(!super::is_recoverable(
        context
            .describe("inspect", super::error::timeout())
            .as_ref()
    ));
}
#[test]
fn private_usb_reply_filter_skips_progress_and_fails_explicit_refusal() {
    for refused in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.write_all(b"stale boot text\n{\"event\":\"local-appliance-progress\"}\n{\"schema\":\"unrelated\"}\n").unwrap();
            if refused {
                socket
                    .write_all(b"{\"event\":\"body-join-refused\"}\n")
                    .unwrap();
            } else {
                socket.write_all(b"{\"schema\":\"challenge\",\"ok\":true}\n{\"invitation\":{\"schema\":\"private\"}}\n").unwrap();
            }
        });
        let mut line = Line::open(&format!("tcp://{address}")).unwrap();
        if refused {
            assert!(line.schema_document("challenge").is_err());
        } else {
            assert_eq!(line.schema_document("challenge").unwrap()["ok"], true);
            assert_eq!(
                line.schema_document("private").unwrap()["invitation"]["schema"],
                "private"
            );
        }
        peer.join().unwrap();
    }
}
#[test]
fn private_usb_reply_filter_enforces_total_discard_budget() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.write_all(&vec![b'x'; 8192]).unwrap();
    });
    let mut line = Line::open(&format!("tcp://{address}")).unwrap();
    assert!(line
        .schema_document("challenge")
        .unwrap_err()
        .to_string()
        .contains("document bound"));
    peer.join().unwrap();
}
