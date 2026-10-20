//! Actual Birth projection reading/navigation; no checkbox or panel claims.
use super::*;
use alloc::{string::String, vec, vec::Vec};
use conduit_birth_plot::{BirthDraft, BirthFaceBasis, BirthPlotChoice};
use conduit_face::{FaceActionAvailability, plan_face_utterances};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

fn draft() -> BirthDraft {
    let choices: Vec<_> = crate::native_workset::profile()
        .installed()
        .iter()
        .map(|plot| BirthPlotChoice {
            title: plot.title().into(),
            search_text: plot.title().into(),
            plot: crate::native_workset::resident(*plot).unwrap(),
            refusal: None,
            selected: true,
        })
        .collect();
    assert_eq!(
        choices.len(),
        4,
        "exercise the installed four-Plot inventory"
    );
    BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap()
}

fn face(draft: &BirthDraft) -> Face {
    let plan = tests::fixture::producer_plan();
    draft
        .face(&BirthFaceBasis {
            host_id: "host/creche-navigation".into(),
            boot_id: "boot/creche-navigation".into(),
            encounter_id: "arrival/creche-navigation".into(),
            producer_plot: conduit_core::PlotIdentity {
                source_document_id: plan.source_document_id,
                checked_plot_id: plan.checked_plot_id,
                expanded_plot_id: plan.expanded_plot_id,
            },
            producer_plan_id: plan.plan_id,
        })
        .unwrap()
}

#[test]
fn source_disclosure_presents_one_title_and_preserves_every_exact_clause() {
    let original = face(&draft());
    let (primary, details) = document::prepare(&original, true).unwrap();
    let commitments: Vec<_> = primary
        .iter()
        .filter(|row| row.role == GraphicsTextRole::Heading && row.control.is_some())
        .collect();
    assert_eq!(commitments.len(), 1);
    assert_eq!(
        original.actions[commitments[0].control.unwrap().action].identity,
        "creche.birth"
    );
    assert_eq!(
        primary
            .iter()
            .filter(|row| row.text == "A body of your own")
            .count(),
        1,
    );
    let exact: String = details.iter().map(|row| row.text.as_str()).collect();
    for clause in plan_face_utterances(&original).unwrap().clauses {
        assert!(exact.contains(&clause.text), "lost exact Source clause");
    }
    assert!(primary.iter().any(|row| {
        row.text.starts_with("Friendly Body name: ")
            && row.control.is_some_and(|control| {
                control.argument == Some(0)
                    && original.actions[control.action].identity == "creche.name"
            })
    }));
    assert!(
        !primary
            .iter()
            .any(|row| row.text.starts_with("Starting Plot "))
    );
    assert!(
        !primary
            .iter()
            .any(|row| row.text.starts_with("Current value: ") || row.text.starts_with("Choice: "))
    );
    for plot in draft().choices() {
        let named: Vec<_> = primary
            .iter()
            .filter(|row| row.text == plot.title)
            .collect();
        assert_eq!(named.len(), 1, "one primary name per verified choice");
        assert!(named[0].checkbox.is_some());
    }
    assert!(!primary.iter().any(|row| row.text == "Included."));
    let mut filtered = draft();
    filtered
        .search_plots(filtered.revision(), "no-match")
        .unwrap();
    let (filtered_primary, _) = document::prepare(&face(&filtered), true).unwrap();
    for plot in filtered.choices().iter().filter(|choice| choice.selected) {
        assert!(filtered_primary.iter().any(|row| row.text == plot.title));
    }
}

#[test]
fn real_naming_catalogue_uses_checked_membership_and_keeps_choice_uncommitted() {
    let original = face(&draft());
    let index = original
        .actions
        .iter()
        .position(|action| action.identity == "creche.naming")
        .unwrap();
    let choices = text_field::choices(&original, index).unwrap();
    assert_eq!(choices.as_slice().len(), 24);
    assert_eq!(
        text_field::adjacent(&original, index, None, false),
        Some("roman")
    );
    assert!(
        original.actions[index].arguments[0]
            .contract
            .validate(b"roman")
            .is_ok()
    );
    let show = tests::fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original.clone(), 1280, 800).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "creche.naming".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
    assert!(matches!(
        scene.key(key(80, false), &show, 1).unwrap(),
        FaceSceneInput::Changed
    ));
    assert_eq!(scene.face(), &original);
    let FaceSceneInput::Submitted { interaction, .. } =
        scene.key(key(40, false), &show, 2).unwrap()
    else {
        panic!("checked naming choice must submit on Enter")
    };
    assert_eq!(interaction.arguments[0].value, b"roman");
    assert_eq!(scene.face(), &original);
}

fn key(usage: u8, shift: bool) -> KeyEvent {
    KeyEvent::new(
        usage,
        KeyTransition::Pressed,
        if shift {
            KeyModifiers::LEFT_SHIFT
        } else {
            KeyModifiers::NONE
        },
    )
    .unwrap()
}

fn controls(scene: &NativeFaceScene) -> Vec<FaceControl> {
    let mut result = vec![];
    for row in scene.rows() {
        if let Some(control) = row.control
            && scene.face().actions[control.action]
                .availability
                .is_available()
            && !result.contains(&control)
        {
            result.push(control);
        }
    }
    // Check completeness independently of the rendered rows: every actual
    // offered action and each exact argument must have a reachable control.
    for (action, contract) in scene.face().actions.iter().enumerate() {
        if contract.availability.is_available() {
            assert!(result.contains(&FaceControl {
                action,
                argument: None
            }));
            for argument in 0..contract.arguments.len() {
                assert!(
                    result.contains(&FaceControl {
                        action,
                        argument: Some(argument)
                    }) || (!scene.showing_details()
                        && argument == 0
                        && boolean_choice::selected(scene.face(), action).is_some()
                        && result.contains(&FaceControl {
                            action,
                            argument: None
                        }))
                );
            }
        }
    }
    result
}

fn traverse(scene: &mut NativeFaceScene, show: &MaskShow) {
    let expected = controls(scene);
    assert!(!expected.is_empty());
    for forward in [true, false] {
        scene.focus = None;
        let order: Vec<_> = if forward {
            expected.clone()
        } else {
            expected.iter().rev().copied().collect()
        };
        for control in order {
            scene.key(key(43, !forward), show, 1).unwrap();
            assert_eq!(scene.focused(), Some(control));
            let frame = scene.frame().unwrap();
            assert!(frame.hits.iter().any(|hit| hit.control == control));
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
        }
        scene.key(key(43, !forward), show, 1).unwrap();
        assert_eq!(
            scene.focused(),
            if forward {
                expected.first()
            } else {
                expected.last()
            }
            .copied()
        );
    }
}

fn read_pages(scene: &mut NativeFaceScene, show: &MaskShow) -> String {
    while scene.page() > 0 {
        scene.key(key(75, false), show, 1).unwrap();
    }
    let mut text = String::new();
    for page in 0..scene.pages() {
        assert_eq!(scene.page(), page);
        let frame = scene.frame().unwrap();
        assert_eq!(frame.pages, scene.pages());
        assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
        // Read actual emitted text, including every wrapped continuation.
        for row in scene.rows().iter().filter(|row| row.page == page) {
            assert!(
                frame
                    .scene
                    .commands()
                    .iter()
                    .any(|command| command.payload() == row.text)
            );
            text.push_str(&row.text);
        }
        scene.key(key(78, false), show, 1).unwrap();
    }
    text
}

#[test]
fn installed_creche_all_controls_and_named_choices_survive_narrow_overflow() {
    let mut draft = draft();
    let name = "Éléonore 雪 ".repeat(4);
    assert!(name.len() <= conduit_birth_plot::names::MAX_FRIENDLY_NAME_BYTES);
    draft.edit_name(draft.revision(), name.clone()).unwrap();
    let original = face(&draft);
    let show = tests::fixture::show(&original);
    for (width, height) in [(320, 240), (640, 480), (1280, 800)] {
        let mut scene = NativeFaceScene::prepare(original.clone(), width, height).unwrap();
        assert!(scene.pages() > 1);
        traverse(&mut scene, &show);
        let primary = read_pages(&mut scene, &show);
        for choice in draft.choices() {
            assert!(
                primary.contains(&choice.title),
                "missing named Plot {}",
                choice.title
            );
        }
        scene.show_details(true);
        traverse(&mut scene, &show);
        let details = read_pages(&mut scene, &show);
        for clause in plan_face_utterances(&original).unwrap().clauses {
            assert!(
                details.contains(&clause.text),
                "lost exact clause {}",
                clause.text
            );
        }
        assert!(details.contains(&name));
        assert_eq!(scene.face(), &original);
    }
}

#[test]
fn unavailable_birth_remains_readable_but_never_focusable() {
    let mut draft = draft();
    draft.edit_name(draft.revision(), String::new()).unwrap();
    let original = face(&draft);
    let birth = original
        .actions
        .iter()
        .find(|action| action.identity == "creche.birth")
        .unwrap();
    let FaceActionAvailability::Unavailable {
        reason_code,
        explanation,
    } = &birth.availability
    else {
        panic!("empty source name must withhold Birth");
    };
    let show = tests::fixture::show(&original);
    for (width, height) in [(320, 240), (1280, 800)] {
        let mut scene = NativeFaceScene::prepare(original.clone(), width, height).unwrap();
        assert_eq!(
            scene.focus_named(&FaceFocusRequest {
                action_id: birth.identity.clone(),
                argument_name: None
            }),
            Err(FaceSceneError::UnavailableAction)
        );
        for details in [false, true] {
            scene.show_details(details);
            traverse(&mut scene, &show);
            let text = read_pages(&mut scene, &show);
            assert!(text.contains(explanation));
            assert!(text.contains(reason_code));
        }
        assert_eq!(scene.face(), &original);
    }
}

#[test]
fn refreshed_creche_focus_uses_action_names_and_filtered_choice_refuses() {
    let mut draft = draft();
    let before = face(&draft);
    let request = FaceFocusRequest {
        action_id: "creche.plot.3".into(),
        argument_name: Some("value".into()),
    };
    let mut prior = NativeFaceScene::prepare(before.clone(), 320, 240).unwrap();
    prior.focus_named(&request).unwrap();
    let old_control = prior.focused().unwrap();
    draft
        .edit_name(draft.revision(), "Éléonore 雪".into())
        .unwrap();
    let refreshed = face(&draft);
    assert_ne!(refreshed.identity, before.identity);
    assert_ne!(refreshed.revision, before.revision);
    let mut scene = NativeFaceScene::prepare(refreshed.clone(), 320, 240).unwrap();
    assert_eq!(
        scene
            .resolve(&before.identity, before.revision, old_control)
            .unwrap_err(),
        FaceSceneError::StaleFace
    );
    scene.focus_named(&request).unwrap();
    let frame = scene.frame().unwrap();
    assert!(frame.hits.iter().any(|hit| {
        hit.action.identity == request.action_id
            && (hit
                .argument
                .is_some_and(|argument| argument.name == "value")
                || (hit.control.argument.is_none()
                    && boolean_choice::selected(&refreshed, hit.control.action).is_some()))
    }));
    draft
        .search_plots(draft.revision(), "no-matching-installed-plot")
        .unwrap();
    let filtered = face(&draft);
    let mut scene = NativeFaceScene::prepare(filtered.clone(), 320, 240).unwrap();
    assert_eq!(
        scene.focus_named(&request),
        Err(FaceSceneError::UnknownControl)
    );
    assert!(scene.focused().is_none());
    assert_eq!(
        draft
            .choices()
            .iter()
            .filter(|choice| choice.selected)
            .count(),
        4
    );
    scene.show_details(true);
    let text = read_pages(&mut scene, &tests::fixture::show(&filtered));
    for choice in draft.choices() {
        assert!(
            text.contains(&choice.title),
            "search hid selected review name {}",
            choice.title
        );
    }
}
