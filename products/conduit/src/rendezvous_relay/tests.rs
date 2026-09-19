use super::*;

fn options(bind: &str, authorize_network: bool) -> ServeOptions {
    ServeOptions {
        bind: bind.into(),
        public_url: "wss://relay.example/conduit".into(),
        tls_cert: "unused-cert.pem".into(),
        tls_key: "unused-key.pem".into(),
        slot: "unused-slot.json".into(),
        accept_timeout_seconds: 1,
        authorize_network,
    }
}

#[test]
fn network_listening_requires_explicit_non_loopback_authority() {
    assert_eq!(
        serve(options("192.0.2.10:7443", false)),
        Err("relay network listening requires --authorize-network".into())
    );
    assert_eq!(
        serve(options("127.0.0.1:7443", true)),
        Err("relay --bind must be one explicit non-loopback socket".into())
    );
    assert_eq!(
        serve(options("192.0.2.10:0", true)),
        Err("relay --bind must be one explicit non-loopback socket".into())
    );
}

#[test]
fn capability_ingress_is_exact_and_erases_the_source() {
    let mut exact = vec![7; 32];
    assert_eq!(take_capability(&mut exact), Ok([7; 32]));
    assert_eq!(exact, vec![0; 32]);

    let mut short = vec![9; 31];
    assert_eq!(
        take_capability(&mut short),
        Err("relay capability must contain exactly 32 bytes".into())
    );
    assert_eq!(short, vec![0; 31]);
}

#[test]
fn explicit_close_control_is_versioned_and_exact() {
    let control: Control =
        serde_json::from_slice(br#"{"schema":"conduit.relay/control@1","kind":"close"}"#).unwrap();
    assert_eq!(control.schema, CONTROL_SCHEMA);
    assert!(matches!(control.kind, ControlKind::Close));
    assert!(serde_json::from_slice::<Control>(
        br#"{"schema":"conduit.relay/control@1","kind":"close","retry":true}"#,
    )
    .is_err());
}
