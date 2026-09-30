use conduit_form::rust_binding::NativeRustBinding;
use conduit_net::{
    DnsRecordKind, NetworkChunkShape, NetworkFrameDirection, NetworkFrameProtocol, NetworkTransport,
};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + Eq,
{
    let encoded = value.encode().unwrap();
    assert_eq!(T::decode(&encoded).unwrap(), value);
}

#[test]
fn application_network_unit_variants_are_native_semantic_types() {
    for value in [NetworkTransport::Tcp, NetworkTransport::Udp] {
        round_trip(value);
    }
    for value in [
        DnsRecordKind::A,
        DnsRecordKind::Aaaa,
        DnsRecordKind::Address,
    ] {
        round_trip(value);
    }
    round_trip(NetworkFrameProtocol::EchoV1);
    for value in [NetworkFrameDirection::Received, NetworkFrameDirection::Sent] {
        round_trip(value);
    }
    for value in [NetworkChunkShape::Datagram, NetworkChunkShape::StreamChunk] {
        round_trip(value);
    }
}
