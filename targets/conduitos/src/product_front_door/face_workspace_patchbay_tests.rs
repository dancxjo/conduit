//! Ordinary installed Source -> native Mask -> resident Fore -> actual editor.
//! CPU native enactment, not a QEMU/physical capture.
use super::*;
use crate::native_workset::{self, NativePlot};
use alloc::borrow::ToOwned;
use conduit_face::FaceInteractionArgument;

#[test]
fn installed_patchbay_typed_edit_opens_actual_source_editor_once() {
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
    let delivered = workspace.mask.submit(input).unwrap();
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
            &delivered.interaction,
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
            &delivered.interaction
        ),
        Err("workspace-face-show-not-selected")
    );
    assert_eq!(journey.projection(), after);
}
