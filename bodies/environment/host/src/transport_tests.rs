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

fn receive_http_request(socket: &mut std::net::TcpStream) {
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    let mut byte = [0];
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        socket.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        assert!(header.len() <= 1024);
    }
}

#[test]
fn declared_http_response_completes_before_peer_closes() {
    for (status, body) in [(202, vec![]), (200, vec![0, 255, 13, 10])] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let expected = body.clone();
        let (complete, returned) = std::sync::mpsc::channel();
        let peer = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            receive_http_request(&mut socket);
            let mut response = format!(
                "HTTP/1.1 {status} OK\r\ncOnTeNt-LeNgTh: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            response.extend_from_slice(&body);
            for fragment in response.chunks(7) {
                socket.write_all(fragment).unwrap();
            }
            // Causal proof, not a latency threshold: this socket remains open
            // until the complete response has actually returned to its caller.
            returned
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("HTTP client waited for EOF despite a complete declared response");
        });
        let response =
            crate::body_join::http(&address.to_string(), "GET", "/complete", &[]).unwrap();
        assert_eq!(response, (status, expected));
        complete.send(()).unwrap();
        peer.join().unwrap();
    }
}

#[test]
fn http_truncation_refuses_and_legacy_close_delimited_body_remains_supported() {
    for declared in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            receive_http_request(&mut socket);
            if declared {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
                    .unwrap();
            } else {
                socket
                    .write_all(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\nab")
                    .unwrap();
            }
        });
        let response = crate::body_join::http(&address.to_string(), "GET", "/truncated", &[]);
        if declared {
            assert!(response.unwrap_err().to_string().contains("truncated body"));
        } else {
            assert_eq!(response.unwrap(), (200, b"ab".to_vec()));
        }
        peer.join().unwrap();
    }
}
