use super::*;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::PrivatePkcs8KeyDer;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::thread;

#[test]
fn secure_remote_listener_cannot_rebrand_loopback() {
    let result = SecureWebSocketListener::bind(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 443)),
        Path::new("missing-cert.pem"),
        Path::new("missing-key.pem"),
        1024,
    );
    assert!(matches!(
        result,
        Err(SecureWebSocketError::InvalidConfiguration)
    ));
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
