//! Actual Source producer and final graphics role; CPU, not guest observation.
use super::*;
use conduit_face::{
    FacePropertyValue, FaceUtteranceProvenance, GraphicsTextRole, plan_face_utterances,
};

#[test]
fn actual_creche_identity_details_use_code_without_changing_face_or_human_text() {
    let (face, recipe, admitted) = actual_creche();
    let plan = plan_face_utterances(&face).unwrap();
    let (_, details) = crate::native_face_scene::document::prepare(&face, true).unwrap();
    assert_eq!(details.len(), plan.clauses.len());
    let mut expected_code = alloc::string::String::new();
    for (detail, clause) in details.iter().zip(&plan.clauses) {
        assert_eq!(detail.text, clause.text);
        let exact = match &clause.provenance {
            FaceUtteranceProvenance::Property(p) => matches!(
                face.properties[*p.index() as usize].value,
                FacePropertyValue::Identity(_)
            ),
            _ => false,
        };
        assert_eq!(
            detail.role,
            if exact {
                GraphicsTextRole::Code
            } else {
                GraphicsTextRole::Body
            }
        );
        if exact {
            expected_code.push_str(&clause.text);
        }
    }
    assert!(!expected_code.is_empty());
    for scale in [100, 200] {
        let mut expression = admitted.expression.clone();
        expression.text_scale_percent = scale;
        let expression = conduit_face::admit_visual_expression(
            Ok(expression),
            conduit_face::VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: scale,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        for (width, height) in [(640, 480), (1280, 800)] {
            let mut scene = NativeFaceScene::prepare_configured(
                face.clone(),
                width,
                height,
                true,
                &recipe,
                &expression,
            )
            .unwrap();
            assert_eq!(scene.face(), &face);
            scene.show_details(true);
            let mut painted_code = alloc::string::String::new();
            assert!(scene.pages() < 256);
            for page in 0..scene.pages() {
                scene.page = page;
                let frame = scene.frame().unwrap();
                assert!(frame.scene.commands().len() <= 48);
                for command in frame.scene.commands() {
                    if command.text_role() == GraphicsTextRole::Code {
                        painted_code.push_str(command.payload());
                    }
                }
            }
            assert_eq!(painted_code, expected_code);
            assert_eq!(scene.face(), &face);
        }
    }
}
