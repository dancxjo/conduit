//! Pure chosen-name parser proof; no radio, discovery or Root authentication.
#[path = "../../usb-http-relay/src/bench_mdns.rs"]
pub(crate) mod bench_mdns;
mod protocol {
    pub(crate) use crate::bench_mdns;
}
#[path = "../src/wifi_mdns_query.rs"]
mod query;
#[test]
fn nonce_and_exact_mac_named_host_are_required() {
    for suffix in [".local", "-2.local", "-3.local"] {
        let mut bytes = vec![1, 2, 3, 4, 5, 6, 7, 8];
        bytes.extend_from_slice(format!("conduit-climate-b0a732286178{suffix}").as_bytes());
        let parsed = query::parse(&bytes).unwrap();
        assert_eq!(parsed.nonce, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(parsed.id(), 258);
        let mut packet = [0; 512];
        assert!(parsed.packet(&mut packet) > 12);
        assert!(parsed
            .answer(&[0; 12], [10, 0, 0, 1], 5353, [10, 0, 0, 2])
            .is_err());
    }
    for host in [
        "other-b0a732286178.local",
        "conduit-climate-B0a732286178.local",
        "conduit-climate-b0a73228617z.local",
        "conduit-climate-b0a732286178-4.local",
        "conduit-climate-b0a732286178.local.evil",
    ] {
        let mut bytes = vec![1; 8];
        bytes.extend_from_slice(host.as_bytes());
        assert!(query::parse(&bytes).is_err(), "{host}");
    }
    for n in 0..8 {
        assert!(query::parse(&vec![1; n]).is_err());
    }
    let mut zero = vec![0; 8];
    zero.extend_from_slice(b"conduit-climate-b0a732286178.local");
    assert!(query::parse(&zero).is_err());
}
