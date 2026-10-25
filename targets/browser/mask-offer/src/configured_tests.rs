use super::*;
use conduit_core::*;
use conduit_face::*;

#[test]
fn actual_checked_todo_roots_use_exact_configured_offer_and_source_recipe() {
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser/component"),
        boot_id: BootId::from("browser/component/boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser"),
        bases: vec![],
        resources: vec![resource_offer("browser/face", FACE_RESOURCE_CLASS, 1)],
        planner_capabilities: vec![],
        capabilities: vec![configured_offer()],
    };
    for recipe in [TodoComponentRecipe::Flat, TodoComponentRecipe::Grouped] {
        let compact = planned_mask(
            &host,
            TODO_COMPONENT_MASK_SOURCE,
            todo_component_mask_root(ComponentMaskLook::Compact, recipe),
        )
        .unwrap();
        let spacious = planned_mask(
            &host,
            TODO_COMPONENT_MASK_SOURCE,
            todo_component_mask_root(ComponentMaskLook::Spacious, recipe),
        )
        .unwrap();
        assert_ne!(compact.plan.plan_id, spacious.plan.plan_id);
        let compact_renderer = &compact.plan.fragments[0].placements[0];
        let spacious_renderer = &spacious.plan.fragments[0].placements[0];
        assert_eq!(
            compact_renderer.capability_id,
            configured_offer().capability_id
        );
        assert_eq!(
            compact_renderer
                .configuration
                .iter()
                .find(|field| field.key == "recipe"),
            spacious_renderer
                .configuration
                .iter()
                .find(|field| field.key == "recipe")
        );
        assert_ne!(
            compact_renderer
                .configuration
                .iter()
                .find(|field| field.key == "look"),
            spacious_renderer
                .configuration
                .iter()
                .find(|field| field.key == "look")
        );
    }
}
