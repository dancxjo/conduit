//! Actual Todo projector plus checked Source/Plan; no installed owner effect claim.
use super::*;
use conduit_core::*;
use conduit_face::*;
#[path = "../../../../../plots/todo/face/tests/common/face.rs"]
mod todo;

#[test]
fn required_item_roots_bind_the_real_todo_slot_and_exact_back_in_both_looks() {
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser/required-item"),
        boot_id: BootId::from("boot/required-item"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/required-item"),
        bases: vec![],
        resources: vec![resource_offer("browser/face", FACE_RESOURCE_CLASS, 1)],
        planner_capabilities: vec![],
        capabilities: vec![owner_component_offer().unwrap()],
    };
    let face = todo::face(1, 1, true);
    let unchanged = face.clone();
    let mut plans = Vec::new();
    for look in [ComponentMaskLook::Compact, ComponentMaskLook::Spacious] {
        let (planned, configuration) = crate::planned_mask_configuration(
            &host,
            TODO_REQUIRED_ITEM_COMPONENT_SOURCE,
            todo_required_item_mask_root(look),
            Some(crate::OwnerCarrierForeLimits {
                face_bytes: 60 * 1024,
                show_bytes: 60 * 1024,
            }),
        )
        .unwrap();
        let configuration = configuration.unwrap();
        assert_eq!(
            crate::configured_mask_configuration(&host, &planned).unwrap(),
            Some(configuration.clone())
        );
        let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .unwrap();
        let prepared = prepare_component_recipe(recipe.clone(), &face).unwrap();
        let item = face
            .subjects
            .iter()
            .position(|subject| subject.identity == "todo/item/task-1")
            .unwrap();
        assert_eq!(prepared.subject_regions[item], Some(0));
        assert_eq!(prepared.reading, plan_face_utterances(&face).unwrap());
        assert_eq!(face, unchanged);
        assert_eq!(
            planned.show_placement().artifact_id,
            owner_component_offer().unwrap().implementation.artifact_id
        );
        // The one-way Source action disappears in the completed projection.
        // Its required contract must refuse; the enclosing renderer separately
        // exposes its declared generic-reading fallback, never a forged slot.
        assert_eq!(
            prepare_component_recipe(recipe, &todo::face(0, 1, true)).err(),
            Some(ComponentRecipeRefusal::SlotAction)
        );
        plans.push(planned.plan.plan_id);
    }
    assert_ne!(plans[0], plans[1]);
    assert!(crate::planned_mask(
        &host,
        TODO_REQUIRED_ITEM_COMPONENT_SOURCE,
        "todo-mask-required-item-unknown"
    )
    .is_err());
}
