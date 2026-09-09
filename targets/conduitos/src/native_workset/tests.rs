use super::*;
use crate::{
    identity::BootIdentities,
    keyboard_offer::{KeyboardMechanism, KeyboardRealization},
    offer::{CpuFeatures, HostOffer},
};
use conduit_body::{Body, BodyWorkset, Wake};

pub(super) fn fixture() -> (BootIdentities, HostOffer<'static>) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        1_048_576,
    )
    .with_keyboard(
        KeyboardRealization {
            mechanism: KeyboardMechanism::UsbHid,
            controller_id: [3; 32],
            device_id: [4; 32],
            interface_id: [5; 32],
            endpoint_id: [6; 32],
            report_buffers: 2,
            transition_slots: 8,
            operation_slots: 2,
        },
        "build",
    )
    .unwrap();
    (identities, offer)
}

pub(super) fn wake(forms: &[NativeForm]) -> Wake {
    let workset =
        BodyWorkset::from_forms(forms.iter().map(|form| resident(*form).unwrap())).unwrap();
    Body::born_with_forms(workset, 1, "sign/born".into())
        .unwrap()
        .wake(2, "sign/woke".into())
        .unwrap()
        .1
}

#[test]
fn native_inventory_preserves_existing_canvas_and_canonical_memory_identities() {
    let old = crate::keyboard_text_plan::checked_form_identity().unwrap();
    let canvas = resident(NativeForm::KeyboardCanvas).unwrap();
    assert_eq!(canvas.source_document_id, old.source_document_id);
    assert_eq!(canvas.checked_form_id, old.checked_form_id);
    assert_ne!(canvas, resident(NativeForm::MemoryLantern).unwrap());
    assert_eq!(
        NativeForm::MemoryLantern.source(),
        include_str!("../../../../forms/memory-lantern/main.conduit")
    );
    assert_eq!(
        catalog::resolve(&canvas).unwrap(),
        NativeForm::KeyboardCanvas
    );
    let mut substituted = canvas;
    substituted.checked_form_id = "checked/substituted".into();
    assert_eq!(
        catalog::resolve(&substituted),
        Err(WorksetRefusal::UnknownForm)
    );
}

#[test]
fn canonical_memory_plans_as_an_exact_native_body_partition_before_play() {
    let (ids, offer) = fixture();
    let wake = wake(&[NativeForm::MemoryLantern]);
    let prepared = prepare(&wake, &ids, &offer, "build").unwrap();
    prepared.plan.validate_for(&wake).unwrap();
    assert_eq!(prepared.plan.forms.len(), 1);
    assert_eq!(prepared.lowered.nodes, 4);
    assert_eq!(prepared.lowered.cords, 3);
    assert!(
        prepared.plan.forms[0].plan.fragments[0]
            .placements
            .iter()
            .any(|gear| gear.implementation_id.as_str() == text_state::TEXT_EDIT_IMPLEMENTATION)
    );
    assert_eq!(
        prepare(&wake, &ids, &offer, "wrong-build").err(),
        Some(WorksetRefusal::Host)
    );
}

#[test]
fn two_forms_reserve_distinct_deliveries_from_one_initialized_keyboard() {
    let (ids, offer) = fixture();
    let wake = wake(&inventory());
    let prepared = prepare(&wake, &ids, &offer, "build").unwrap();
    assert_eq!(prepared.lowered.nodes, 8);
    assert_eq!(prepared.plan.forms.len(), 2);
    assert_eq!(prepared.keyboard, offer.keyboard.unwrap().realization);
    for form in &prepared.plan.forms {
        let keyboard = form.plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::KEYBOARD_KIND)
            .unwrap();
        assert_eq!(
            keyboard.implementation_id.as_str(),
            keyboard_delivery::IMPLEMENTATION
        );
        assert!(
            keyboard
                .resources
                .iter()
                .any(
                    |resource| resource.class_id.as_str() == keyboard_delivery::RESOURCE
                        && resource.units == 1
                )
        );
        assert!(
            !keyboard.resources.iter().any(
                |resource| resource.class_id.as_str() == crate::keyboard_offer::DEVICE_RESOURCE
            )
        );
    }
    assert!(
        prepared
            .advertisement
            .resources
            .iter()
            .filter(|resource| resource.class_id.as_str() == crate::keyboard_offer::DEVICE_RESOURCE)
            .all(|resource| resource.capacity_units == 1)
    );
}
