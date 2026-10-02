use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{AvailabilityState, CandidateConflict};

#[test]
fn candidate_conflict_preserves_native_bounds_and_json_shape() {
    let conflict = CandidateConflict::new("p".repeat(128), AvailabilityState::Busy).unwrap();
    assert_eq!(conflict.participant_identity().len(), 128);
    assert_eq!(*conflict.state(), AvailabilityState::Busy);

    let structured = conflict.clone().into_structured().unwrap();
    assert_eq!(
        CandidateConflict::from_structured(structured).unwrap(),
        conflict
    );
    assert_eq!(
        serde_json::to_string(&conflict).unwrap(),
        format!(
            "{{\"participant_identity\":\"{}\",\"state\":\"Busy\"}}",
            "p".repeat(128)
        )
    );

    assert!(CandidateConflict::new(String::new(), AvailabilityState::Free).is_err());
    assert!(CandidateConflict::new("p".repeat(129), AvailabilityState::Unavailable).is_err());
}
