//! Persistent transport measurement, separate from Body/kernel conformance.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_std_host::websocket::{NativeWebSocketLine, NativeWebSocketListener};
use std::{
    sync::mpsc::sync_channel,
    time::{Duration, Instant},
};
const BYTES: usize = 4096;
const ROUNDS: usize = 64;

fn receive(
    line: &mut NativeWebSocketLine,
    borrowed: bool,
    buffer: &mut [u8],
    expected: &[u8],
) -> usize {
    if borrowed {
        line.receive_binary_with(|bytes| assert_eq!(bytes, expected))
            .unwrap();
        0
    } else {
        let length = line.receive_binary(buffer).unwrap();
        assert_eq!(&buffer[..length], expected);
        length
    }
}

#[test]
#[ignore = "manual matched persistent-Line transport measurement"]
fn established_native_line_compares_copy_and_borrow_without_reconnection() {
    for borrowed in [false, true] {
        let started = Instant::now();
        let listener = NativeWebSocketListener::bind_loopback(BYTES as u32).unwrap();
        let address = listener.local_addr().unwrap();
        let (peer_ready, ready) = sync_channel(0);
        let (start, peer_start) = sync_channel(0);
        let peer = std::thread::spawn(move || {
            let mut line = listener
                .accept_with_timeout(Duration::from_secs(5))
                .unwrap();
            line.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buffer = [0; BYTES];
            let mut payload = [0x5a; BYTES];
            peer_ready.send(()).unwrap();
            peer_start.recv_timeout(Duration::from_secs(5)).unwrap();
            let started = Instant::now();
            let (copied, allocation) = allocation_probe::observe(|| {
                let mut copied = 0;
                for sequence in 0..ROUNDS as u64 {
                    payload[..8].copy_from_slice(&sequence.to_le_bytes());
                    copied += receive(&mut line, borrowed, &mut buffer, &payload);
                    line.send_binary(&sequence.to_le_bytes()).unwrap();
                }
                copied
            });
            (copied, allocation, started.elapsed().as_nanos())
        });
        let mut line =
            NativeWebSocketLine::connect(address, &format!("ws://{address}/conduit"), BYTES as u32)
                .unwrap();
        line.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut payload = [0x5a; BYTES];
        let mut acknowledgement = [0; 8];
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let setup_ns = started.elapsed().as_nanos();
        start.send(()).unwrap();
        let started = Instant::now();
        let (copied, allocation) = allocation_probe::observe(|| {
            let mut copied = 0;
            for sequence in 0..ROUNDS as u64 {
                payload[..8].copy_from_slice(&sequence.to_le_bytes());
                line.send_binary(&payload).unwrap();
                copied += receive(
                    &mut line,
                    borrowed,
                    &mut acknowledgement,
                    &sequence.to_le_bytes(),
                );
            }
            copied
        });
        let warm_ns = started.elapsed().as_nanos();
        let (peer_copied, peer_allocation, peer_ns) = peer.join().unwrap();
        assert_eq!(copied, if borrowed { 0 } else { ROUNDS * 8 });
        assert_eq!(peer_copied, if borrowed { 0 } else { ROUNDS * BYTES });
        println!(
            "WARM_WEBSOCKET_RECEIVE={}",
            serde_json::json!({
                "borrowed": borrowed, "rounds": ROUNDS, "payload_bytes": BYTES,
                "scope": "established loopback native WebSocket transport; excludes session codec/kernel and OS/framing copies",
                "setup_ns": setup_ns, "warm_client_ns": warm_ns, "warm_peer_ns": peer_ns,
                "client_allocations": allocation.allocations, "peer_allocations": peer_allocation.allocations,
                "client_reallocations": allocation.reallocations, "peer_reallocations": peer_allocation.reallocations,
                "client_receive_copy_bytes": copied, "peer_receive_copy_bytes": peer_copied,
                "wire_application_bytes": ROUNDS * (BYTES + 8),
            })
        );
    }
}

#[test]
fn borrowed_receive_refuses_text_and_oversized_frames_before_callback() {
    use conduit_std_host::websocket::NativeWebSocketError;
    use tungstenite::Message;
    for (message, expected) in [
        (
            Message::Text("not binary".into()),
            NativeWebSocketError::TextMessageRejected,
        ),
        (
            Message::Binary(vec![0; 17].into()),
            NativeWebSocketError::OversizedMessage,
        ),
    ] {
        let listener = NativeWebSocketListener::bind_loopback(16).unwrap();
        let url = listener.url().unwrap();
        let peer = std::thread::spawn(move || {
            let mut line = listener
                .accept_with_timeout(Duration::from_secs(5))
                .unwrap();
            line.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            assert_eq!(
                line.receive_binary_with(|_| panic!("invalid message reached consumer")),
                Err(expected)
            );
        });
        let (mut socket, _) = tungstenite::connect(url.as_str()).unwrap();
        socket.send(message).unwrap();
        peer.join().unwrap();
    }
}

#[test]
fn traffic_counts_actual_borrowed_and_copied_receives() {
    use conduit_std_host::websocket::NativeWebSocketError;
    let listener = NativeWebSocketListener::bind_loopback(16).unwrap();
    let address = listener.local_addr().unwrap();
    let url = listener.url().unwrap();
    let peer = std::thread::spawn(move || {
        let mut line = listener
            .accept_with_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(line.traffic().is_none());
        line.start_traffic_observation();
        line.receive_binary_with(|bytes| assert_eq!(bytes, b"abc"))
            .unwrap();
        let mut buffer = [0; 3];
        assert_eq!(line.receive_binary(&mut buffer).unwrap(), 3);
        assert_eq!(&buffer, b"def");
        assert_eq!(
            line.receive_binary(&mut []),
            Err(NativeWebSocketError::OutputTooSmall)
        );
        let traffic = line.traffic().unwrap();
        assert_eq!(traffic.received_messages, 3);
        assert_eq!(traffic.received_binary_bytes, 9);
        assert_eq!(traffic.receive_copy_bytes, 3);
        assert!(!traffic.saturated);
    });
    let mut line = NativeWebSocketLine::connect(address, &url, 16).unwrap();
    line.start_traffic_observation();
    for bytes in [b"abc", b"def", b"ghi"] {
        line.send_binary(bytes).unwrap();
    }
    assert_eq!(
        line.send_binary(&[0; 17]),
        Err(NativeWebSocketError::OversizedMessage)
    );
    let traffic = line.traffic().unwrap();
    assert_eq!(traffic.sent_messages, 3);
    assert_eq!(traffic.sent_binary_bytes, 9);
    assert_eq!(traffic.transmit_copy_bytes, 9);
    peer.join().unwrap();
}
