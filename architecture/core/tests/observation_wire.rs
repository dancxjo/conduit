//! Persisted observation tags remain stable when shared evidence is extended.
use conduit_core::{ObservationKind, PortId, RoutedContentEvidence};

#[test]
fn preexisting_observation_tags_keep_their_postcard_encoding() {
    for (kind, wire) in [
        (ObservationKind::HostStarted, vec![0]),
        (ObservationKind::AdvertisementPublished, vec![1]),
        (ObservationKind::PlanFragmentReceived, vec![2]),
        (ObservationKind::PlacementPrepared, vec![3]),
        (ObservationKind::PlanPlayStarted, vec![4]),
        (ObservationKind::PlacementCompleted, vec![9]),
        (ObservationKind::PlanCompleted, vec![10]),
        (ObservationKind::Cancelled, vec![15]),
        (ObservationKind::Released, vec![16]),
        (ObservationKind::SignGap { dropped: 7 }, vec![17, 7]),
    ] {
        assert_eq!(postcard::to_allocvec(&kind).unwrap(), wire);
        assert_eq!(postcard::from_bytes::<ObservationKind>(&wire).unwrap(), kind);
    }
}

#[test]
fn shared_routing_evidence_roundtrips_without_becoming_a_value_payload() {
    let kind = ObservationKind::RoutedContent {
        evidence: RoutedContentEvidence {
            native_event_sequence: 41,
            output_port: PortId::from("result"),
            byte_length: 128,
            digest_domain: "conduit/routed-value-content@1".into(),
            digest_scheme: "core/semantic-sha256@1".into(),
            digest: [19; 32],
        },
    };
    let wire = postcard::to_allocvec(&kind).unwrap();
    assert_eq!(wire[0], 18);
    assert_eq!(postcard::from_bytes::<ObservationKind>(&wire).unwrap(), kind);
    let json = serde_json::to_value(&kind).unwrap();
    assert!(json.get("RoutedContent").is_some());
    assert!(json.get("ValueProduced").is_none());
    assert_eq!(serde_json::from_value::<ObservationKind>(json).unwrap(), kind);
}
