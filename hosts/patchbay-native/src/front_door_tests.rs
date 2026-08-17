use super::*;
use crate::gui_hit::GuiAction;

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
    assert!(ordinary.contains("OPEN [ENTER]  ·  AVAILABLE"));
    assert!(ordinary.contains("BIRTH [F4]  ·  UNAVAILABLE — No admitted authority"));
    assert!(!ordinary.contains(presentation.identity.as_str()));
    assert!(!ordinary.contains("source-document-id"));
}

#[test]
fn native_front_door_keys_invoke_current_open_and_disclose_exact_details() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let original = application.entrance_presentation.as_ref().unwrap().clone();

    assert!(application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ))
        .unwrap());
    let opened = application.entrance_presentation.as_ref().unwrap();
    assert_eq!(opened.revision, original.revision + 1);
    assert_ne!(opened.identity, original.identity);
    assert!(opened.properties.iter().any(|property| {
        property.name == "opened"
            && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
    }));
    let opened_identity = opened.identity.clone();

    assert!(application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F2,))
        .unwrap());
    assert!(application.linear_view);
    let details = application.presentation_lines().join("\n");
    assert!(details.contains(opened_identity.as_str()));
    assert!(details.contains("ACTION id="));
}

#[test]
fn unavailable_birth_key_uses_current_action_and_cannot_create_a_body() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();

    assert!(application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F4,))
        .unwrap());
    assert!(application.build_birth.body().is_none());
    let refusal = application.interaction_status.current().unwrap();
    assert_eq!(
        refusal.code,
        crate::interaction_status::InteractionStatusCode::Refused
    );
    assert_eq!(
        refusal.text,
        "BIRTH unavailable while FORM_UNAVAILABLE: Open a checked Form to begin"
    );
}

#[test]
fn focus_loss_clears_renderer_modifiers_and_transient_gestures_before_regain() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    application.modifiers = winit::keyboard::ModifiersState::CONTROL;
    application.canvas_pan_drag = Some((12.0, 34.0));

    application.handle_window_focus(false);
    assert!(application.modifiers.is_empty());
    assert!(application.canvas_pan_drag.is_none());

    application.handle_window_focus(true);
    assert!(application.modifiers.is_empty());
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
    let seed_id = application
        .zero_body_front_door
        .as_ref()
        .unwrap()
        .seed_ids()
        .into_iter()
        .next()
        .unwrap();
    let seed_subject = format!("seed/{}", seed_id.as_str());
    application
        .select_front_door_subject(&seed_subject)
        .unwrap();
    let selected = Some(seed_subject);
    application.handle_gui_action(GuiAction::OpenBack).unwrap();
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
