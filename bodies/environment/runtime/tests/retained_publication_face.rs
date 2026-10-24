#![cfg(feature = "face-projection")]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{PreparedStructuredComposer, Quantity, Unit};
use conduit_environment_face::PublicationRefusal;
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedSourceFrameDecoder},
    publication::{PublicationChannel as Channel, PublicationContract, PublicationSnapshot},
    retained_publication_face::{RetainedPublicationFace, RetainedPublicationRefusal as Refusal},
};
use conduit_face::{Face, FaceBasis, FacePropertyValue};
fn basis() -> FaceBasis {
    // CPU-only producer scope. No Body/Wake is invented for this fixture.
    FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some("source/fixture".into()),
        checked_plot_id: Some("checked/fixture".into()),
        expanded_plot_id: Some("expanded/fixture".into()),
        plan_id: Some("plan/fixture".into()),
        active_play_id: Some("play/fixture".into()),
        sign_ids: vec![],
    }
}
fn snapshot(channel: Channel, sequence: u64, canonical: &[u8]) -> PublicationSnapshot<'_> {
    PublicationSnapshot {
        channel,
        sequence,
        canonical,
    }
}
fn value<'a>(face: &'a Face, name: &str) -> &'a FacePropertyValue {
    &face
        .properties
        .iter()
        .find(|p| p.name == name)
        .unwrap()
        .value
}
#[test]
fn source_publications_become_retained_quantity_faces_without_play_allocations() {
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    let reading = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap().to_vec();
    let failure = PublicationContract::prepare(Channel::ProviderFailure).unwrap();
    let mut empty = PreparedStructuredComposer::new(failure.value_type(), 1024).unwrap();
    let failure = empty.leaf(&[]).unwrap();
    let expected_basis = basis();
    let mut owner = RetainedPublicationFace::prepare(expected_basis.clone(), 3, 30000).unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        owner
            .publish(snapshot(Channel::Reading, 1, &reading), 1000)
            .unwrap();
        assert_eq!(&owner.current().basis, &expected_basis);
        let FacePropertyValue::TypedValue { bytes, .. } = value(owner.current(), "temperature")
        else {
            panic!()
        };
        assert!(Quantity::decode(bytes)
            .unwrap()
            .compare(Quantity::from_decimal(202, -1, Unit::Celsius).unwrap())
            .unwrap()
            .is_eq());
        let FacePropertyValue::TypedValue { bytes, .. } =
            value(owner.current(), "relative-humidity")
        else {
            panic!()
        };
        assert_eq!(
            Quantity::decode(bytes).unwrap(),
            Quantity::new(53, Unit::Percent)
        );
        owner
            .publish(snapshot(Channel::ProviderFailure, 2, failure), 1001)
            .unwrap();
        assert!(
            matches!(value(owner.current(), "sensor-status"), FacePropertyValue::Text(text) if text == "unavailable")
        );
        assert!(matches!(
            value(owner.current(), "temperature"),
            FacePropertyValue::TypedValue { .. }
        ));
        assert!(matches!(
            value(owner.at(32000).unwrap(), "fresh"),
            FacePropertyValue::Flag(false)
        ));
        assert_eq!(owner.last_source_sequence(), 2);
        owner
            .publish(snapshot(Channel::Reading, 3, &reading), 33000)
            .unwrap();
        assert!(matches!(
            value(owner.current(), "fresh"),
            FacePropertyValue::Flag(true)
        ));
        assert_eq!(owner.current().revision, 4);
        owner.current().validate().unwrap();
        assert!(matches!(
            owner.publish(snapshot(Channel::Reading, 4, &reading), 34000),
            Err(Refusal::Publication(PublicationRefusal::Exhausted))
        ));
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(owner.last_source_sequence(), 3);
}
#[test]
fn refused_sequences_types_and_clock_preserve_the_exact_previous_face() {
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    let reading = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
    let mut owner = RetainedPublicationFace::prepare(basis(), 2, 30000).unwrap();
    owner
        .publish(snapshot(Channel::Reading, 1, reading), 1000)
        .unwrap();
    let before = owner.current().clone();
    for sequence in [0, 1, 3, u64::MAX] {
        assert!(
            matches!(owner.publish(snapshot(Channel::Reading, sequence, reading), 1001), Err(Refusal::Sequence { expected: 2, actual }) if actual == sequence)
        );
        assert_eq!(owner.current(), &before);
    }
    for (channel, bytes) in [
        (Channel::ProviderFailure, reading),
        (Channel::Reading, &[][..]),
    ] {
        assert!(matches!(
            owner.publish(snapshot(channel, 2, bytes), 1001),
            Err(Refusal::Structured(_))
        ));
        assert_eq!(owner.current(), &before);
    }
    assert_eq!(
        owner.publish(snapshot(Channel::Reading, 2, reading), 999),
        Err(Refusal::Publication(PublicationRefusal::ClockReversed))
    );
    assert_eq!(owner.current(), &before);
    assert_eq!(owner.last_source_sequence(), 1);
    owner
        .publish(snapshot(Channel::Reading, 2, reading), 1001)
        .unwrap();
}
#[test]
fn a_live_publication_owner_requires_complete_execution_provenance() {
    let mut missing = basis();
    missing.active_play_id = None;
    assert!(matches!(
        RetainedPublicationFace::prepare(missing, 1, 30000),
        Err(Refusal::MissingExecutionBasis)
    ));
    let mut missing = basis();
    // Untrusted metadata cannot substitute for coherent execution provenance.
    missing.body_id = Some(serde_json::from_str(r#""untrusted/body""#).unwrap());
    missing.wake_id = None;
    assert!(matches!(
        RetainedPublicationFace::prepare(missing, 1, 30000),
        Err(Refusal::MissingExecutionBasis)
    ));
}
