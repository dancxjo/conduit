use conduit_presentation::{Presentation, PresentationRole};

use crate::{
    portable_demonstration, EntranceAction, EntranceLayer, EntranceRefusal,
    EntranceUpdateDisposition, PatchbayEntranceState,
};

fn revised(presentation: &Presentation, revision: u64) -> Presentation {
    Presentation::new(
        revision,
        presentation.basis.clone(),
        presentation.subjects.clone(),
        presentation.relationships.clone(),
        presentation.properties.clone(),
        presentation.text.clone(),
    )
    .unwrap()
}

#[test]
fn front_door_begins_in_world_on_here_and_progressively_exposes_other_layers() {
    let presentation = portable_demonstration().unwrap();
    let mut state = PatchbayEntranceState::enter(&presentation).unwrap();
    assert_eq!(state.layer, EntranceLayer::World);
    assert!(state.selected_subject.starts_with("part/"));
    assert!(state.available_actions.contains(&EntranceAction::Inspect));

    state
        .show_layer(&presentation, EntranceLayer::Intent)
        .unwrap();
    state
        .show_layer(&presentation, EntranceLayer::Realization)
        .unwrap();
    assert_eq!(state.layer, EntranceLayer::Realization);

    let candidate = presentation
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Candidate)
        .unwrap();
    state.select(&presentation, &candidate.identity).unwrap();
    assert_eq!(
        state.available_actions,
        vec![
            EntranceAction::Inspect,
            EntranceAction::Admit,
            EntranceAction::Refuse
        ]
    );
}

#[test]
fn exact_revision_updates_preserve_or_explicitly_stale_selection() {
    let presentation = portable_demonstration().unwrap();
    let mut state = PatchbayEntranceState::enter(&presentation).unwrap();
    let host = presentation
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Host)
        .unwrap()
        .identity
        .clone();
    state.select(&presentation, &host).unwrap();

    let next = revised(&presentation, presentation.revision + 1);
    assert_eq!(
        state.update(&next),
        Ok(EntranceUpdateDisposition::SelectionPreserved)
    );

    let mut subjects = next.subjects.clone();
    subjects.retain(|subject| subject.identity != host);
    let mut relationships = next.relationships.clone();
    relationships.retain(|relationship| relationship.source != host && relationship.target != host);
    let mut properties = next.properties.clone();
    properties.retain(|property| property.subject != host);
    let mut text = next.text.clone();
    text.retain(|text| text.subject != host);
    let rebooted = Presentation::new(
        next.revision + 1,
        next.basis.clone(),
        subjects,
        relationships,
        properties,
        text,
    )
    .unwrap();
    assert_eq!(
        state.update(&rebooted),
        Ok(EntranceUpdateDisposition::SelectionBecameStale)
    );
    assert!(state.selected_subject.starts_with("part/"));
    assert_eq!(state.body_id, rebooted.basis.body_id);
}

#[test]
fn stale_and_unknown_inputs_refuse_without_shadow_state() {
    let presentation = portable_demonstration().unwrap();
    let mut state = PatchbayEntranceState::enter(&presentation).unwrap();
    let selected = state.selected_subject.clone();
    assert_eq!(
        state.select(&presentation, "dom/widget/unknown"),
        Err(EntranceRefusal::UnknownSubject)
    );
    assert_eq!(state.selected_subject, selected);
    assert_eq!(
        state.update(&presentation),
        Err(EntranceRefusal::StaleRevision)
    );
}
