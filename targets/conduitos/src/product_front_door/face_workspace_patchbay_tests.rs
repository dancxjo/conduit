//! Ordinary installed Source -> native Mask -> resident Fore -> actual editor.
//! CPU native enactment, not a QEMU/physical capture.
use super::*;
use crate::native_workset::{self, NativePlot};
use alloc::borrow::ToOwned;
use conduit_face::FaceInteractionArgument;

#[test]
fn installed_patchbay_typed_edit_opens_actual_source_editor_once() {
    edit_once(false);
}

#[test]
fn installed_patchbay_keyboard_choice_opens_actual_source_editor_once() {
    edit_once(true);
}

fn edit_once(keyboard: bool) {
    let (mut journey, mut door, mut workspace, mut display) = tests::encounter();
    let patchbay = native_workset::resident(NativePlot::Patchbay).unwrap();
    journey.select_plot(&patchbay, journey.revision()).unwrap();
    assert!(journey.foreground_canonical_patchbay_active());
    assert!(journey.foreground_application_view().is_none());
    door.observe_product(&journey).unwrap();
    workspace.present_current(&door, &mut display).unwrap();
    let face = door.face().unwrap();
    let show = workspace.selected_show.clone().unwrap();
    let subject = journey
        .foreground_patchbay_graph()
        .unwrap()
        .subject_identities()
        .next()
        .unwrap()
        .to_owned();
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == patchbay_application::EDIT_CURRENT_ACTION_ID)
        .unwrap();
    let delivered = if keyboard {
        workspace
            .mask
            .focus_named(
                &crate::native_face_scene::FaceFocusRequest {
                    action_id: action.identity.clone(),
                    argument_name: Some("subject".into()),
                },
                &mut display,
            )
            .unwrap();
        let before_choice = journey.projection();
        let right = conduit_human::KeyEvent::new(
            79,
            conduit_human::KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap();
        assert!(matches!(
            workspace.mask.key(right, 1, &mut display).unwrap(),
            NativeFaceMaskInput::Redrawn(_)
        ));
        assert_eq!(
            journey.projection(),
            before_choice,
            "choosing is only a Mask-local draft"
        );
        let tab = conduit_human::KeyEvent::new(
            43,
            conduit_human::KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap();
        assert!(matches!(
            workspace.mask.key(tab, 2, &mut display).unwrap(),
            NativeFaceMaskInput::Redrawn(_)
        ));
        assert_eq!(
            journey.projection(),
            before_choice,
            "leaving a command argument must not execute it"
        );
        workspace
            .mask
            .focus_named(
                &crate::native_face_scene::FaceFocusRequest {
                    action_id: action.identity.clone(),
                    argument_name: Some("subject".into()),
                },
                &mut display,
            )
            .unwrap();
        workspace.mask.key(right, 3, &mut display).unwrap();
        let enter = conduit_human::KeyEvent::new(
            40,
            conduit_human::KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap();
        let NativeFaceMaskInput::Submitted { correlation, .. } =
            workspace.mask.key(enter, 4, &mut display).unwrap()
        else {
            panic!("Enter must submit the offered subject choice");
        };
        correlation.interaction
    } else {
        let input = FaceInteraction::new(
            &face,
            &show,
            &action.identity,
            &action.target,
            alloc::vec![FaceInteractionArgument {
                name: "subject".into(),
                value_kind: action.arguments[0].contract.value_kind.as_str().into(),
                value: subject.into_bytes()
            }],
            1,
        )
        .unwrap();
        workspace.mask.submit(input).unwrap().interaction
    };
    let before = journey.projection();
    let (ids, offer, _) = crate::product_journey::test_support::fixture();
    let make = MakeRecord {
        build_id: "build",
        ..MakeRecord::legacy()
    };
    workspace
        .apply_interaction(
            &mut door,
            &mut journey,
            &mut display,
            &ids,
            &offer,
            &make,
            &show,
            &delivered,
        )
        .unwrap();
    let after = journey.projection();
    assert_eq!(after.body_id, before.body_id);
    assert_eq!(after.plan_id, before.plan_id);
    assert_eq!(after.active_play_id, before.active_play_id);
    assert!(journey.foreground_text_editor_active());
    assert_eq!(
        after.result.as_deref(),
        Some(NativePlot::KeyboardCanvas.source())
    );
    assert!(workspace.editor_surface());
    assert_ne!(
        workspace.selected_show.as_ref().unwrap().show_id,
        show.show_id
    );
    assert_eq!(
        workspace.apply_interaction(
            &mut door,
            &mut journey,
            &mut display,
            &ids,
            &offer,
            &make,
            &show,
            &delivered
        ),
        Err("workspace-face-show-not-selected")
    );
    assert_eq!(journey.projection(), after);
}
