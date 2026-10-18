use super::*;
use crate::{
    display::{DisplayError, DisplayFormat},
    native_compositor::InputRoute,
    native_workset::{self, NativePlot},
    product_front_door::workspace_input::tests::listening,
    product_journey::test_support::fixture,
};
use conduit_face::GraphicsCommandKind;
use conduit_human::{KeyModifiers, KeyTransition};

#[derive(Default)]
pub(super) struct CountingDisplay(pub(super) u64, pub(super) bool);

impl PixelTarget for CountingDisplay {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: if self.1 { 1024 } else { 640 },
            height: if self.1 { 768 } else { 480 },
            pitch: if self.1 { 4096 } else { 2560 },
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }

    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        self.0 += 1;
        Ok(())
    }
}

#[test]
fn live_resident_patchbay_face_reaches_mask_show_and_diagram_then_returns_to_application() {
    let (mut journey, mut door) = listening();
    let patchbay = native_workset::resident(NativePlot::Patchbay).unwrap();
    journey.select_plot(&patchbay, journey.revision()).unwrap();
    door.observe_product(&journey).unwrap();
    assert!(journey.foreground_patchbay_graph().is_some());
    let face = door.face().unwrap();
    let encoded = serde_json::to_vec(&face).unwrap();
    assert!(
        encoded.len() <= crate::native_face_snapshot::MAX_SNAPSHOT_BYTES,
        "selected Patchbay Face needs {} snapshot bytes",
        encoded.len()
    );
    let projection = journey.projection();
    let provider = crate::product_bases::fixture_surface_provider();
    let mut workspace = FaceWorkspace::prepare(
        projection.host_id.clone(),
        projection.boot_id.clone(),
        projection.offer_generation,
        "build",
        provider.entry.base_id.clone(),
        &provider,
    )
    .unwrap();
    workspace.continue_after_birth(8, 12);
    let mut presenter = FrontDoorPresenter::prepare(
        projection.host_id,
        projection.boot_id,
        projection.offer_generation,
        "profile",
        "image",
        provider.entry.base_id.clone(),
        2,
    )
    .unwrap();
    let mut display = CountingDisplay::default();
    presenter.present(&door, &mut display).unwrap();
    let receipt = workspace
        .enter(&door, &mut presenter, &mut display)
        .unwrap();
    assert_eq!(receipt.face_id, face.identity);
    assert_eq!(
        workspace.mask.publication().unwrap().observation_sequence,
        8
    );
    assert_eq!(workspace.mask.show().unwrap().show.play_sequence, 12);
    assert!(workspace.active());
    assert!(workspace.mask.route_keyboard().unwrap());
    assert!(workspace.mask.scene().unwrap().has_diagram());
    let event = KeyEvent::new(60, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    let (ids, offer, _) = fixture();
    let make = crate::make::MakeRecord {
        build_id: "build",
        ..crate::make::MakeRecord::legacy()
    };
    assert!(
        workspace
            .accept_key(
                event,
                &mut door,
                &mut journey,
                &mut display,
                &ids,
                &offer,
                &make,
            )
            .unwrap()
            .is_some()
    );
    let scene = workspace.mask.scene().unwrap();
    assert!(scene.showing_diagram());
    assert!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|command| { command.kind == GraphicsCommandKind::OrthogonalPath })
    );
    let prior_show = workspace.mask.show().unwrap().show_id.clone();
    workspace
        .invoke_product_action(
            patchbay_control::PatchbayAction::Stop,
            &mut door,
            &mut journey,
            &mut display,
            &ids,
            &offer,
            &make,
        )
        .unwrap();
    assert_eq!(
        journey.status(),
        crate::product_journey::JourneyStatus::Stopped
    );
    assert_ne!(workspace.mask.show().unwrap().show_id, prior_show);
    workspace
        .leave(&door, &mut presenter, &mut display)
        .unwrap();
    assert!(!workspace.active());
    assert!(matches!(
        presenter.route_keyboard().unwrap(),
        InputRoute::Delivered(_)
    ));
}

pub(super) fn encounter() -> (ProductJourney, FrontDoor, FaceWorkspace, CountingDisplay) {
    let (journey, door) = listening();
    let projection = journey.projection();
    let provider = crate::product_bases::fixture_surface_provider();
    let mut workspace = FaceWorkspace::prepare(
        projection.host_id,
        projection.boot_id,
        projection.offer_generation,
        "build",
        provider.entry.base_id.clone(),
        &provider,
    )
    .unwrap();
    let mut display = CountingDisplay::default();
    workspace.present_current(&door, &mut display).unwrap();
    workspace.active = true;
    (journey, door, workspace, display)
}

fn stop(face: &Face, show: &MaskShow) -> FaceInteraction {
    let action = face
        .actions
        .iter()
        .find(|action| action.intent == patchbay_control::PatchbayAction::Stop.face_intent())
        .unwrap();
    FaceInteraction::new(
        face,
        show,
        &action.identity,
        &action.target,
        alloc::vec![],
        1,
    )
    .unwrap()
}

#[test]
fn current_action_rejects_forged_sequence_arguments_and_failed_show() {
    let (_journey, door, workspace, _display) = encounter();
    let face = door.face().unwrap();
    let show = workspace.selected_show.as_ref().unwrap();
    let interaction = stop(&face, show);
    assert_eq!(
        resolve_current_action(&face, show, &interaction),
        Ok(patchbay_control::PatchbayAction::Stop)
    );
    let mut forged = interaction.clone();
    forged.sequence += 1;
    assert_eq!(
        resolve_current_action(&face, show, &forged),
        Err("workspace-face-interaction-invalid")
    );
    let mut forged = interaction.clone();
    forged
        .arguments
        .push(conduit_face::FaceInteractionArgument {
            name: "invented".into(),
            value_kind: "value/text".into(),
            value: alloc::vec![0],
        });
    assert_eq!(
        resolve_current_action(&face, show, &forged),
        Err("workspace-face-interaction-invalid")
    );
    let failed = show
        .fail(
            conduit_face::ManifestationFailure::AdapterUnavailable,
            conduit_core::SignId::from("fixture/lost"),
        )
        .unwrap();
    assert_eq!(
        resolve_current_action(&face, &failed, &interaction),
        Err("workspace-face-failed-show")
    );
}

#[test]
fn replaced_show_and_changed_unobserved_owner_refuse_before_effect() {
    let (mut journey, mut door, mut workspace, mut display) = encounter();
    let face = door.face().unwrap();
    let old = workspace.selected_show.clone().unwrap();
    let interaction = stop(&face, &old);
    workspace.present_current(&door, &mut display).unwrap();
    let (ids, offer, _) = fixture();
    let make = MakeRecord {
        build_id: "build",
        ..MakeRecord::legacy()
    };
    let before = journey.projection();
    assert_eq!(
        workspace.apply_interaction(
            &mut door,
            &mut journey,
            &mut display,
            &ids,
            &offer,
            &make,
            &old,
            &interaction
        ),
        Err("workspace-face-show-not-selected")
    );
    assert_eq!(journey.projection(), before);
    let current = workspace.selected_show.clone().unwrap();
    let interaction = stop(&face, &current);
    journey.select_next_plot(journey.revision()).unwrap();
    let changed = journey.projection();
    assert_eq!(
        workspace.apply_interaction(
            &mut door,
            &mut journey,
            &mut display,
            &ids,
            &offer,
            &make,
            &current,
            &interaction
        ),
        Err("workspace-face-owner-changed")
    );
    assert_eq!(journey.projection(), changed);
}

#[test]
fn admitted_stop_is_once_only_and_keeps_body_identity() {
    let (mut journey, mut door, mut workspace, mut display) = encounter();
    let show = workspace.selected_show.clone().unwrap();
    let interaction = stop(&door.face().unwrap(), &show);
    let before = journey.projection();
    let (ids, offer, _) = fixture();
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
            &interaction,
        )
        .unwrap();
    let after = journey.projection();
    assert_eq!(after.body_id, before.body_id);
    assert_eq!(after.born_sign_id, before.born_sign_id);
    assert_eq!(after.status, crate::product_journey::JourneyStatus::Stopped);
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
            &interaction
        ),
        Err("workspace-face-show-not-selected")
    );
    assert_eq!(journey.projection(), after);
}

#[test]
fn failed_replacement_retires_the_old_control_before_scanout() {
    struct LostDisplay;
    impl PixelTarget for LostDisplay {
        fn format(&self) -> DisplayFormat {
            CountingDisplay::default().format()
        }
        fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
            Err(DisplayError::Lost)
        }
    }
    let (journey, door, mut workspace, _display) = encounter();
    let before = journey.projection();
    assert!(workspace.present_current(&door, &mut LostDisplay).is_err());
    assert!(workspace.selected_show.is_none());
    assert!(workspace.mask.show().is_none());
    assert!(workspace.route_keyboard().is_err());
    assert_eq!(journey.projection(), before);
}

#[test]
fn context_revision_target_and_availability_use_shared_admission() {
    let (_journey, door, workspace, _display) = encounter();
    let face = door.face().unwrap();
    let show = workspace.selected_show.as_ref().unwrap();
    let interaction = stop(&face, show);
    let mut wrong = interaction.clone();
    wrong.target = "host/another".into();
    assert_eq!(
        resolve_current_action(&face, show, &wrong),
        Err("workspace-face-action-target-mismatch")
    );
    let mut wrong = interaction.clone();
    wrong.action_id = "invented/action".into();
    assert_eq!(
        resolve_current_action(&face, show, &wrong),
        Err("workspace-face-action-unknown")
    );
    let mut wrong = interaction.clone();
    wrong.face_revision += 1;
    assert_eq!(
        resolve_current_action(&face, show, &wrong),
        Err("workspace-face-stale-face")
    );
    let context = face
        .clone()
        .with_interaction_context(conduit_face::FaceInteractionContext {
            identity: "conduitos/another-context".into(),
            basis: alloc::vec![],
        })
        .unwrap();
    assert_eq!(
        resolve_current_action(&context, show, &interaction),
        Err("workspace-face-stale-face")
    );
    let mut wrong = interaction;
    wrong.action_id = face
        .actions
        .iter()
        .find(|action| action.intent == patchbay_control::PatchbayAction::Wake.face_intent())
        .unwrap()
        .identity
        .clone();
    assert_eq!(
        resolve_current_action(&face, show, &wrong),
        Err("workspace-face-action-unavailable")
    );
}
