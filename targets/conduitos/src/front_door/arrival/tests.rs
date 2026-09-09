use super::*;
use alloc::borrow::ToOwned;
use conduit_core::{BootId, CheckedFormId, HostId, OfferGeneration, SourceDocumentId};
use conduit_presentation::{PresentationActionAvailability, PresentationPropertyValue};

fn door(refusal: Option<String>) -> FrontDoor {
    let mut door = FrontDoor::new(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(1),
        "profile",
        "build",
        "image",
        SourceDocumentId::from("source"),
        CheckedFormId::from("checked"),
        7,
        true,
    );
    door.open_creche("00112233-4455-6677-8899-aabbccddeeff".into(), refusal)
        .unwrap();
    door
}
fn press(door: &mut FrontDoor, usage: u8) -> ArrivalInput {
    door.accept_creche(
        KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::from_bits(0)).unwrap(),
        door.revision(),
    )
    .unwrap()
}

#[test]
fn native_draft_edits_name_and_submits_exact_checked_selection_without_birth() {
    let mut door = door(None);
    assert!(door.presentation().unwrap().basis.body_id.is_none());
    let stale = door.revision();
    press(&mut door, 21); // r replaces the selected suggested name.
    press(&mut door, 18); // o
    let event = KeyEvent::new(40, KeyTransition::Pressed, KeyModifiers::from_bits(0)).unwrap();
    assert!(matches!(
        door.accept_creche(event, stale),
        Err(Error::StaleInput)
    ));
    let ArrivalInput::Birth(selection) = press(&mut door, 40) else {
        panic!("expected birth selection")
    };
    assert_eq!(selection.friendly_name, "ro");
    assert_eq!(
        selection.workset.forms(),
        &[ResidentForm::new(
            SourceDocumentId::from("source"),
            CheckedFormId::from("checked")
        )]
    );
    assert!(door.presentation().unwrap().basis.body_id.is_none());
    assert!(door.close_creche().is_err());
}

#[test]
fn selection_and_availability_are_visible_and_refuse_empty_or_unavailable_birth() {
    let mut door = door(None);
    for _ in 0..3 {
        press(&mut door, 43);
    }
    press(&mut door, 44); // remove the only available Form
    let presentation = door.presentation().unwrap();
    assert!(
        presentation
            .properties
            .iter()
            .any(|p| p.name == "selected" && p.value == PresentationPropertyValue::Flag(false))
    );
    assert!(matches!(
        presentation
            .actions
            .iter()
            .find(|a| a.identity == "creche.birth")
            .unwrap()
            .availability,
        PresentationActionAvailability::Unavailable { .. }
    ));
    assert!(matches!(press(&mut door, 60), ArrivalInput::Changed));
    press(&mut door, 44);
    assert!(matches!(press(&mut door, 60), ArrivalInput::Birth(_)));

    let mut unavailable = self::door(Some("keyboard not offered".into()));
    assert!(matches!(press(&mut unavailable, 60), ArrivalInput::Changed));
    for _ in 0..3 {
        press(&mut unavailable, 43);
    }
    press(&mut unavailable, 44);
    assert!(!unavailable.arrival.as_ref().unwrap().draft.choices()[0].selected);
}

#[test]
fn suggestion_and_tradition_use_shared_catalog_without_creating_lifecycle() {
    let mut door = door(None);
    let before = door
        .arrival
        .as_ref()
        .unwrap()
        .draft
        .friendly_name()
        .to_owned();
    press(&mut door, 59);
    assert_ne!(door.arrival.as_ref().unwrap().draft.friendly_name(), before);
    press(&mut door, 43);
    press(&mut door, 79);
    assert_ne!(
        door.arrival.as_ref().unwrap().draft.requested_system(),
        "surprise"
    );
    assert!(door.presentation().unwrap().basis.body_id.is_none());
}

#[test]
fn unrelated_input_preserves_refusal_and_revision_exhaustion_is_atomic() {
    let mut door = door(None);
    press(&mut door, 42);
    press(&mut door, 60);
    let before = door.presentation().unwrap();
    assert!(matches!(press(&mut door, 70), ArrivalInput::Unchanged));
    assert_eq!(door.presentation().unwrap(), before);
    door.revision = u64::MAX;
    let name = door
        .arrival
        .as_ref()
        .unwrap()
        .draft
        .friendly_name()
        .to_owned();
    let event = KeyEvent::new(21, KeyTransition::Pressed, KeyModifiers::from_bits(0)).unwrap();
    assert!(matches!(
        door.accept_creche(event, u64::MAX),
        Err(Error::Presentation)
    ));
    assert_eq!(door.arrival.as_ref().unwrap().draft.friendly_name(), name);
}
