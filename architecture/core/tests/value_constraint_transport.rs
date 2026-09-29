use conduit_core::{
    kind_id, CheckedTextPattern, CheckedValueContract, TextPatternState, TextPatternTransition,
    ValueConstraint, ValueConstraintRefusal, TEXT_INFO_ID,
};

fn lowercase_z_pattern() -> CheckedTextPattern {
    CheckedTextPattern::new(
        vec![
            TextPatternState {
                accepting: false,
                transitions: vec![TextPatternTransition {
                    first_scalar: 'z' as u32,
                    last_scalar: 'z' as u32,
                    target_state: 1,
                }],
            },
            TextPatternState {
                accepting: true,
                transitions: vec![],
            },
        ],
        0,
        8,
        8,
    )
    .unwrap()
}

fn exact_contract() -> CheckedValueContract {
    CheckedValueContract::new(
        kind_id(TEXT_INFO_ID),
        8,
        vec![
            ValueConstraint::CanonicalMembership {
                members: vec![b"admin".to_vec(), b"root".to_vec()],
                negated: true,
            },
            ValueConstraint::TextPattern {
                pattern: lowercase_z_pattern(),
                anchored_start: false,
                anchored_end: true,
                negated: true,
            },
        ],
    )
    .unwrap()
}

#[test]
fn exact_negated_constraint_profile_round_trips_across_portable_and_json_carriers() {
    let contract = exact_contract();

    let portable = postcard::to_allocvec(&contract).unwrap();
    let portable_round_trip: CheckedValueContract = postcard::from_bytes(&portable).unwrap();
    assert_eq!(portable_round_trip, contract);
    assert_eq!(
        portable_round_trip.identity_bytes(),
        contract.identity_bytes()
    );

    let json = serde_json::to_vec(&contract).unwrap();
    let json_round_trip: CheckedValueContract = serde_json::from_slice(&json).unwrap();
    assert_eq!(json_round_trip, contract);
    assert_eq!(json_round_trip.identity_bytes(), contract.identity_bytes());

    assert_eq!(contract.validate(b"guest"), Ok(()));
    assert_eq!(
        contract.validate(b"root"),
        Err(ValueConstraintRefusal::Membership)
    );
    assert_eq!(
        contract.validate(b"endsz"),
        Err(ValueConstraintRefusal::TextPattern)
    );
}
