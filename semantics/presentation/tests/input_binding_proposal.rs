//! Canonical proposal carrier only. No input mapping or authority is admitted.
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_presentation::{ActionInputBindingProposalInfo, GeneratedActionAffordance};

fn proposal(
    label: &str,
    mapping_revision: u64,
) -> Result<ActionInputBindingProposalInfo, conduit_plot::rust_binding::NativeBindingRefusal> {
    ActionInputBindingProposalInfo::new(
        true,
        "boot/fixture".into(),
        "control/fire".into(),
        label.into(),
        "face/fixture".into(),
        "host/fixture".into(),
        "plan/input-fixture".into(),
        mapping_revision,
        GeneratedActionAffordance::new("game/start".into(), 7).unwrap(),
        "show/fixture".into(),
        1,
    )
}

#[test]
fn canonical_input_binding_proposal_is_bounded_and_preserves_source_reference() {
    let value = proposal("Fire", 1).unwrap();
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        ActionInputBindingProposalInfo::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(value.offered_action().action_identity(), "game/start");
    assert_eq!(*value.offered_action().source_presentation_revision(), 7);
    assert!(proposal("", 1).is_err());
    assert!(proposal(&"x".repeat(65), 1).is_err());
    assert!(proposal("Fire", 0).is_err());
    assert!(proposal(&"x".repeat(64), u64::MAX).is_ok());
}

#[test]
fn proposal_cannot_smuggle_look_layout_or_application_callbacks() {
    let original = serde_json::to_value(proposal("Fire", 1).unwrap()).unwrap();
    for field in [
        "look",
        "component_tree",
        "callback",
        "game_state",
        "execute",
    ] {
        let mut forged = original.clone();
        forged
            .as_object_mut()
            .unwrap()
            .insert(field.into(), serde_json::json!("invented"));
        assert!(serde_json::from_value::<ActionInputBindingProposalInfo>(forged).is_err());
    }
}

#[test]
fn deserialized_proposal_must_still_satisfy_canonical_constraints() {
    let original = serde_json::to_value(proposal("Fire", 1).unwrap()).unwrap();
    for (field, bad) in [
        ("mapping_revision", serde_json::json!(0)),
        ("control_label", serde_json::json!("x".repeat(65))),
        ("version", serde_json::json!(2)),
    ] {
        let mut forged = original.clone();
        forged[field] = bad;
        match serde_json::from_value::<ActionInputBindingProposalInfo>(forged) {
            Err(_) => {}
            Ok(value) => assert!(
                ActionInputBindingProposalInfo::from_structured(value.into_structured().unwrap(),)
                    .is_err(),
                "malformed {field} crossed checked native decoding"
            ),
        }
    }
}
