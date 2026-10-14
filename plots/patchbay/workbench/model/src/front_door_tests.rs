use conduit_face::{Face, FaceRole};
use patchbay_application::{
    EntranceAction, EntranceLayer, EntranceRefusal, EntranceUpdateDisposition,
    PatchbayEntranceState,
};

use crate::portable_demonstration;

fn revised(face: &Face, revision: u64) -> Face {
    Face::new_with_semantics(
        revision,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
        face.actions.clone(),
        face.disclosures.clone(),
    )
    .unwrap()
}

#[test]
fn front_door_begins_in_world_on_here_and_progressively_exposes_other_layers() {
    let face = portable_demonstration().unwrap();
    let mut state = PatchbayEntranceState::enter(&face).unwrap();
    assert_eq!(state.layer, EntranceLayer::World);
    assert!(state
        .selected_subject
        .as_deref()
        .is_some_and(|subject| subject.starts_with("part/")));
    assert!(state.available_actions.contains(&EntranceAction::Inspect));

    state.show_layer(&face, EntranceLayer::Intent).unwrap();
    state.show_layer(&face, EntranceLayer::Realization).unwrap();
    assert_eq!(state.layer, EntranceLayer::Realization);

    let candidate = face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Candidate)
        .unwrap();
    state.select(&face, &candidate.identity).unwrap();
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
    let face = portable_demonstration().unwrap();
    let mut state = PatchbayEntranceState::enter(&face).unwrap();
    let host = face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Host)
        .unwrap()
        .identity
        .clone();
    state.select(&face, &host).unwrap();

    let next = revised(&face, face.revision + 1);
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
    let mut actions = next.actions.clone();
    actions.retain(|action| action.target != host);
    let mut disclosures = next.disclosures.clone();
    disclosures.retain(|disclosure| disclosure.subject != host);
    let rebooted = Face::new_with_semantics(
        next.revision + 1,
        next.basis.clone(),
        subjects,
        relationships,
        properties,
        text,
        actions,
        disclosures,
    )
    .unwrap();
    assert_eq!(
        state.update(&rebooted),
        Ok(EntranceUpdateDisposition::SelectionBecameStale)
    );
    assert!(state
        .selected_subject
        .as_deref()
        .is_some_and(|subject| subject.starts_with("part/")));
    assert_eq!(state.body_id, rebooted.basis.body_id);
}

#[test]
fn stale_and_unknown_inputs_refuse_without_shadow_state() {
    let face = portable_demonstration().unwrap();
    let mut state = PatchbayEntranceState::enter(&face).unwrap();
    let selected = state.selected_subject.clone();
    assert_eq!(
        state.select(&face, "dom/widget/unknown"),
        Err(EntranceRefusal::UnknownSubject)
    );
    assert_eq!(state.selected_subject, selected);
    assert_eq!(state.update(&face), Err(EntranceRefusal::StaleRevision));
}

#[test]
fn clearing_selection_returns_to_quiet_world_across_revisions() {
    let face = portable_demonstration().unwrap();
    let mut state = PatchbayEntranceState::enter(&face).unwrap();
    state.show_layer(&face, EntranceLayer::Intent).unwrap();
    state.clear_selection(&face).unwrap();
    assert_eq!(state.layer, EntranceLayer::World);
    assert!(state.selected_subject.is_none());
    assert!(state.available_actions.is_empty());

    let next = revised(&face, face.revision + 1);
    assert_eq!(
        state.update(&next),
        Ok(EntranceUpdateDisposition::SelectionPreserved)
    );
    assert!(state.selected_subject.is_none());
    assert!(state.available_actions.is_empty());
}
