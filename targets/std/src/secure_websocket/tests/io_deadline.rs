//! Established client lease enforcement, distinct from connection deadlines.
use super::*;

fn connected(
    serve: impl FnOnce(WebSocket<StreamOwned<ServerConnection, TcpStream>>) + Send + 'static,
) -> (SecureWebSocketClientLine, thread::JoinHandle<()>) {
    let (config, binding) = pinned_server();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let connection = ServerConnection::new(config).unwrap();
        serve(tungstenite::accept(StreamOwned::new(connection, stream)).unwrap());
    });
    let client = SecureWebSocketClientLine::connect_pinned(
        address,
        &format!("wss://localhost:{}/conduit", address.port()),
        "localhost",
        binding,
        Duration::from_secs(2),
        128,
    )
    .unwrap();
    (client, server)
}

#[test]
fn client_io_deadline_stops_trickle_and_failed_replacement_keeps_prior_guard() {
    let (mut client, server) = connected(|mut socket| {
        socket.get_mut().write_all(&[0x82, 64]).unwrap();
        socket.get_mut().flush().unwrap();
        for _ in 0..64 {
            if socket.get_mut().write_all(&[1]).is_err() || socket.get_mut().flush().is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    });
    let started = Instant::now();
    client
        .replace_io_deadline(started + Duration::from_millis(100))
        .unwrap();
    assert!(client
        .replace_io_deadline(Instant::now() - Duration::from_millis(1))
        .is_err());
    let result = client.receive_binary(&mut [0; 128]);
    let elapsed = started.elapsed();
    drop(client);
    server.join().unwrap();
    assert!(result.is_err());
    assert!(elapsed < Duration::from_secs(1), "I/O took {elapsed:?}");
}

#[test]
fn client_io_deadline_replacement_cancels_old_guard_and_enforces_new_lease() {
    let (mut client, server) = connected(|mut socket| {
        assert_eq!(socket.read().unwrap(), Message::binary(&b"renewed"[..]));
        socket.send(Message::binary(&b"accepted"[..])).unwrap();
        assert!(socket.read().is_err());
    });
    let old = Instant::now() + Duration::from_millis(120);
    let renewed = old + Duration::from_millis(700);
    client.replace_io_deadline(old).unwrap();
    client.replace_io_deadline(renewed).unwrap();
    thread::sleep((old + Duration::from_millis(80)).saturating_duration_since(Instant::now()));
    client.send_binary(b"renewed").unwrap();
    let mut output = [0; 128];
    let count = client.receive_binary(&mut output).unwrap();
    assert_eq!(&output[..count], b"accepted");
    let result = client.receive_binary(&mut output);
    let ended = Instant::now();
    drop(client);
    server.join().unwrap();
    assert!(result.is_err());
    assert!(ended >= renewed, "the obsolete guard closed the socket");
    assert!(ended < renewed + Duration::from_secs(1));
}
