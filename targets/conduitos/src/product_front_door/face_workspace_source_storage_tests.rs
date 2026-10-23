//! Actual native actor -> owner Face -> selected storage -> software scanout.
//! This uses the ordinary code with a counting display, not QEMU/physical proof.
use super::*;
use crate::native_workset::{self, NativePlot};
use conduit_conduitos_mask_offer::NativeMaskStorageProfile;
use conduit_face::FacePropertyValue;

#[test]
fn checked_source_workspace_uses_explicit_collection_storage_without_replanning_body() {
    exercise(NativeMaskStorageProfile::Collection);
}

#[test]
fn standard_workspace_refuses_larger_source_and_retires_prior_show() {
    exercise(NativeMaskStorageProfile::Standard);
}

fn exercise(storage: NativeMaskStorageProfile) {
    let (mut journey, mut door, mut workspace, mut display) =
        tests::encounter_with_storage(storage);
    journey
        .select_plot(
            &native_workset::resident(NativePlot::Patchbay).unwrap(),
            journey.revision(),
        )
        .unwrap();
    door.observe_product(&journey).unwrap();
    workspace.present_current(&door, &mut display).unwrap();
    let face = door.face().unwrap();
    let show = workspace.selected_show.clone().unwrap();
    let action = face
        .actions
        .iter()
        .find(|action| {
            action.intent == "patchbay/select-plot@1"
                && face.properties.iter().any(|property| {
                    property.subject == action.target
                        && property.name == "inspected-plan-present"
                        && property.value == FacePropertyValue::Flag(false)
                })
        })
        .unwrap();
    let source_target = action.target.clone();
    let input = FaceInteraction::new(
        &face,
        &show,
        &action.identity,
        &action.target,
        alloc::vec![],
        1,
    )
    .unwrap();
    let delivered = workspace.mask.submit(input).unwrap().interaction;
    let before = journey.projection();
    let (ids, offer, _) = crate::product_journey::test_support::fixture();
    let make = MakeRecord {
        build_id: "build",
        ..MakeRecord::legacy()
    };
    let result = workspace.apply_interaction(
        &mut door,
        &mut journey,
        &mut display,
        &ids,
        &offer,
        &make,
        &show,
        &delivered,
    );
    let selected = door.face().unwrap();
    let bytes = serde_json::to_vec(&selected).unwrap().len();
    assert!(bytes > crate::native_face_snapshot::MAX_SNAPSHOT_BYTES);
    assert!(bytes < crate::native_face_snapshot::COLLECTION_SNAPSHOT_BYTES);
    let after = journey.projection();
    assert_eq!(after.body_id, before.body_id);
    assert_eq!(after.plan_id, before.plan_id);
    assert_eq!(after.active_play_id, before.active_play_id);
    assert!(
        selected
            .properties
            .iter()
            .any(|property| property.subject == source_target
                && property.name == "patchbay-selected"
                && property.value == FacePropertyValue::Flag(true))
    );
    assert!(
        !selected
            .actions
            .iter()
            .any(|action| action.intent == "patchbay/edit-subject@1")
    );
    match storage {
        NativeMaskStorageProfile::Standard => {
            assert_eq!(result.unwrap_err(), "native-face-mask-producer-capacity");
            assert!(workspace.mask.show().is_none());
            assert!(workspace.mask.publication().is_none());
            assert!(workspace.mask.route_keyboard().is_err());
        }
        NativeMaskStorageProfile::Collection => {
            result.unwrap();
            let receipt = workspace.mask.publication().unwrap();
            assert_eq!(receipt.encoded_bytes as usize, bytes);
            let budget = receipt.collection_storage.as_ref().unwrap();
            assert_eq!(
                budget.maximum_value_bytes as usize,
                crate::native_face_snapshot::COLLECTION_SNAPSHOT_BYTES
            );
            assert_eq!(budget.value_slots, 2);
            assert_eq!(workspace.mask.show().unwrap().face_id, selected.identity);
            assert!(workspace.mask.route_keyboard().unwrap());
            assert!(workspace.mask.scene().unwrap().has_diagram());
            std::println!(
                "checked Source Face bytes={bytes}; selected maximum={}",
                budget.maximum_value_bytes
            );
        }
    }
}
