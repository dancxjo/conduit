use conduit_form::rust_binding::NativeRustBinding;
use conduit_net::{
    ApplicationNetworkRefusal, DnsRecordKind, DnsTtl, NetworkJoinError, NetworkTransport,
    RecordTranscriptDirection, RecordTranscriptTerminal,
};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + Eq,
{
    let encoded = value.encode().unwrap();
    assert_eq!(T::decode(&encoded).unwrap(), value);
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
