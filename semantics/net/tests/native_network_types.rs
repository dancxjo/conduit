use conduit_form::rust_binding::NativeRustBinding;
use conduit_net::{
    ApplicationNetworkRefusal, DnsQuery, DnsRecordKind, DnsTtl, NetworkAttachmentId,
    NetworkJoinError, NetworkTransport, RecordTranscriptDirection, RecordTranscriptTerminal,
};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + Eq,
{
    let encoded = value.encode().unwrap();
    assert_eq!(T::decode(&encoded).unwrap(), value);
}

#[test]
fn network_attachment_identity_is_native_bounded_text_with_stable_json() {
    let identity = NetworkAttachmentId::new("attachment/network-1".into()).unwrap();
    let structured = identity.clone().into_structured().unwrap();
    assert_eq!(
        structured.value_type(),
        &NetworkAttachmentId::semantic_type().unwrap()
    );
    assert_eq!(
        NetworkAttachmentId::from_structured(structured).unwrap(),
        identity
    );
    assert_eq!(
        serde_json::to_string(&identity).unwrap(),
        "\"attachment/network-1\""
    );
    assert_eq!(
        serde_json::from_str::<NetworkAttachmentId>("\"attachment/network-1\"").unwrap(),
        identity
    );
    let mut postcard_golden = vec![20];
    postcard_golden.extend_from_slice(b"attachment/network-1");
    assert_eq!(postcard::to_allocvec(&identity).unwrap(), postcard_golden);
    assert_eq!(
        postcard::from_bytes::<NetworkAttachmentId>(&postcard_golden).unwrap(),
        identity
    );

    assert!(NetworkAttachmentId::new(String::new()).is_err());
    assert!(NetworkAttachmentId::new("x".repeat(96)).is_ok());
    assert!(NetworkAttachmentId::new("x".repeat(97)).is_err());
    assert!(
        serde_json::from_str::<NetworkAttachmentId>(&format!("\"{}\"", "x".repeat(97))).is_err()
    );
}

fn round_trip_owned<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + Eq,
{
    let encoded = value.clone().encode().unwrap();
    assert_eq!(T::decode(&encoded).unwrap(), value);
}

#[test]
fn application_network_vocabularies_are_native_semantic_types() {
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
    for value in [
        ApplicationNetworkRefusal::EmptyName,
        ApplicationNetworkRefusal::NameTooLarge,
        ApplicationNetworkRefusal::InvalidPort,
        ApplicationNetworkRefusal::TooManyCandidates,
        ApplicationNetworkRefusal::CandidateTransportMismatch,
    ] {
        round_trip(value);
    }
    for value in [
        NetworkJoinError::MalformedRequest,
        NetworkJoinError::CredentialTooLarge,
        NetworkJoinError::StaleHostBoot,
        NetworkJoinError::Unsupported,
        NetworkJoinError::MissingResource,
        NetworkJoinError::ResourceMismatch,
        NetworkJoinError::MissingAuthority,
        NetworkJoinError::StaleAuthority,
        NetworkJoinError::AuthorityMismatch,
        NetworkJoinError::InvalidAttachment,
        NetworkJoinError::OutputTooSmall,
    ] {
        round_trip(value);
    }
    for value in [
        RecordTranscriptDirection::Sent,
        RecordTranscriptDirection::Received,
    ] {
        round_trip(value);
    }

    round_trip_owned(DnsTtl::known_seconds(30).unwrap());
    round_trip_owned(DnsTtl::Unavailable);
    for value in [
        RecordTranscriptTerminal::Completed,
        RecordTranscriptTerminal::Cancelled,
        RecordTranscriptTerminal::TransportUnavailable,
        RecordTranscriptTerminal::Disconnected,
        RecordTranscriptTerminal::TimedOut,
        RecordTranscriptTerminal::refused(17).unwrap(),
        RecordTranscriptTerminal::failed(23).unwrap(),
    ] {
        round_trip_owned(value);
    }
}

#[test]
fn dns_query_round_trips_and_owns_its_exact_bounds() {
    let query = DnsQuery::new(
        "fixture.local".into(),
        443,
        DnsRecordKind::Address,
        NetworkTransport::Tcp,
    )
    .unwrap();
    let structured = query.clone().into_structured().unwrap();
    assert_eq!(structured.value_type(), &DnsQuery::semantic_type().unwrap());
    assert_eq!(DnsQuery::from_structured(structured).unwrap(), query);

    assert!(DnsQuery::new(
        String::new(),
        443,
        DnsRecordKind::Address,
        NetworkTransport::Tcp,
    )
    .is_err());
    assert!(DnsQuery::new(
        "x".repeat(254),
        443,
        DnsRecordKind::Address,
        NetworkTransport::Tcp,
    )
    .is_err());
    assert!(DnsQuery::new(
        "fixture.local".into(),
        0,
        DnsRecordKind::Address,
        NetworkTransport::Tcp,
    )
    .is_err());
}
