use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{ApplicationAction, ApplicationEventKind};

#[test]
fn application_action_round_trips_at_the_exact_identity_bound() {
    let action = ApplicationAction::new(ApplicationEventKind::Submit, "a".repeat(48)).unwrap();
    let structured = action.clone().into_structured().unwrap();
    assert_eq!(
        ApplicationAction::from_structured(structured).unwrap(),
        action
    );

    assert!(ApplicationAction::new(ApplicationEventKind::Submit, String::new()).is_err());
    assert!(ApplicationAction::new(ApplicationEventKind::Submit, "a".repeat(49)).is_err());
}

#[test]
fn application_action_has_no_handwritten_duplicate() {
    assert!(!include_str!("../src/application_view.rs").contains("pub struct ApplicationAction"));
}
