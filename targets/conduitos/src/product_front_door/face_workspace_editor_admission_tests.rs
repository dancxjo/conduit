//! Deterministic native admission regression over a real planned editor execution.
//! This uses the ordinary owner input and Mask submission paths, not QEMU evidence.
use super::tests::encounter;
use super::*;
use crate::{
    native_workset::{self, NativePlot},
    product_journey::{
        JourneyStatus,
        test_support::{fixture, key},
    },
};
use conduit_human::{KeyModifiers, KeyTransition};

#[test]
fn actual_editor_commit_retires_old_lifecycle_show_then_fresh_stop_preserves_body() {
    let (mut journey, mut door, mut workspace, mut display) = encounter();
    let editor = native_workset::resident(NativePlot::MemoryLantern).unwrap();
    journey.select_plot(&editor, journey.revision()).unwrap();
    door.observe_product(&journey).unwrap();
    let original = journey.projection();
    assert!(original.text_editor_active);
    assert_eq!(original.text_editor_label.as_deref(), Some("Message"));
    workspace.present_current(&door, &mut display).unwrap();
    let face = door.face().unwrap();
    let old_show = workspace.selected_show.clone().unwrap();
    let stop = face
        .actions
        .iter()
        .find(|action| action.intent == patchbay_control::PatchbayAction::Stop.face_intent())
        .unwrap();
    let old_interaction = FaceInteraction::new(
        &face,
        &old_show,
        &stop.identity,
        &stop.target,
        alloc::vec![],
        1,
    )
    .unwrap();

    // Commit actual input to the admitted editor, advancing its owner Face.
    for transition in [KeyTransition::Pressed, KeyTransition::Released] {
        crate::product_front_door::workspace_input::accept(
            key(4, transition),
            &mut journey,
            &mut door,
        )
        .unwrap();
    }
    let edited = journey.projection();
    assert_eq!(edited.result.as_deref(), Some("a"));
    assert!(edited.text_editor_active);
    assert_ne!(edited.revision, original.revision);
    assert_ne!(door.face().unwrap().identity, face.identity);
    assert_eq!(edited.body_id, original.body_id);
    assert_eq!(edited.born_sign_id, original.born_sign_id);

    // The retained Mask can return its old interaction, but the current owner
    // refuses that obsolete Face before admitting any lifecycle operation.
    let correlation = workspace.mask.submit(old_interaction).unwrap();
    let (ids, offer, _) = fixture();
    let make = MakeRecord {
        build_id: "build",
        ..MakeRecord::legacy()
    };
    assert_eq!(
        workspace.apply_interaction(
            &mut door,
            &mut journey,
            &mut display,
            &ids,
            &offer,
            &make,
            &old_show,
            &correlation.interaction,
        ),
        Err("workspace-face-stale-face")
    );
    assert_eq!(journey.projection(), edited);

    workspace.present_current(&door, &mut display).unwrap();
    let fresh_show = workspace.selected_show.as_ref().unwrap().show_id.clone();
    assert_ne!(fresh_show, old_show.show_id);
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
    let stopped = journey.projection();
    assert_eq!(stopped.status, JourneyStatus::Stopped);
    assert!(!stopped.text_editor_active);
    assert_eq!(stopped.body_id, original.body_id);
    assert_eq!(stopped.born_sign_id, original.born_sign_id);
    assert_eq!(stopped.result.as_deref(), Some("a"));
    assert_ne!(
        workspace.selected_show.as_ref().unwrap().show_id,
        fresh_show
    );
}

#[test]
fn play_from_specialized_presenter_enters_the_actual_editor_mask_surface() {
    let (ids, offer, mut journey) = fixture();
    let editor = native_workset::resident(NativePlot::MemoryLantern).unwrap();
    journey
        .birth_from_creche(conduit_birth_plot::BirthSelection {
            revision: 3,
            friendly_name: "Roseau".into(),
            workset: conduit_body::BodyWorkset::one(editor).unwrap(),
        })
        .unwrap();
    for action in [
        patchbay_control::PatchbayAction::Wake,
        patchbay_control::PatchbayAction::Plan,
    ] {
        crate::product_journey::test_support::invoke(&mut journey, action, &ids, &offer).unwrap();
    }
    let planned = journey.projection();
    assert_eq!(planned.status, JourneyStatus::Planned);
    assert!(planned.active_play_id.is_none());
    assert!(!planned.text_editor_active);
    let mut door = FrontDoor::new(
        planned.host_id.clone(),
        planned.boot_id.clone(),
        planned.offer_generation,
        "profile",
        "build",
        "image",
        planned.source_document_id.clone().unwrap(),
        planned.checked_plot_id.clone().unwrap(),
        7,
        true,
    );
    door.observe_product(&journey).unwrap();
    assert!(!door.home_open());
    assert!(!door.exact_details_open());
    let provider = crate::product_bases::fixture_surface_provider();
    let mut workspace = FaceWorkspace::prepare(
        planned.host_id.clone(),
        planned.boot_id.clone(),
        planned.offer_generation,
        "build",
        provider.entry.base_id.clone(),
        &provider,
    )
    .unwrap();
    let mut presenter = FrontDoorPresenter::prepare(
        planned.host_id.clone(),
        planned.boot_id.clone(),
        planned.offer_generation,
        "profile",
        "image",
        provider.entry.base_id.clone(),
        2,
    )
    .unwrap();
    let mut display = super::tests::CountingDisplay::default();
    presenter.present(&door, &mut display).unwrap();
    assert!(!workspace.active());
    let make = MakeRecord {
        build_id: "build",
        ..MakeRecord::legacy()
    };
    let receipt = workspace
        .invoke_from_presenter(
            patchbay_control::PatchbayAction::Play,
            &mut door,
            &mut journey,
            &mut presenter,
            &mut display,
            &ids,
            &offer,
            &make,
        )
        .unwrap();
    let playing = journey.projection();
    assert_eq!(playing.status, JourneyStatus::QuiescentAwaitingInput);
    assert!(playing.text_editor_active);
    assert_eq!(playing.body_id, planned.body_id);
    assert_eq!(playing.born_sign_id, planned.born_sign_id);
    assert!(
        workspace.active(),
        "Play must restore the editor's actual Mask"
    );
    assert!(workspace.editor_surface());
    assert!(workspace.route_keyboard().unwrap());
    let show = workspace.selected_show.as_ref().unwrap();
    let face = door.face().unwrap();
    assert_eq!(receipt.face_id, face.identity);
    assert_eq!(show.face_id, face.identity);
    assert_eq!(
        show.show.lifecycle,
        conduit_face::ManifestationLifecycle::Available
    );
    let frame = workspace.mask.scene().unwrap().frame().unwrap();
    assert!(
        frame
            .scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Message")
    );
    assert!(frame.scene.commands().iter().any(|command| {
        command.kind == conduit_face::GraphicsCommandKind::Rect
            && command.paint == conduit_face::GraphicsPaintRole::Focus
            && command.bounds.width == 2
    }));

    // A refused repeated Play must preserve its reason on the specialized
    // surface instead of automatically reentering the editor and hiding it.
    workspace
        .leave(&door, &mut presenter, &mut display)
        .unwrap();
    let before_refusal = journey.projection();
    workspace
        .invoke_from_presenter(
            patchbay_control::PatchbayAction::Play,
            &mut door,
            &mut journey,
            &mut presenter,
            &mut display,
            &ids,
            &offer,
            &make,
        )
        .unwrap();
    assert_eq!(journey.projection(), before_refusal);
    assert!(
        !workspace.active(),
        "Unavailable Play must retain the visible refusal surface"
    );
    assert!(!workspace.editor_surface());
    assert!(workspace.selected_show.is_none());
    assert!(matches!(
        presenter.route_keyboard().unwrap(),
        crate::native_compositor::InputRoute::Delivered(_)
    ));
    let refused_face = door.face().unwrap();
    let reason = refused_face
        .properties
        .iter()
        .find_map(|property| match (&property.name[..], &property.value) {
            ("action-refusal", conduit_face::FacePropertyValue::Text(reason)) => Some(reason),
            _ => None,
        })
        .expect("source Face retains the actual unavailable Play reason");
    assert!(reason.contains("Play is not available"));
    let scene = door.scene(&display).unwrap();
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload() == "Action refused. Details:")
    );
    assert!(
        scene
            .commands()
            .iter()
            .any(|command| command.payload() == reason
                && command.bounds.y >= command.clip.y
                && i32::from(command.bounds.y)
                    < i32::from(command.clip.y) + i32::from(command.clip.height))
    );
}

#[test]
fn held_editor_release_before_service_refuses_old_mask_enter_and_recovers() {
    held_release_before_service(4, "a");
}

#[test]
fn held_modifier_release_before_service_refuses_old_mask_enter_and_recovers() {
    held_release_before_service(225, "");
}

fn held_release_before_service(usage: u8, expected_text: &str) {
    let held_key = |code, transition| {
        KeyEvent::new(
            code,
            transition,
            if usage == 225 {
                KeyModifiers::LEFT_SHIFT
            } else {
                KeyModifiers::from_bits(0)
            },
        )
        .unwrap()
    };
    let (mut journey, mut door, mut workspace, mut display) = encounter();
    let editor = native_workset::resident(NativePlot::MemoryLantern).unwrap();
    journey.select_plot(&editor, journey.revision()).unwrap();
    door.observe_product(&journey).unwrap();
    // Ordinary F2 inspection has no automatic editor-surface Tab binding.
    workspace.present_current(&door, &mut display).unwrap();
    assert!(!workspace.editor_surface());
    let (ids, offer, _) = fixture();
    let make = MakeRecord {
        build_id: "build",
        ..MakeRecord::legacy()
    };
    workspace
        .accept_key(
            held_key(usage, KeyTransition::Pressed),
            &mut door,
            &mut journey,
            &mut display,
            &ids,
            &offer,
            &make,
        )
        .unwrap();
    assert!(journey.owns_key_release(key(usage, KeyTransition::Released)));
    assert_eq!(journey.projection().result.as_deref(), Some(expected_text));
    for transition in [KeyTransition::Pressed, KeyTransition::Released] {
        workspace
            .accept_key(
                held_key(59, transition),
                &mut door,
                &mut journey,
                &mut display,
                &ids,
                &offer,
                &make,
            )
            .unwrap();
    }
    assert!(workspace.mask.scene().unwrap().showing_details());
    let mut stop_focused = false;
    for _ in 0..32 {
        for transition in [KeyTransition::Pressed, KeyTransition::Released] {
            workspace
                .accept_key(
                    held_key(43, transition),
                    &mut door,
                    &mut journey,
                    &mut display,
                    &ids,
                    &offer,
                    &make,
                )
                .unwrap();
        }
        let scene = workspace.mask.scene().unwrap();
        if scene.focused().is_some_and(|control| {
            scene.face().actions[control.action].intent
                == patchbay_control::PatchbayAction::Stop.face_intent()
        }) {
            stop_focused = true;
            break;
        }
    }
    assert!(stop_focused, "ordinary Mask navigation must focus Stop");
    let old_show = workspace.selected_show.clone().unwrap();
    // The production USB report drains releases before presses, with Service
    // deferred until the queue is idle. This is the same release/Enter window.
    let mut pending = crate::product_front_door::workspace_input::PendingInput::default();
    pending
        .accept(key(usage, KeyTransition::Released), &mut journey, &mut door)
        .unwrap();
    let after_release = journey.projection();
    assert_ne!(door.face().unwrap().identity, old_show.face_id);
    let result = workspace.accept_key(
        key(40, KeyTransition::Pressed),
        &mut door,
        &mut journey,
        &mut display,
        &ids,
        &offer,
        &make,
    );
    assert!(
        result.is_ok(),
        "stale real Mask submission must recover: {result:?}"
    );
    assert_eq!(journey.projection(), after_release);
    let fresh = workspace
        .selected_show
        .as_ref()
        .expect("refusal must publish a fresh Show");
    assert_ne!(fresh.show_id, old_show.show_id);
    assert_eq!(fresh.face_id, door.face().unwrap().identity);
    assert!(workspace.route_keyboard().unwrap());
    assert!(door.face().unwrap().properties.iter().any(|property| {
        property.name == "action-refusal"
            && matches!(&property.value, conduit_face::FacePropertyValue::Text(text)
                if text == "workspace-face-stale-face")
    }));
    // A deferred Service may re-present the same owner. Its notice must stay
    // visible instead of disappearing between the refusal and the capture.
    workspace.present_current(&door, &mut display).unwrap();
    assert!(
        workspace
            .mask
            .scene()
            .unwrap()
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|command| command.payload().contains("workspace-face-stale-face"))
    );
    assert_eq!(journey.projection(), after_release);
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
    let stopped = journey.projection();
    assert_eq!(stopped.status, JourneyStatus::Stopped);
    assert_eq!(stopped.body_id, after_release.body_id);
    assert_eq!(stopped.born_sign_id, after_release.born_sign_id);
    assert_eq!(stopped.result, after_release.result);
}
