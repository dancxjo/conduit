use super::*;

#[test]
fn native_front_door_begins_on_truthful_zero_body_world_state() {
    let application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let state = application.entrance_state.as_ref().unwrap();
    let presentation = application.entrance_presentation.as_ref().unwrap();
    assert!(!application.parts_open);
    assert!(application.form_editor.is_none());
    assert!(application.build_birth.body().is_none());
    assert_eq!(state.layer, patchbay_model::EntranceLayer::World);
    assert!(state.body_id.is_none());
    assert!(presentation.basis.body_id.is_none());
    assert!(presentation.basis.wake_id.is_none());
    assert!(presentation.basis.seed_id.is_none());
    assert!(state
        .selected_subject
        .as_deref()
        .is_some_and(|subject| subject.starts_with("host/")));
    assert!(presentation
        .subjects
        .iter()
        .any(|subject| subject.role == conduit_presentation::PresentationRole::Seed));
    let renderer = application.renderer_execution.as_ref().unwrap();
    assert_eq!(renderer.presentation.identity, presentation.identity);
    assert!(renderer.plan.fragments.iter().any(|fragment| {
        fragment.placements.iter().any(|placement| {
            placement.implementation_id.as_str() == "presentation/renderer-wayland@1"
        })
    }));
    let ordinary = application.presentation_lines().join("\n");
    assert!(ordinary.contains("Seed  Patchbay entrance specimen"));
    assert!(ordinary.contains("OPEN  ·  AVAILABLE"));
    assert!(ordinary.contains("BE BORN  ·  UNAVAILABLE — No admitted authority"));
    assert!(!ordinary.contains(presentation.identity.as_str()));
    assert!(!ordinary.contains("source-document-id"));
}

#[test]
fn native_front_door_discloses_exact_truth_only_in_details() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let identity = application
        .entrance_presentation
        .as_ref()
        .unwrap()
        .identity
        .as_str()
        .to_owned();

    application.linear_view = true;
    let details = application.presentation_lines().join("\n");
    assert!(details.contains(&identity));
    assert!(details.contains("ACTION id="));
    assert!(details.contains("authority/not-admitted"));
}

#[test]
fn native_open_seed_revision_remains_unbodied_and_preserves_selection() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let selected = application
        .entrance_state
        .as_ref()
        .unwrap()
        .selected_subject
        .clone();
    let session = application.zero_body_front_door.as_mut().unwrap();
    let seed_id = session.seed_ids().into_iter().next().unwrap();
    session.open_seed(&seed_id, session.revision()).unwrap();
    application.refresh_front_door().unwrap();
    let opened = application.entrance_presentation.as_ref().unwrap();
    assert_eq!(opened.revision, 2);
    assert!(opened.basis.body_id.is_none());
    assert!(opened.basis.seed_id.is_none());
    assert_eq!(
        application
            .entrance_state
            .as_ref()
            .unwrap()
            .selected_subject,
        selected
    );
    assert!(opened.properties.iter().any(|property| {
        property.name == "opened"
            && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
    }));
}
