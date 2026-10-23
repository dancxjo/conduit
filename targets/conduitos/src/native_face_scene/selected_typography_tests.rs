//! Prepared selected metrics and provisional input; CPU fixtures are not guest proof.
use super::*;
use conduit_face::{
    VisualExpression, VisualTypography, VisualUserConstraints, admit_visual_expression,
};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

fn expression(intent: VisualTypography, scale: u16) -> conduit_face::AdmittedVisualExpression {
    let mut value = VisualExpression::phosphor();
    value.typography = intent;
    value.text_scale_percent = scale;
    admit_visual_expression(
        Ok(value),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap()
}

#[test]
fn selected_profiles_measure_actual_frame_bounds_and_retain_exact_face_and_named_focus() {
    let face = tests::face(1);
    let focus = FaceFocusRequest {
        action_id: "edit".into(),
        argument_name: Some("value".into()),
    };
    for (width, height, scale) in [(640, 480, 100), (640, 480, 200), (1280, 800, 125)] {
        for intent in [VisualTypography::Editorial, VisualTypography::Engineering] {
            let selected = expression(intent, scale);
            let mut scene = NativeFaceScene::prepare_visual_fallback(
                face.clone(),
                width,
                height,
                true,
                &selected,
            )
            .unwrap();
            scene.focus_named(&focus).unwrap();
            assert_eq!(scene.focused_request(), Some(focus.clone()));
            assert_eq!(scene.face(), &face);
            for command in scene.frame().unwrap().scene.commands() {
                if command.kind == conduit_face::GraphicsCommandKind::Text {
                    let measured = TextLayout::new_selected(
                        command.payload(),
                        scene.typography.style(command.text_role().into()),
                        command.bounds.width,
                        scale,
                    )
                    .unwrap()
                    .finish_height();
                    assert!(
                        measured <= u32::from(command.bounds.height),
                        "{}",
                        command.payload()
                    );
                }
            }
        }
    }
}

#[test]
fn typed_then_erased_and_submitted_input_remains_pending_for_safe_look_swap() {
    let face = tests::face(1);
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "edit".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
    let show = tests::fixture::show(&face);
    let key = |usage| KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    assert!(!scene.has_pending_input());
    assert!(matches!(
        scene.key(key(4), &show, 1).unwrap(),
        input::FaceSceneInput::Changed
    ));
    assert!(scene.has_pending_input());
    assert!(matches!(
        scene.key(key(42), &show, 2).unwrap(),
        input::FaceSceneInput::Changed
    ));
    assert!(
        scene.has_pending_input(),
        "empty text still has an uncommitted occurrence"
    );
    assert!(matches!(
        scene.key(key(41), &show, 3).unwrap(),
        input::FaceSceneInput::Changed
    ));
    assert!(
        !scene.has_pending_input(),
        "explicit cancellation releases the provisional occurrence"
    );
    scene.key(key(4), &show, 4).unwrap();
    assert!(matches!(
        scene.key(key(40), &show, 5).unwrap(),
        input::FaceSceneInput::Submitted { .. }
    ));
    assert!(
        scene.has_pending_input(),
        "producer has not acknowledged the occurrence"
    );
    assert_eq!(scene.face(), &face);
    let refreshed = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    assert!(
        !refreshed.has_pending_input(),
        "authoritative Face presentation constructs fresh input state"
    );
}
