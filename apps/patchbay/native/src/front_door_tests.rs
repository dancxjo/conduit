use super::*;
use crate::gui_hit::GuiAction;

fn action_lines_for_seed_label<'a>(lines: &'a [String], label: &str) -> Option<&'a [String]> {
    let header = format!("Seed  {}", label);
    let start = lines.iter().position(|line| line == &header)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| {
            if line.starts_with("Seed  ") {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(lines.len());
    Some(&lines[start + 1..end])
}

fn two_seed_front_door_projection(
    application: &mut PatchbayApplication,
) -> Result<(String, String), String> {
    let session = application
        .zero_body_front_door
        .as_mut()
        .ok_or("zero-body front-door session is absent")?;
    let extra_seed = patchbay_model::SeedCandidate::from_source(
        "Patchbay entrance secondary",
        "hello.conduit",
        include_str!("../../../../examples/hello.conduit"),
        "registry sample two",
        conduit_core::SignId::from("patchbay/front-door/seed/secondary"),
        2,
    )
    .map_err(|error| format!("seed candidate: {error}"))?;
    session
        .add_seed(extra_seed)
        .map_err(|error| format!("session seed: {error}"))?;
    application
        .refresh_front_door()
        .map_err(|error| format!("front-door refresh: {error}"))?;
    let entrance = application
        .entrance
        .as_ref()
        .ok_or("front-door entrance is absent")?;
    let seed_subjects = entrance
        .presentation
        .subjects
        .iter()
        .filter(|subject| subject.role == conduit_presentation::PresentationRole::Seed)
        .map(|subject| subject.identity.clone())
        .collect::<Vec<_>>();
    if seed_subjects.len() != 2 {
        return Err(format!(
            "expected two seeds after adding a candidate, found {}",
            seed_subjects.len()
        ));
    }
    let (seed_a, seed_b) = (seed_subjects[0].clone(), seed_subjects[1].clone());
    Ok((seed_a, seed_b))
}

fn install_mutated_presentation(
    application: &mut PatchbayApplication,
    presentation: conduit_presentation::Presentation,
) -> Result<(), String> {
    let focus = application
        .entrance
        .as_ref()
        .ok_or("front-door entrance is absent")?
        .navigation
        .cursor
        .focus
        .clone();
    let navigation = if presentation.basis.body_id.is_some() {
        patchbay_model::PatchbayNavigationProjection::for_embodied(&presentation)
            .map_err(|error| format!("front-door mutated projection: {error}"))?
    } else {
        patchbay_model::PatchbayNavigationProjection::for_zero_body(&presentation, false)
            .map_err(|error| format!("front-door mutated projection: {error}"))?
    };
    let entrance =
        crate::front_door::NativeFrontDoorPresentation::new(presentation.clone(), navigation)
            .map_err(|error| format!("front-door mutated state: {error}"))?;
    application.entrance = Some(entrance);
    if let Some(execution) = application.renderer_execution.as_mut() {
        execution.presentation = presentation.clone();
    }
    if let Some(focus) = focus {
        application.select_front_door_subject(&focus)?;
    }
    Ok(())
}

fn rebuild_presentation_with_actions(
    source: &conduit_presentation::Presentation,
    actions: Vec<conduit_presentation::PresentationAction>,
) -> Result<conduit_presentation::Presentation, String> {
    conduit_presentation::Presentation::new_with_semantics(
        source.revision,
        source.basis.clone(),
        source.subjects.clone(),
        source.relationships.clone(),
        source.properties.clone(),
        source.text.clone(),
        actions,
        source.disclosures.clone(),
    )
    .map_err(|error| format!("presentation rebuild: {error:?}"))
}

fn set_seed_action_availability(
    presentation: &conduit_presentation::Presentation,
    subject: &str,
    intent: &str,
    availability: conduit_presentation::PresentationActionAvailability,
) -> Result<conduit_presentation::Presentation, String> {
    let mut actions = presentation.actions.clone();
    let mut modified = false;
    for action in &mut actions {
        if action.target == subject && action.intent == intent {
            action.availability = availability.clone();
            modified = true;
        }
    }
    if !modified {
        return Err(format!(
            "no matching action for subject {subject} and intent {intent}"
        ));
    }
    rebuild_presentation_with_actions(presentation, actions)
}

#[test]
fn native_front_door_begins_on_truthful_zero_body_world_state() {
    let application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let entrance = application.entrance.as_ref().unwrap();
    let presentation = &entrance.presentation;
    assert!(!application.parts_open);
    assert!(application.form_editor.is_none());
    assert!(application.build_birth.body().is_none());
    assert!(presentation.basis.body_id.is_none());
    assert!(presentation.basis.wake_id.is_none());
    assert!(presentation.basis.seed_id.is_none());
    let initial_focus = entrance
        .navigation
        .cursor
        .focus
        .as_deref()
        .expect("zero-Body front door has an exact initial Focus");
    assert!(presentation.subjects.iter().any(|subject| {
        subject.identity == initial_focus
            && subject.role == conduit_presentation::PresentationRole::Seed
    }));
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
    let focused_open = crate::presentation::focused_action_for_binding(
        presentation,
        &entrance.navigation,
        patchbay_model::PatchbayAction::OpenBack,
    )
    .expect("initial binding projection is valid")
    .expect("initial Seed Focus resolves the advertised OPEN binding");
    assert_eq!(focused_open.target, initial_focus);
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
    let seed_subject = application
        .zero_body_front_door
        .as_ref()
        .unwrap()
        .seed_ids()
        .first()
        .map(|seed_id| format!("seed/{}", seed_id.as_str()))
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
    let presentation = patchbay_model::portable_demonstration_with_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )
    .unwrap();
    let navigation =
        patchbay_model::PatchbayNavigationProjection::for_embodied(&presentation).unwrap();
    application.entrance = Some(
        crate::front_door::NativeFrontDoorPresentation::new(presentation.clone(), navigation)
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

    let presentation = patchbay_model::portable_demonstration_with_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )
    .unwrap();
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
    let seed_subject = application
        .zero_body_front_door
        .as_ref()
        .unwrap()
        .seed_ids()
        .first()
        .map(|seed_id| format!("seed/{}", seed_id.as_str()))
        .unwrap();
    application
        .select_front_door_subject(&seed_subject)
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
fn front_door_focus_selection_is_single_sourced_by_navigation_cursor() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let (seed_one, seed_two) =
        two_seed_front_door_projection(&mut application).expect("two seed subjects expected");
    let baseline_revision = application
        .zero_body_front_door
        .as_ref()
        .expect("zero-body front-door session is present")
        .revision();
    let presentation = application.entrance.as_ref().unwrap().presentation.clone();
    let presentation = set_seed_action_availability(
        &presentation,
        &seed_one,
        patchbay_model::PatchbayAction::OpenBack.presentation_intent(),
        conduit_presentation::PresentationActionAvailability::Unavailable {
            reason_code: "cursor-test-blocked-alt".into(),
            explanation: "Cursor-only selection cannot grant OPEN".into(),
        },
    )
    .unwrap();
    let presentation = set_seed_action_availability(
        &presentation,
        &seed_two,
        patchbay_model::PatchbayAction::OpenBack.presentation_intent(),
        conduit_presentation::PresentationActionAvailability::Unavailable {
            reason_code: "cursor-test-blocked".into(),
            explanation: "Selection should not determine offered action".into(),
        },
    )
    .unwrap();
    install_mutated_presentation(&mut application, presentation).unwrap();

    application.select_front_door_subject(&seed_one).unwrap();
    assert_eq!(
        application
            .entrance
            .as_ref()
            .expect("front-door entrance should be present")
            .navigation
            .cursor
            .focus,
        Some(seed_one.clone())
    );
    let focused_action = crate::presentation::focused_action_for_binding(
        &application.entrance.as_ref().unwrap().presentation,
        &application.entrance.as_ref().unwrap().navigation,
        patchbay_model::PatchbayAction::OpenBack,
    )
    .expect("projection lookup should work")
    .expect("focused OPEN action should exist");
    assert_eq!(focused_action.target, seed_one);
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ))
        .unwrap();
    assert_eq!(
        application
            .zero_body_front_door
            .as_ref()
            .unwrap()
            .revision(),
        baseline_revision
    );
    let refusal = application.interaction_status.current().unwrap();
    assert_eq!(
        refusal.code,
        crate::interaction_status::InteractionStatusCode::Refused
    );

    application.select_front_door_subject(&seed_two).unwrap();
    assert_eq!(
        application
            .entrance
            .as_ref()
            .expect("front-door entrance should be present")
            .navigation
            .cursor
            .focus,
        Some(seed_two.clone())
    );
    let focused_action = crate::presentation::focused_action_for_binding(
        &application.entrance.as_ref().unwrap().presentation,
        &application.entrance.as_ref().unwrap().navigation,
        patchbay_model::PatchbayAction::OpenBack,
    )
    .expect("projection lookup should work")
    .expect("focused OPEN action should exist");
    assert_eq!(focused_action.target, seed_two);
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ))
        .unwrap();
    assert_eq!(
        application
            .zero_body_front_door
            .as_ref()
            .unwrap()
            .revision(),
        baseline_revision
    );
    let refusal = application.interaction_status.current().unwrap();
    assert_eq!(
        refusal.code,
        crate::interaction_status::InteractionStatusCode::Refused
    );
}

#[test]
fn native_front_door_focuses_and_invokes_actions_only_on_current_projection_subject() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let (seed_one, seed_two) =
        two_seed_front_door_projection(&mut application).expect("two seed subjects expected");
    let initial_revision = application
        .zero_body_front_door
        .as_ref()
        .unwrap()
        .revision();
    let presentation = application.entrance.as_ref().unwrap().presentation.clone();
    let presentation = set_seed_action_availability(
        &presentation,
        &seed_one,
        patchbay_model::PatchbayAction::OpenBack.presentation_intent(),
        conduit_presentation::PresentationActionAvailability::Available,
    )
    .unwrap();
    let presentation = set_seed_action_availability(
        &presentation,
        &seed_two,
        patchbay_model::PatchbayAction::OpenBack.presentation_intent(),
        conduit_presentation::PresentationActionAvailability::Unavailable {
            reason_code: "seed-two-blocked".into(),
            explanation: "Projection test keeps this OPEN unavailable".into(),
        },
    )
    .unwrap();
    install_mutated_presentation(&mut application, presentation).unwrap();

    application.select_front_door_subject(&seed_two).unwrap();
    let projected_lines = application.presentation_lines();
    let seed_two_label = application
        .entrance
        .as_ref()
        .expect("front-door entrance is present")
        .presentation
        .subjects
        .iter()
        .find(|subject| subject.identity == seed_two)
        .expect("focus seed subject should exist")
        .label
        .as_str()
        .to_string();
    let focused_actions = action_lines_for_seed_label(&projected_lines, &seed_two_label)
        .expect("focused seed action block must exist");
    assert!(focused_actions.iter().any(|line| {
        line.contains("OPEN [ENTER]  ·  UNAVAILABLE — Projection test keeps this OPEN unavailable")
    }));
    assert!(!focused_actions
        .iter()
        .any(|line| line.contains("OPEN [ENTER]  ·  AVAILABLE")));
    let seed_one_label = application
        .entrance
        .as_ref()
        .expect("front-door entrance is present")
        .presentation
        .subjects
        .iter()
        .find(|subject| subject.identity == seed_one)
        .expect("other seed subject should exist")
        .label
        .as_str()
        .to_string();
    let unfocused_actions = action_lines_for_seed_label(&projected_lines, &seed_one_label)
        .expect("unfocused seed action block must exist");
    assert!(unfocused_actions
        .iter()
        .any(|line| line == "  OPEN  ·  AVAILABLE"));
    assert!(!unfocused_actions
        .iter()
        .any(|line| line.contains("OPEN [ENTER]")));
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ))
        .unwrap();
    assert_eq!(
        application
            .zero_body_front_door
            .as_ref()
            .unwrap()
            .revision(),
        initial_revision
    );

    application.select_front_door_subject(&seed_one).unwrap();
    let projected_lines = application.presentation_lines();
    let seed_one_label = application
        .entrance
        .as_ref()
        .expect("front-door entrance is present")
        .presentation
        .subjects
        .iter()
        .find(|subject| subject.identity == seed_one)
        .expect("focus seed subject should exist")
        .label
        .as_str()
        .to_string();
    let focused_actions = action_lines_for_seed_label(&projected_lines, &seed_one_label)
        .expect("focused seed action block must exist");
    assert!(focused_actions
        .iter()
        .any(|line| line.contains("OPEN [ENTER]  ·  AVAILABLE")));
    assert!(!focused_actions.iter().any(|line| {
        line.contains("UNAVAILABLE — Projection test keeps this OPEN unavailable")
    }));
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ))
        .unwrap();
    assert_eq!(
        application
            .zero_body_front_door
            .as_ref()
            .unwrap()
            .revision(),
        initial_revision + 1
    );
    let opened = application
        .zero_body_front_door
        .as_ref()
        .unwrap()
        .opened()
        .expect("seed should be opened");
    assert!(matches!(
        opened,
        patchbay_model::OpenedFrontDoorSubject::Seed { .. }
    ));
}

#[test]
fn native_front_door_refused_projection_action_cannot_be_invoked() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let seed = application
        .zero_body_front_door
        .as_ref()
        .unwrap()
        .seed_ids()
        .first()
        .map(|seed_id| format!("seed/{}", seed_id.as_str()))
        .unwrap();
    application.select_front_door_subject(&seed).unwrap();
    let presentation = set_seed_action_availability(
        &application.entrance.as_ref().unwrap().presentation,
        &seed,
        patchbay_model::PatchbayAction::Birth.presentation_intent(),
        conduit_presentation::PresentationActionAvailability::Refused {
            reason_code: "test-blocked".into(),
            explanation: "Refused for projection-only test".into(),
        },
    )
    .unwrap();
    install_mutated_presentation(&mut application, presentation).unwrap();
    application
        .handle_front_door_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F4))
        .unwrap();
    let refusal = application.interaction_status.current().unwrap();
    assert!(
        refusal.text.contains("Interaction refused: ActionRefused")
            || refusal.text.contains("test-blocked")
    );
}

#[test]
fn native_front_door_stale_presentation_and_unknown_action_ids_fail_closed() {
    let mut application = PatchbayApplication::new(Arguments {
        front_door: true,
        ..Arguments::default()
    })
    .unwrap();
    let seed = application
        .zero_body_front_door
        .as_ref()
        .unwrap()
        .seed_ids()
        .first()
        .map(|seed_id| format!("seed/{}", seed_id.as_str()))
        .unwrap();
    application.select_front_door_subject(&seed).unwrap();
    let initial_presentation = application.entrance.as_ref().unwrap().presentation.clone();
    let stale_revision = initial_presentation.revision + 1;
    let stale = set_seed_action_availability(
        &initial_presentation,
        &seed,
        patchbay_model::PatchbayAction::OpenBack.presentation_intent(),
        conduit_presentation::PresentationActionAvailability::Available,
    )
    .unwrap();
    let stale = {
        let mut stale = stale;
        stale.revision = stale_revision;
        stale
    };
    let stale = rebuild_presentation_with_actions(&stale, stale.actions.clone()).unwrap();
    install_mutated_presentation(&mut application, stale).unwrap();
    application.select_front_door_subject(&seed).unwrap();
    assert!(application
        .handle_front_door_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ))
        .is_ok());
    assert_eq!(
        application.interaction_status.current().unwrap().text,
        "Interaction refused: OperationRejected"
    );
    assert_eq!(
        application
            .zero_body_front_door
            .as_ref()
            .unwrap()
            .revision(),
        initial_presentation.revision
    );

    let known_action = application.entrance.as_ref().unwrap().presentation.actions[0]
        .identity
        .clone();
    assert!(application
        .dispatch_invocation_with_action_id(&known_action)
        .is_ok());
    assert!(application
        .dispatch_invocation_with_action_id("action/does-not-exist")
        .is_err());
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
    application.handle_gui_action(GuiAction::OpenBack).unwrap();
    let opened = &application.entrance.as_ref().unwrap().presentation;
    assert_eq!(opened.revision, 2);
    assert!(opened.basis.body_id.is_none());
    assert!(opened.basis.seed_id.is_none());
    assert!(opened.properties.iter().any(|property| {
        property.name == "opened"
            && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
    }));
}
