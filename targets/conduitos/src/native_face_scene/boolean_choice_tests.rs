//! Reversible choices use current semantic value and the exact Show contract.
use super::*;
use alloc::{string::String, vec};
use conduit_birth_plot::{BirthActionOutcome, BirthDraft, BirthFaceBasis, BirthPlotChoice};
use conduit_core::ValueConstraint;
use conduit_face::{FaceActionAvailability, FacePropertyValue, GraphicsCommandKind};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

fn fixture() -> (BirthDraft, BirthFaceBasis) {
    let choices = crate::native_workset::profile()
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
    let plan = tests::fixture::producer_plan();
    (
        BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap(),
        BirthFaceBasis {
            host_id: "host/boolean-choice".into(),
            boot_id: "boot/boolean-choice".into(),
            encounter_id: "arrival/boolean-choice".into(),
            producer_plot: conduit_core::PlotIdentity {
                source_document_id: plan.source_document_id,
                checked_plot_id: plan.checked_plot_id,
                expanded_plot_id: plan.expanded_plot_id,
            },
            producer_plan_id: plan.plan_id,
        },
    )
}

fn key(usage: u8) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
}

fn request(argument: bool) -> FaceFocusRequest {
    FaceFocusRequest {
        action_id: "creche.plot.3".into(),
        argument_name: argument.then(|| String::from("value")),
    }
}

fn submitted(result: FaceSceneInput) -> FaceInteraction {
    let FaceSceneInput::Submitted {
        interaction,
        next_focus,
    } = result
    else {
        panic!("choice must submit")
    };
    assert_eq!(next_focus.unwrap().action_id, "creche.plot.3");
    *interaction
}

#[test]
fn keyboard_boolean_false_then_true_requires_source_acceptance_and_fresh_show() {
    for usage in [40, 44] {
        let (mut draft, basis) = fixture();
        for (sequence, expected) in [(1, false), (2, true)] {
            let face = draft.face(&basis).unwrap();
            let show = tests::fixture::show(&face);
            let mut scene = NativeFaceScene::prepare(face.clone(), 320, 240).unwrap();
            // Fresh-focus requests may name the exact argument; the primary
            // choice remains the single activation control, not a text prompt.
            scene.focus_named(&request(true)).unwrap();
            assert!(!scene.showing_details());
            assert_eq!(scene.focused().unwrap().argument, None);
            let control = scene.focused().unwrap();
            assert_eq!(
                boolean_choice::selected(&face, control.action),
                Some(!expected)
            );
            let interaction = submitted(scene.key(key(usage), &show, sequence).unwrap());
            assert_eq!(interaction.action_id, "creche.plot.3");
            assert_eq!(interaction.target, face.actions[control.action].target);
            assert_eq!(interaction.arguments.len(), 1);
            assert_eq!(interaction.arguments[0].name, "value");
            assert_eq!(interaction.arguments[0].value_kind, "value/bool");
            assert_eq!(interaction.arguments[0].value, vec![u8::from(expected)]);
            assert_eq!(scene.face(), &face, "Mask cannot commit source selection");
            assert_eq!(draft.choices()[3].selected, !expected);
            assert_eq!(
                scene.key(key(usage), &show, sequence + 1).unwrap_err(),
                FaceSceneError::StaleFace
            );
            assert!(matches!(
                draft
                    .apply_face_interaction(&basis, &show, &interaction)
                    .unwrap(),
                BirthActionOutcome::Changed
            ));
            assert_eq!(draft.choices()[3].selected, expected);
            let refreshed = draft.face(&basis).unwrap();
            assert_ne!(refreshed.identity, face.identity);
            let action = refreshed
                .actions
                .iter()
                .position(|action| action.identity == "creche.plot.3")
                .unwrap();
            assert_eq!(boolean_choice::selected(&refreshed, action), Some(expected));
            assert!(
                draft
                    .apply_face_interaction(&basis, &show, &interaction)
                    .is_err()
            );
        }
        assert_eq!(
            draft
                .choices()
                .iter()
                .filter(|choice| choice.selected)
                .count(),
            4
        );
    }
}

#[test]
fn pointer_checkbox_has_named_hit_and_independent_non_color_state_geometry() {
    let (draft, basis) = fixture();
    let face = draft.face(&basis).unwrap();
    let show = tests::fixture::show(&face);
    for (width, height) in [(320, 240), (640, 480), (1280, 800)] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, height).unwrap();
        scene.focus_named(&request(false)).unwrap();
        let control = scene.focused().unwrap();
        let bounds = {
            let frame = scene.frame().unwrap();
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            let hit = frame
                .hits
                .iter()
                .find(|hit| hit.control == control)
                .unwrap();
            assert_eq!(hit.action.identity, "creche.plot.3");
            let square = frame
                .scene
                .commands()
                .iter()
                .find(|command| {
                    command.kind == GraphicsCommandKind::Rect
                        && command.bounds.width == 18
                        && command.bounds.height == 18
                })
                .unwrap();
            assert_ne!(square.bounds, hit.bounds, "checkbox is not the row outline");
            assert!(
                frame
                    .scene
                    .commands()
                    .iter()
                    .any(|command| command.kind == GraphicsCommandKind::Rect
                        && command.bounds.width == 10
                        && command.bounds.height == 10)
            );
            assert!(
                frame
                    .scene
                    .commands()
                    .iter()
                    .filter(|command| command.kind == GraphicsCommandKind::Rect
                        && command.paint == GraphicsPaintRole::Focus)
                    .count()
                    >= 2
            );
            hit.bounds
        };
        let interaction = submitted(scene.pointer(bounds.x + 1, bounds.y + 1, &show, 1).unwrap());
        assert_eq!(interaction.arguments[0].value, vec![0]);
        assert_eq!(scene.face(), &face);
    }
}

#[test]
fn generic_or_one_way_boolean_is_not_inferred_as_reversible_choice() {
    let (draft, basis) = fixture();
    let original = draft.face(&basis).unwrap();
    let index = original
        .actions
        .iter()
        .position(|action| action.identity == "creche.plot.3")
        .unwrap();
    let target = original.actions[index].target.clone();
    let mut generic = original.clone();
    generic
        .properties
        .retain(|property| property.subject != target || property.name != "boolean-change-action");
    assert_eq!(boolean_choice::selected(&generic, index), None);
    let mut wrong = original.clone();
    wrong
        .properties
        .iter_mut()
        .find(|property| property.subject == target && property.name == "boolean-change-action")
        .unwrap()
        .value = FacePropertyValue::Identity("another-action".into());
    assert_eq!(boolean_choice::selected(&wrong, index), None);
    let mut one_way = original.clone();
    one_way.actions[index].arguments[0]
        .contract
        .constraints
        .push(ValueConstraint::CanonicalMembership {
            members: vec![vec![1]],
            negated: false,
        });
    assert_eq!(boolean_choice::selected(&one_way, index), None);
    let mut ambiguous = original.clone();
    let property = ambiguous
        .properties
        .iter()
        .find(|property| property.subject == target && property.name == "selected")
        .unwrap()
        .clone();
    ambiguous.properties.push(property);
    assert_eq!(boolean_choice::selected(&ambiguous, index), None);
    let mut missing = original.clone();
    missing
        .properties
        .retain(|property| property.subject != target || property.name != "selected");
    assert_eq!(boolean_choice::selected(&missing, index), None);
}

#[test]
fn unavailable_and_stale_choices_never_toggle_or_grow_scene_budget() {
    let (mut draft, basis) = fixture();
    draft.select(draft.revision(), 3, false).unwrap();
    let available = draft.face(&basis).unwrap();
    let stale = tests::fixture::show(&available);
    draft
        .edit_name(draft.revision(), "A fresh name".into())
        .unwrap();
    let current = draft.face(&basis).unwrap();
    let mut scene = NativeFaceScene::prepare(current.clone(), 320, 240).unwrap();
    scene.focus_named(&request(false)).unwrap();
    assert_eq!(
        scene.key(key(44), &stale, 1).unwrap_err(),
        FaceSceneError::StaleFace
    );
    assert!(!draft.choices()[3].selected);
    // Withhold an existing exact source action while retaining current value.
    let mut withheld = current.clone();
    withheld
        .actions
        .iter_mut()
        .find(|action| action.identity == "creche.plot.3")
        .unwrap()
        .availability = FaceActionAvailability::Unavailable {
        reason_code: "test/host-unavailable".into(),
        explanation: "Host unavailable".into(),
    };
    let withheld = Face::new_with_semantics(
        withheld.revision,
        withheld.basis,
        withheld.subjects,
        withheld.relationships,
        withheld.properties,
        withheld.text,
        withheld.actions,
        withheld.disclosures,
    )
    .unwrap();
    let mut scene = NativeFaceScene::prepare(withheld.clone(), 320, 240).unwrap();
    assert_eq!(
        scene.focus_named(&request(false)),
        Err(FaceSceneError::UnavailableAction)
    );
    let mut text = String::new();
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
        assert!(
            frame
                .hits
                .iter()
                .all(|hit| hit.action.identity != "creche.plot.3")
        );
        for row in scene.rows().iter().filter(|row| row.page == scene.page()) {
            text.push_str(&row.text);
        }
        scene.turn_page(true);
    }
    assert!(text.contains("Host unavailable"));
    assert_eq!(scene.face(), &withheld);
}
