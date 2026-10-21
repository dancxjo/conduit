use super::*;
use alloc::borrow::ToOwned;
use conduit_core::{BootId, CheckedPlotId, HostId, OfferGeneration, SourceDocumentId};
use conduit_face::{FaceActionAvailability, FacePropertyValue};

fn door(refusal: Option<String>) -> FrontDoor {
    let mut door = FrontDoor::new(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(1),
        "profile",
        "build",
        "image",
        SourceDocumentId::from("source"),
        CheckedPlotId::from("checked"),
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
    assert!(door.face().unwrap().basis.body_id.is_none());
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
        selection.workset.plots(),
        conduit_body::BodyWorkset::from_plots(
            crate::native_workset::inventory()
                .into_iter()
                .map(|plot| crate::native_workset::resident(plot).unwrap())
        )
        .unwrap()
        .plots()
    );
    assert!(door.face().unwrap().basis.body_id.is_none());
    assert!(door.close_creche().is_err());
}

#[test]
fn zero_body_arrival_has_no_lifecycle_plot_while_creche_inventory_is_usable() {
    let mut door = door(None);
    let journey = crate::product_journey::ProductJourney::new(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(1),
    )
    .unwrap();
    let lifecycle = journey.projection();
    assert!(lifecycle.body_id.is_none());
    assert!(lifecycle.source_document_id.is_none());
    assert!(lifecycle.checked_plot_id.is_none());
    assert!(lifecycle.expanded_plot_id.is_none());
    assert!(!door.reading.plot_open);
    door.observe_journey(lifecycle).unwrap();

    let face = door.face().unwrap();
    assert!(face.basis.body_id.is_none());
    assert!(face.basis.source_document_id.is_none());
    assert!(face.basis.checked_plot_id.is_none());
    assert_eq!(
        face.subjects
            .iter()
            .filter(|subject| subject.role == conduit_face::FaceRole::Plot)
            .count(),
        crate::native_workset::candidate_inventory().len()
    );
    assert!(matches!(press(&mut door, 60), ArrivalInput::Birth(_)));
}

#[test]
fn selection_and_availability_are_visible_and_refuse_empty_or_unavailable_birth() {
    let mut door = door(None);
    for _ in 0..4 {
        press(&mut door, 43);
    }
    press(&mut door, 44); // remove the first plot
    press(&mut door, 43);
    press(&mut door, 44); // remove the second plot
    press(&mut door, 43);
    press(&mut door, 44); // remove Tour
    press(&mut door, 43);
    press(&mut door, 44); // remove Patchbay
    let face = door.face().unwrap();
    assert!(
        face.properties
            .iter()
            .any(|p| p.name == "selected" && p.value == FacePropertyValue::Flag(false))
    );
    assert!(matches!(
        face.actions
            .iter()
            .find(|a| a.identity == "creche.birth")
            .unwrap()
            .availability,
        FaceActionAvailability::Unavailable { .. }
    ));
    assert!(matches!(press(&mut door, 60), ArrivalInput::Changed));
    press(&mut door, 44);
    assert!(matches!(press(&mut door, 60), ArrivalInput::Birth(_)));

    let mut unavailable = self::door(Some("keyboard not offered".into()));
    assert!(matches!(press(&mut unavailable, 60), ArrivalInput::Changed));
    for _ in 0..4 {
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
    assert!(door.face().unwrap().basis.body_id.is_none());
}

#[test]
fn unrelated_input_preserves_refusal_and_revision_exhaustion_is_atomic() {
    let mut door = door(None);
    press(&mut door, 42);
    press(&mut door, 60);
    let before = door.face().unwrap();
    assert!(matches!(press(&mut door, 70), ArrivalInput::Unchanged));
    assert_eq!(door.face().unwrap(), before);
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
        Err(Error::Face)
    ));
    assert_eq!(door.arrival.as_ref().unwrap().draft.friendly_name(), name);
}

#[test]
fn shared_search_filters_visible_controls_without_losing_included_plots() {
    let mut door = door(None);
    for _ in 0..3 {
        press(&mut door, 43);
    }
    for usage in [16, 8, 16, 18, 21, 28] {
        press(&mut door, usage);
    } // memory
    let actions = door.arrival.as_ref().unwrap().controls();
    assert!(actions.iter().any(|action| action == "creche.plot.1"));
    assert!(!actions.iter().any(|action| action == "creche.plot.0"));
    let scene = door.scene(&super::super::tests::Sink);
    assert!(scene.is_ok(), "{scene:?}");
    press(&mut door, 29); // mz: no match
    assert!(door.scene(&super::super::tests::Sink).is_ok());
    assert!(
        door.arrival
            .as_ref()
            .unwrap()
            .controls()
            .iter()
            .all(|action| !action.starts_with("creche.plot."))
    );
    let ArrivalInput::Birth(selection) = press(&mut door, 60) else {
        panic!("selected plots survive filtering")
    };
    assert_eq!(selection.workset.len(), 4);
}

#[test]
fn five_candidate_scene_keeps_choices_status_and_birth_visually_separate() {
    let mut door = door(None);
    assert!(door.scene(&super::super::tests::Sink).is_ok());
    press(&mut door, 42);
    press(&mut door, 60);
    let scene = door.scene(&super::super::tests::Sink).unwrap();
    assert!(scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
    let choices: alloc::vec::Vec<_> = scene
        .commands()
        .iter()
        .filter(|command| {
            command.payload().starts_with("●  ") || command.payload().starts_with("○  ")
        })
        .collect();
    assert_eq!(choices.len(), 5);
    let unavailable = choices.last().unwrap().payload();
    assert!(unavailable.starts_with("○  ") && unavailable.contains("Hello, world."));
    assert!(unavailable.contains("/ unavailable"));
    assert!(choices.windows(2).all(|rows| {
        rows[1].bounds.y - rows[0].bounds.y
            >= crate::display::profile::line_height(conduit_face::GraphicsTextRole::Body) as i16
    }));
    let selected = scene
        .commands()
        .iter()
        .find(|command| command.payload().starts_with("Selected:"))
        .unwrap();
    assert!(selected.payload().contains("Review Birth choices"));
    let birth = scene
        .commands()
        .iter()
        .find(|command| command.payload().contains("Enter / F3"))
        .unwrap();
    assert!(selected.bounds.y > choices.last().unwrap().bounds.y);
    assert!(birth.bounds.y > selected.bounds.y);
    crate::display::render_scene(&mut super::super::tests::Sink, &scene).unwrap();
}

#[test]
fn native_plot_availability_is_reviewed_independently() {
    let mut door = door(None);
    door.arrival = None;
    door.open_creche_reviewed(
        "00112233-4455-6677-8899-aabbccddeeff".into(),
        [Some("Canvas unavailable".into()), None, None, None, None],
    )
    .unwrap();
    let ArrivalInput::Birth(selection) = press(&mut door, 60) else {
        panic!("the available plot can be included")
    };
    let mut expected = [
        crate::native_workset::resident(crate::native_workset::NativePlot::Tour).unwrap(),
        crate::native_workset::resident(crate::native_workset::NativePlot::MemoryLantern).unwrap(),
        crate::native_workset::resident(crate::native_workset::NativePlot::Patchbay).unwrap(),
    ];
    // Worksets are canonical identity sets, independent of menu display order.
    expected.sort();
    assert_eq!(selection.workset.plots(), &expected);
}

#[test]
fn altgr_dead_keys_survive_release_and_edit_the_name_before_birth() {
    let mut door = door(None);
    for _ in 0..2 {
        let dead = KeyEvent::new(52, KeyTransition::Pressed, KeyModifiers::RIGHT_ALT).unwrap();
        assert!(matches!(
            door.accept_creche(dead, door.revision()).unwrap(),
            ArrivalInput::Unchanged
        ));
        let release = KeyEvent::new(52, KeyTransition::Released, KeyModifiers::NONE).unwrap();
        door.accept_creche(release, door.revision()).unwrap();
        assert!(matches!(press(&mut door, 8), ArrivalInput::Changed));
    }
    let ArrivalInput::Birth(selection) = press(&mut door, 40) else {
        panic!("a later Enter births")
    };
    assert_eq!(selection.friendly_name, "éé");
}

#[test]
fn fifth_visible_source_candidate_stays_unavailable_and_unselected() {
    let mut door = door(None);
    let draft = &mut door.arrival.as_mut().unwrap().draft;
    assert_eq!(draft.choices().len(), 5);
    assert_eq!(
        draft
            .choices()
            .iter()
            .filter(|choice| choice.selected)
            .count(),
        4
    );
    let candidate = &draft.choices()[4];
    assert_eq!(
        candidate.plot,
        crate::native_workset::resident(crate::native_workset::NativePlot::Hello).unwrap()
    );
    assert_eq!(
        candidate.refusal.as_deref(),
        Some(crate::native_workset::WorksetRefusal::UnknownPlot.as_str())
    );
    assert!(!candidate.selected);
    let revision = draft.revision();
    assert_eq!(
        draft.select(revision, 4, true),
        Err(BirthDraftRefusal::UnavailablePlot)
    );
    assert_eq!(draft.revision(), revision);
    assert!(!draft.choices()[4].selected);
    let face = door.face().unwrap();
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == "creche.plot.4")
        .unwrap();
    assert!(matches!(
        action.availability,
        FaceActionAvailability::Unavailable { .. }
    ));
    assert!(face.basis.body_id.is_none());
}
