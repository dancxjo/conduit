use super::*;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::{PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{CertificateError, Error as TlsError};
use std::fs;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::thread;

#[path = "tests/io_deadline.rs"]
mod io_deadline;

#[test]
fn secure_remote_listener_cannot_rebrand_loopback() {
    let result = SecureWebSocketListener::bind(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 443)),
        Path::new("missing-cert.pem"),
        Path::new("missing-key.pem"),
        1024,
        false,
    );
    assert!(matches!(
        result,
        Err(SecureWebSocketError::InvalidConfiguration)
    ));
}

#[test]
fn wildcard_listener_requires_explicit_authorization_and_retains_exact_scope() {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["host.example".into()]).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "conduit-secure-websocket-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("wildcard")
    ));
    fs::create_dir_all(&directory).unwrap();
    let certificate = directory.join("certificate.pem");
    let private_key = directory.join("private-key.pem");
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    let reservation = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));

    assert!(matches!(
        SecureWebSocketListener::bind(address, &certificate, &private_key, 1024, false),
        Err(SecureWebSocketError::InvalidConfiguration)
    ));
    let listener =
        SecureWebSocketListener::bind(address, &certificate, &private_key, 1024, true).unwrap();
    assert_eq!(listener.local_addr().unwrap(), address);

    // One byte of a TLS handshake must not extend the absolute route window
    // through repeated per-read socket timeouts.
    let client = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    let mut client = client;
    client.write_all(&[22]).unwrap();
    let started = Instant::now();
    assert!(matches!(
        listener.accept_with_timeout(Duration::from_millis(80)),
        Err(SecureWebSocketError::Handshake | SecureWebSocketError::AcceptDeadline)
    ));
    assert!(started.elapsed() < Duration::from_secs(1));

    drop(listener);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn absolute_socket_deadline_terminates_a_partial_frame() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut server, _) = listener.accept().unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let _deadline =
        SocketDeadlineGuard::start(&server, Instant::now() + Duration::from_millis(80)).unwrap();
    client.write_all(b"partial").unwrap();
    let started = Instant::now();
    let mut complete_frame = [0_u8; 128];
    assert!(server.read_exact(&mut complete_frame).is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn native_client_verifies_exact_certificate_binding_and_bounded_frames() {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate = cert.der().clone();
    let binding: [u8; 32] = Sha256::digest(certificate.as_ref()).into();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
    let provider = rustls::crypto::ring::default_provider();
    let server_config = ServerConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)
        .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let connection = ServerConnection::new(Arc::new(server_config)).unwrap();
        let tls = StreamOwned::new(connection, stream);
        let mut socket = tungstenite::accept(tls).unwrap();
        assert_eq!(
            socket.read().unwrap(),
            Message::binary(&b"native-pinned"[..])
        );
        socket.send(Message::binary(&b"server-bound"[..])).unwrap();
        socket.close(None).unwrap();
    });
    let mut client = SecureWebSocketClientLine::connect_pinned(
        address,
        &format!("wss://localhost:{}/conduit", address.port()),
        "localhost",
        binding,
        Duration::from_secs(2),
        32,
    )
    .unwrap();
    client.send_binary(b"native-pinned").unwrap();
    let mut response = [0_u8; 32];
    let length = client.receive_binary(&mut response).unwrap();
    assert_eq!(&response[..length], b"server-bound");
    server.join().unwrap();

    assert!(matches!(
        SecureWebSocketClientLine::connect_pinned(
            address,
            &format!("wss://localhost:{}/conduit", address.port()),
            "other-host.invalid",
            binding,
            Duration::from_secs(2),
            32,
        ),
        Err(SecureWebSocketError::InvalidConfiguration)
    ));

    let verifier = PinnedServerCertificate {
        binding_sha256: [7; 32],
        supported: rustls::crypto::ring::default_provider().signature_verification_algorithms,
    };
    assert!(matches!(
        verifier.verify_server_cert(
            cert.der(),
            &[],
            &ServerName::try_from("localhost").unwrap(),
            &[],
            UnixTime::since_unix_epoch(Duration::from_secs(0)),
        ),
        Err(TlsError::InvalidCertificate(
            CertificateError::ApplicationVerificationFailure
        ))
    ));
}

fn pinned_server() -> (Arc<ServerConfig>, [u8; 32]) {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let binding = Sha256::digest(cert.der().as_ref()).into();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
    let config =
        ServerConfig::builder_with_provider(rustls::crypto::ring::default_provider().into())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], key)
            .unwrap();
    (Arc::new(config), binding)
}

#[test]
fn native_client_absolute_connection_deadline_ends_trickled_tls() {
    let (config, binding) = pinned_server();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut connection = ServerConnection::new(config).unwrap();
        while !connection.wants_write() {
            assert_ne!(connection.read_tls(&mut stream).unwrap(), 0);
            connection.process_new_packets().unwrap();
        }
        let mut response = Vec::new();
        while connection.wants_write() {
            connection.write_tls(&mut response).unwrap();
        }
        assert!(!response.is_empty());
        let started = Instant::now();
        // Valid TLS response bytes arrive well inside every individual read
        // timeout. The old per-read bound lasts until the peer finally closes.
        for byte in response {
            if started.elapsed() >= Duration::from_secs(2) || stream.write_all(&[byte]).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        started.elapsed()
    });
    let started = Instant::now();
    let result = SecureWebSocketClientLine::connect_pinned(
        address,
        &format!("wss://localhost:{}/conduit", address.port()),
        "localhost",
        binding,
        Duration::from_millis(250),
        32,
    );
    let elapsed = started.elapsed();
    let peer_closed_after = server.join().unwrap();
    assert!(result.is_err());
    assert!(elapsed < Duration::from_secs(1), "client took {elapsed:?}");
    assert!(
        peer_closed_after < Duration::from_secs(1),
        "failed attempt left its exact socket open for {peer_closed_after:?}"
    );
}

#[test]
fn native_client_completed_handshake_cancels_connection_watchdog() {
    let (config, binding) = pinned_server();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let connection = ServerConnection::new(config).unwrap();
        let mut socket = tungstenite::accept(StreamOwned::new(connection, stream)).unwrap();
        assert_eq!(
            socket.read().unwrap(),
            Message::binary(&b"still-admitted"[..])
        );
        socket
            .send(Message::binary(&b"still-connected"[..]))
            .unwrap();
    });
    let timeout = Duration::from_secs(1);
    let started = Instant::now();
    let mut client = SecureWebSocketClientLine::connect_pinned(
        address,
        &format!("wss://localhost:{}/conduit", address.port()),
        "localhost",
        binding,
        timeout,
        32,
    )
    .unwrap();
    thread::sleep(
        (started + timeout + Duration::from_millis(100)).saturating_duration_since(Instant::now()),
    );
    client.send_binary(b"still-admitted").unwrap();
    let mut response = [0; 32];
    let count = client.receive_binary(&mut response).unwrap();
    assert_eq!(&response[..count], b"still-connected");
    server.join().unwrap();
}

#[test]
fn native_client_exchange_deadline_also_ends_trickled_response_frames() {
    let (config, binding) = pinned_server();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let connection = ServerConnection::new(config).unwrap();
        let mut socket = tungstenite::accept(StreamOwned::new(connection, stream)).unwrap();
        // One valid server binary frame, deliberately incomplete until well
        // after the attempt cutoff, despite continuously arriving TLS records.
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
    let mut client = SecureWebSocketClientLine::connect_pinned_for_exchange(
        address,
        &format!("wss://localhost:{}/conduit", address.port()),
        "localhost",
        binding,
        Duration::from_millis(300),
        128,
    )
    .unwrap();
    let result = client.receive_binary(&mut [0; 128]);
    let elapsed = started.elapsed();
    drop(client);
    server.join().unwrap();
    assert!(result.is_err());
    assert!(
        elapsed < Duration::from_secs(1),
        "exchange took {elapsed:?}"
    );
}

#[test]
fn pinned_tls_partial_frame_survives_poll_timeout() {
    use std::sync::mpsc;
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let directory =
        std::env::temp_dir().join(format!("conduit-tls-partial-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let certificate = directory.join("certificate.pem");
    let private_key = directory.join("private-key.pem");
    fs::write(&certificate, cert.pem()).unwrap();
    fs::write(&private_key, signing_key.serialize_pem()).unwrap();
    let reservation = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let listener =
        SecureWebSocketListener::bind(address, &certificate, &private_key, 1024, true).unwrap();
    let address =
        std::net::SocketAddr::from(([127, 0, 0, 1], listener.local_addr().unwrap().port()));
    let pin = listener.certificate_binding_sha256();
    let (partial, ready) = mpsc::sync_channel(1);
    let (finish, continue_frame) = mpsc::sync_channel(1);
    let (ping, begin_pings) = mpsc::sync_channel(1);
    let client = thread::spawn(move || {
        let mut client = SecureWebSocketClientLine::connect_pinned(
            address,
            &format!("wss://localhost:{}/conduit", address.port()),
            "localhost",
            pin,
            Duration::from_secs(2),
            1024,
        )
        .unwrap();
        // Write a real masked binary WebSocket frame through the established
        // pinned TLS stream in two pieces, rather than splitting application data.
        let payload = [0, 255, 128, 1, 0, 7];
        let mask = [3, 1, 4, 1];
        let mut frame = vec![0x82, 0x80 | payload.len() as u8];
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        client.socket.get_mut().write_all(&frame[..8]).unwrap();
        client.socket.get_mut().flush().unwrap();
        partial.send(()).unwrap();
        continue_frame.recv_timeout(Duration::from_secs(2)).unwrap();
        client.socket.get_mut().write_all(&frame[8..]).unwrap();
        client.socket.get_mut().flush().unwrap();
        begin_pings.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..100 {
            if client.socket.send(Message::Ping(vec![1].into())).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });
    let mut accepted = listener
        .accept_with_timeout(Duration::from_secs(2))
        .unwrap();
    // Authentication handoff is complete; release only this earlier absolute input deadline.
    accepted.complete_bounded_input();
    assert_eq!(accepted.maximum_message_bytes(), 1024);
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    accepted
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let mut bytes = [0; 1024];
    assert!(matches!(
        accepted.receive_binary(&mut bytes),
        Err(SecureWebSocketError::Transport(
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ))
    ));
    finish.send(()).unwrap();
    accepted
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let length = accepted.receive_binary(&mut bytes).unwrap();
    assert_eq!(&bytes[..length], &[0, 255, 128, 1, 0, 7]);
    // Control traffic must not keep an unrenewed participant alive. Replace
    // the completed handshake deadline with a separately selected short lease.
    let started = Instant::now();
    accepted
        .replace_input_deadline(started + Duration::from_millis(80))
        .unwrap();
    ping.send(()).unwrap();
    assert!(accepted.receive_binary(&mut bytes).is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
    client.join().unwrap();
    drop(accepted);
    drop(listener);
    fs::remove_dir_all(directory).unwrap();
}
