use super::*;
use crate::gui_hit::GuiAction;

#[test]
fn native_front_door_begins_on_truthful_zero_body_world_state() {
    let application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let entrance = application.entrance.as_ref().unwrap();
    let state = &entrance.state;
    let presentation = &entrance.presentation;
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
    let navigation = &entrance.navigation;
    assert_eq!(
        navigation.cursor.place,
        conduit_presentation::PresentationPlace::Entrance
    );
    assert_eq!(
        navigation.cursor.aspect,
        conduit_presentation::PresentationAspect::Structure
    );
    let renderer = application.renderer_execution.as_ref().unwrap();
    assert_eq!(renderer.presentation.identity, presentation.identity);
    assert!(renderer.plan.fragments.iter().any(|fragment| {
        fragment.placements.iter().any(|placement| {
            placement.implementation_id.as_str() == "presentation/renderer-wayland@1"
        })
    }));
    let ordinary = application.presentation_lines().join("\n");
    assert!(ordinary.contains("PLACE Entrance  ·  ASPECT Structure"));
    assert!(ordinary.contains(
        "CTRL-TAB PLACE  ·  CTRL-PAGEUP/PAGEDOWN ASPECT  ·  F2 EXACT  ·  SHIFT-F3 CHOOSE / F3 FOLLOW"
    ));
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
    let original = application.entrance.as_ref().unwrap().presentation.clone();

    assert!(application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ))
        .unwrap());
    let opened = &application.entrance.as_ref().unwrap().presentation;
    assert_eq!(opened.revision, original.revision + 1);
    assert_ne!(opened.identity, original.identity);
    assert!(opened.properties.iter().any(|property| {
        property.name == "opened"
            && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
    }));
    let opened_identity = opened.identity.clone();
    assert_eq!(
        application
            .entrance
            .as_ref()
            .unwrap()
            .navigation
            .cursor
            .place,
        conduit_presentation::PresentationPlace::Program
    );

    assert!(application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F2,))
        .unwrap());
    assert!(application.linear_view);
    let details = application.presentation_lines().join("\n");
    assert!(details.contains("CURSOR place=Program aspect=Structure"));
    assert!(details.contains("depth=Exact"));
    assert!(details.contains(opened_identity.as_str()));
    assert!(details.contains("ACTION id="));

    application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F2))
        .unwrap();
    assert!(!application.linear_view);
    assert_eq!(
        application
            .entrance
            .as_ref()
            .unwrap()
            .navigation
            .cursor
            .depth,
        conduit_presentation::PresentationDepth::Primary
    );
}

#[test]
fn native_place_and_aspect_keys_mutate_only_the_portable_cursor() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ))
        .unwrap();
    let presentation = application.entrance.as_ref().unwrap().presentation.clone();
    let before_basis = presentation.basis.clone();

    assert!(!application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab))
        .unwrap());
    application.modifiers = winit::keyboard::ModifiersState::CONTROL;
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab))
        .unwrap();
    assert_eq!(
        application
            .entrance
            .as_ref()
            .unwrap()
            .navigation
            .cursor
            .place,
        conduit_presentation::PresentationPlace::Entrance
    );
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::PageDown,
        ))
        .unwrap();
    assert_eq!(
        application
            .entrance
            .as_ref()
            .unwrap()
            .navigation
            .cursor
            .aspect,
        conduit_presentation::PresentationAspect::Signs
    );
    assert_eq!(
        &application.entrance.as_ref().unwrap().presentation,
        &presentation
    );
    assert_eq!(
        application.entrance.as_ref().unwrap().presentation.basis,
        before_basis
    );
}

#[test]
fn native_follow_crosses_exact_documentary_correlation_and_returns() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let presentation = patchbay_model::portable_demonstration().unwrap();
    let navigation =
        patchbay_model::PatchbayNavigationProjection::for_embodied(&presentation).unwrap();
    let state = patchbay_model::PatchbayEntranceState::enter(&presentation).unwrap();
    application.entrance = Some(
        crate::front_door::NativeFrontDoorPresentation::new(
            state,
            presentation.clone(),
            navigation,
        )
        .unwrap(),
    );
    application
        .navigate_front_door(conduit_presentation::NavigationOperation::Show(
            conduit_presentation::PresentationAspect::Plan,
        ))
        .unwrap();
    let forward = application
        .entrance
        .as_ref()
        .unwrap()
        .navigation
        .navigation
        .follows[0]
        .clone();
    application
        .navigate_front_door(conduit_presentation::NavigationOperation::Focus(
            forward.source_subject.clone(),
        ))
        .unwrap();
    let ordinary = crate::presentation::ordinary_front_door_lines(
        &presentation,
        &application.entrance.as_ref().unwrap().navigation,
        application.selected_follow.as_deref(),
    )
    .unwrap()
    .join("\n");
    let destination = presentation
        .subjects
        .iter()
        .find(|subject| subject.identity == forward.target_subject)
        .unwrap();
    assert!(ordinary.contains("FOLLOW Realizes"));
    assert!(ordinary.contains(&destination.accessibility_name));
    assert!(ordinary.contains("[F3 SELECTED]"));

    let before = presentation.clone();
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F3))
        .unwrap();
    let followed = &application.entrance.as_ref().unwrap().navigation.cursor;
    assert_eq!(followed.place, forward.target_place);
    assert_eq!(followed.aspect, forward.target_aspect);
    assert_eq!(
        followed.focus.as_deref(),
        Some(forward.target_subject.as_str())
    );

    let reverse = application
        .entrance
        .as_ref()
        .unwrap()
        .navigation
        .navigation
        .follows
        .iter()
        .find(|follow| {
            follow.source_subject == forward.target_subject
                && follow.target_subject == forward.source_subject
        })
        .unwrap()
        .identity
        .clone();
    for _ in 0..application
        .entrance
        .as_ref()
        .unwrap()
        .navigation
        .navigation
        .follows
        .len()
    {
        application.modifiers = winit::keyboard::ModifiersState::SHIFT;
        application
            .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F3))
            .unwrap();
        if application.selected_follow.as_deref() == Some(reverse.as_str()) {
            break;
        }
    }
    assert_eq!(
        application.selected_follow.as_deref(),
        Some(reverse.as_str())
    );
    application.modifiers = winit::keyboard::ModifiersState::empty();
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F3))
        .unwrap();
    let returned = &application.entrance.as_ref().unwrap().navigation.cursor;
    assert_eq!(
        returned.place,
        conduit_presentation::PresentationPlace::Program
    );
    assert_eq!(
        returned.aspect,
        conduit_presentation::PresentationAspect::Plan
    );
    assert_eq!(
        returned.focus.as_deref(),
        Some(forward.source_subject.as_str())
    );
    assert_eq!(
        &application.entrance.as_ref().unwrap().presentation,
        &before
    );
}

#[test]
fn native_follow_refuses_zero_or_ambiguous_correlations_without_motion() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let before = application
        .entrance
        .as_ref()
        .unwrap()
        .navigation
        .cursor
        .clone();
    application.follow_front_door().unwrap();
    assert_eq!(
        application.entrance.as_ref().unwrap().navigation.cursor,
        before
    );
    assert_eq!(
        application.interaction_status.current().unwrap().text,
        "FOLLOW unavailable for the current Focus"
    );

    let presentation = patchbay_model::portable_demonstration().unwrap();
    let navigation =
        patchbay_model::PatchbayNavigationProjection::for_embodied(&presentation).unwrap();
    let forward = &navigation.navigation.follows[0];
    let focus = &forward.target_subject;
    let mut cursor = navigation.cursor.clone();
    cursor.focus = Some(focus.clone());
    let reverse = navigation
        .navigation
        .follows
        .iter()
        .find(|follow| {
            follow.source_subject == forward.target_subject
                && follow.target_subject == forward.source_subject
        })
        .unwrap();
    assert_eq!(
        crate::front_door_follow::exact_current_follow(&navigation.navigation, &cursor, None,),
        Err(crate::front_door_follow::NativeFollowRefusal::Ambiguous)
    );
    assert_eq!(
        crate::front_door_follow::exact_current_follow(
            &navigation.navigation,
            &cursor,
            Some(&reverse.identity),
        )
        .unwrap()
        .identity,
        reverse.identity
    );
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
        .entrance
        .as_ref()
        .unwrap()
        .presentation
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
    assert_eq!(
        application
            .entrance
            .as_ref()
            .unwrap()
            .navigation
            .cursor
            .focus
            .as_deref(),
        Some(seed_subject.as_str())
    );
    let selected = Some(seed_subject);
    application.handle_gui_action(GuiAction::OpenBack).unwrap();
    let opened = &application.entrance.as_ref().unwrap().presentation;
    assert_eq!(opened.revision, 2);
    assert!(opened.basis.body_id.is_none());
    assert!(opened.basis.seed_id.is_none());
    assert_eq!(
        application
            .entrance
            .as_ref()
            .unwrap()
            .state
            .selected_subject,
        selected
    );
    assert!(opened.properties.iter().any(|property| {
        property.name == "opened"
            && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
    }));
}
