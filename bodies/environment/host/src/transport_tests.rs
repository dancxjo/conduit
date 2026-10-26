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
