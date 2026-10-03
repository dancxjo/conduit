use conduit_plot::rust_binding::NativeRustBinding;
use conduit_presentation::FaceUtteranceProvenance;

fn assert_round_trip(value: FaceUtteranceProvenance) {
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        FaceUtteranceProvenance::from_structured(structured).unwrap(),
        value
    );
}

#[test]
fn face_utterance_provenance_round_trips_every_payload_shape() {
    for value in [
        FaceUtteranceProvenance::subject("subject".into()).unwrap(),
        FaceUtteranceProvenance::relationship(3).unwrap(),
        FaceUtteranceProvenance::property(4).unwrap(),
        FaceUtteranceProvenance::composition("composition".into()).unwrap(),
        FaceUtteranceProvenance::text(5).unwrap(),
        FaceUtteranceProvenance::action("action".into()).unwrap(),
        FaceUtteranceProvenance::action_argument("action".into(), "argument".into()).unwrap(),
        FaceUtteranceProvenance::disclosure(6).unwrap(),
        FaceUtteranceProvenance::temporal_reference(7).unwrap(),
        FaceUtteranceProvenance::temporal_fact(8).unwrap(),
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn face_utterance_provenance_enforces_exact_identity_boundaries() {
    assert!(FaceUtteranceProvenance::subject("x".repeat(256)).is_ok());
    assert!(FaceUtteranceProvenance::subject(String::new()).is_err());
    assert!(FaceUtteranceProvenance::subject("x".repeat(257)).is_err());
    assert!(FaceUtteranceProvenance::action_argument("action".into(), String::new()).is_err());
    assert!(FaceUtteranceProvenance::action_argument("x".repeat(257), "argument".into()).is_err());
}
