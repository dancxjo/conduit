//! Source identity conformance; acknowledgement is a simulated Mask fixture.
use super::*;
use alloc::vec::Vec;
use conduit_face::{FacePropertyValue, render_linear_face};

fn inventory() -> Vec<BirthPlotChoice> {
    ["alpha", "beta", "gamma"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| BirthPlotChoice {
            title: "東京 · same label".into(),
            search_text: name.into(),
            plot: ResidentPlot::new(
                format!("source/{name}").into(),
                format!("checked/{name}").into(),
            ),
            refusal: (index == 2).then(|| "unsupported test Back".into()),
            selected: index == 0,
        })
        .collect()
}

fn from_inventory(choices: Vec<BirthPlotChoice>) -> BirthDraft {
    BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap()
}

fn option<'a>(face: &'a Face, checked: &str) -> &'a conduit_face::FaceSubject {
    let property = face
        .properties
        .iter()
        .find(|p| {
            p.name == "checked-plot-id" && p.value == FacePropertyValue::Identity(checked.into())
        })
        .expect("exact source identity survives projection");
    face.subjects
        .iter()
        .find(|s| s.identity == property.subject)
        .unwrap()
}

#[test]
fn duplicate_labels_filtering_and_inventory_reorder_preserve_source_item_identity() {
    let choices = inventory();
    let mut draft = from_inventory(choices.clone());
    let face = draft.face(&basis()).unwrap();
    let alpha = option(&face, "checked/alpha");
    let beta = option(&face, "checked/beta");
    assert_eq!(alpha.name, beta.name);
    assert_ne!(alpha.identity, beta.identity);
    let selection = draft.selection(draft.revision()).unwrap();
    draft.search_plots(draft.revision(), "beta").unwrap();
    let filtered = draft.face(&basis()).unwrap();
    assert_eq!(option(&filtered, "checked/beta").identity, beta.identity);
    assert!(
        !filtered
            .subjects
            .iter()
            .any(|s| s.identity == alpha.identity)
    );
    assert_eq!(
        draft.selection(draft.revision()).unwrap().workset,
        selection.workset
    );
    let lines = render_linear_face(&filtered).unwrap().lines.join("\n");
    assert!(lines.contains("checked/beta"));
    assert!(lines.contains("東京 · same label"));

    let mut reordered = choices;
    reordered.reverse();
    let reordered = from_inventory(reordered).face(&basis()).unwrap();
    for checked in ["checked/alpha", "checked/beta", "checked/gamma"] {
        assert_eq!(
            option(&reordered, checked).identity,
            option(&face, checked).identity
        );
    }
}

#[test]
fn filtered_choice_uses_current_source_action_and_rejects_stale_or_retargeted_returns() {
    let mut draft = from_inventory(inventory());
    let (old_show, old_input) = input(&draft, "creche.plot.1", Some(("value/bool", &[1]))).unwrap();
    draft.search_plots(draft.revision(), "beta").unwrap();
    let revision = draft.revision();
    let selection = draft.selection(revision).unwrap();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &old_show, &old_input),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    assert_eq!(draft.revision(), revision);
    assert_eq!(draft.selection(revision).unwrap(), selection);

    let face = draft.face(&basis()).unwrap();
    let beta = option(&face, "checked/beta");
    let action = face
        .actions
        .iter()
        .find(|a| a.target == beta.identity)
        .unwrap();
    // Filtered position zero must still select original source item one.
    assert_eq!(action.identity, "creche.plot.1");
    let (show, mut fresh) = input(&draft, &action.identity, Some(("value/bool", &[1]))).unwrap();
    fresh.target = "forged/target".into();
    assert_eq!(
        draft.apply_face_interaction(&basis(), &show, &fresh),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::WrongTarget
        ))
    );
    assert_eq!(draft.revision(), revision);
    assert_eq!(draft.selection(revision).unwrap(), selection);
    assert_eq!(
        apply(&mut draft, "creche.plot.1", Some(("value/bool", &[1]))),
        Ok(BirthActionOutcome::Changed)
    );
    assert!(draft.choices()[0].selected);
    assert!(draft.choices()[1].selected);
    assert!(!draft.choices()[2].selected);
}

#[test]
fn empty_filter_and_disabled_choice_preserve_selection_without_offering_authority() {
    let mut draft = from_inventory(inventory());
    let selection = draft.selection(draft.revision()).unwrap();
    draft.search_plots(draft.revision(), "gamma").unwrap();
    let face = draft.face(&basis()).unwrap();
    let item = option(&face, "checked/gamma");
    let action = face
        .actions
        .iter()
        .find(|a| a.target == item.identity)
        .unwrap();
    assert!(!action.availability.is_available());
    assert_eq!(
        input(&draft, "creche.plot.2", Some(("value/bool", &[1]))),
        Err(FaceInteractionRefusal::UnavailableAction)
    );
    draft.search_plots(draft.revision(), "missing").unwrap();
    let empty = draft.face(&basis()).unwrap();
    assert!(
        !empty
            .actions
            .iter()
            .any(|a| a.identity.starts_with("creche.plot."))
    );
    assert_eq!(
        draft.selection(draft.revision()).unwrap().workset,
        selection.workset
    );
}

#[test]
fn filtering_does_not_rename_the_second_source_subject_to_the_first() {
    let mut draft = from_inventory(inventory());
    let face = draft.face(&basis()).unwrap();
    let target = |face: &Face| {
        face.actions
            .iter()
            .find(|a| a.identity == "creche.plot.1")
            .unwrap()
            .target
            .clone()
    };
    let original = target(&face);
    draft.search_plots(draft.revision(), "beta").unwrap();
    assert_eq!(target(&draft.face(&basis()).unwrap()), original);
}

#[test]
fn longest_encounter_retains_bounded_identifiers_and_inventory_replacement_invalidates_show() {
    let mut longest = basis();
    longest.encounter_id = "e".repeat(128);
    let face = from_inventory(inventory()).face(&longest).unwrap();
    assert!(
        face.subjects
            .iter()
            .all(|s| s.identity.len() <= conduit_face::MAX_FACE_ID_BYTES)
    );
    let original = from_inventory(inventory());
    let (show, action) = input(&original, "creche.plot.1", Some(("value/bool", &[1]))).unwrap();
    let mut choices = inventory();
    choices.reverse();
    let mut replacement = from_inventory(choices);
    let revision = replacement.revision();
    let selection = replacement.selection(revision).unwrap();
    assert_eq!(
        replacement.apply_face_interaction(&basis(), &show, &action),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    );
    assert_eq!(replacement.revision(), revision);
    assert_eq!(replacement.selection(revision).unwrap(), selection);
}
