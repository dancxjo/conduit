use super::*;

#[test]
fn native_front_door_begins_on_canonical_awake_body_world_truth() {
    let application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let body = application.build_birth.body().unwrap();
    let wake = application.build_birth.wake_value().unwrap();
    let state = application.entrance_state.as_ref().unwrap();
    let presentation = application.entrance_presentation.as_ref().unwrap();
    assert!(application.parts_open);
    assert!(application.form_editor.is_some() && application.graphical_form.is_some());
    assert_eq!(state.layer, patchbay_model::EntranceLayer::World);
    assert_eq!(state.body_id, body.body_id);
    assert_eq!(presentation.basis.body_id, body.body_id);
    assert_eq!(presentation.basis.wake_id, wake.wake_id);
    assert!(state.selected_subject.starts_with("part/"));
    let renderer = application.renderer_execution.as_ref().unwrap();
    assert_eq!(renderer.presentation.identity, presentation.identity);
    assert!(renderer.plan.fragments.iter().any(|fragment| {
        fragment.placements.iter().any(|placement| {
            placement.implementation_id.as_str() == "presentation/renderer-wayland@1"
        })
    }));
}

#[test]
fn native_front_door_revises_the_same_semantic_entrance_through_plan_and_play() {
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
    application.plan_play().unwrap();
    let planned = application.entrance_presentation.as_ref().unwrap();
    assert_eq!(planned.revision, 2);
    assert!(planned.basis.plan_id.is_some());
    assert!(planned.basis.active_play_id.is_none());
    assert_eq!(
        application
            .entrance_state
            .as_ref()
            .unwrap()
            .selected_subject,
        selected
    );

    application.play_plan().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut completed = false;
    while application.control.is_running() && std::time::Instant::now() < deadline {
        completed |= application.control.poll().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(completed);
    application.refresh_front_door().unwrap();
    let playing = application.entrance_presentation.as_ref().unwrap();
    assert_eq!(playing.revision, 3);
    assert!(playing.basis.active_play_id.is_some());
    assert_eq!(
        application
            .entrance_state
            .as_ref()
            .unwrap()
            .selected_subject,
        selected
    );
}
