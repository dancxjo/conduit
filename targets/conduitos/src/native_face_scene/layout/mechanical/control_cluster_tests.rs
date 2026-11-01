//! Oversized help may continue, while a fitting typed control pair stays usable.
use super::tests::{assert_geometry, item, narrow, prepared};
use super::*;
use alloc::vec;
use conduit_face::VisualTypography;

#[test]
fn oversized_subject_help_keeps_fitting_action_and_argument_on_one_page() {
    for typography in [VisualTypography::Editorial, VisualTypography::Engineering] {
        let face = crate::native_face_scene::tests::face(7);
        let before = face.clone();
        let profile = prepared(&face, vec![narrow()]);
        let help = "Give this Body a name. ".repeat(6);
        let value = "Friendly Body name: Café Llanfairpwllgwyngyll long Body name";
        let mut action = item("Edit Body name", Some(1), true);
        action.control = Some(FaceControl {
            action: 1,
            argument: None,
        });
        let mut argument = item(value, Some(1), true);
        argument.control = Some(FaceControl {
            action: 1,
            argument: Some(0),
        });
        let rows = admit(
            vec![item(&help, Some(1), false), action, argument],
            LayoutRect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
            &profile,
            200,
            RowSpacing::legacy(SPACE_SM * 2),
            Typography::new(typography),
        )
        .unwrap();
        assert_eq!(
            rows.iter().map(|r| r.text.as_str()).collect::<String>(),
            alloc::format!("{help}Edit Body name{value}")
        );
        let controls = rows
            .iter()
            .filter(|row| row.control.is_some())
            .collect::<Vec<_>>();
        assert_eq!(controls.len(), 2);
        assert_eq!(
            controls[0].control,
            Some(FaceControl {
                action: 1,
                argument: None
            })
        );
        assert_eq!(
            controls[1].control,
            Some(FaceControl {
                action: 1,
                argument: Some(0)
            })
        );
        assert_eq!(
            controls[0].page, controls[1].page,
            "fitting interactive pair must not inherit oversized help continuation"
        );
        assert!(controls[0].page > rows[0].page);
        assert_geometry(&rows, 640, 480);
        assert_eq!(face, before);
    }
}
