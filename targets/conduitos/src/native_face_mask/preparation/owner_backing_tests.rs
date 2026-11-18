//! Actual checked component Plans exercise owned backing, not factory peaks.
use conduit_conduitos_mask_offer::{
    NativeMaskStorageProfile, native_layout_host_advertisement, prepare_layout_component_document,
};
use conduit_face::{
    AmbientClockEncounter, ConfiguredMaskConfiguration, PlannedMaskPlot, ambient_clock_mask_source,
    configured_mask_configuration_backing_bytes, native_mask_plan_backing_bytes,
};
use core::mem::size_of;

fn prepared(encounter: AmbientClockEncounter) -> (PlannedMaskPlot, ConfiguredMaskConfiguration) {
    let provider = crate::product_bases::fixture_surface_provider();
    let offer = native_layout_host_advertisement(
        &"host/configured-backing".into(),
        &"boot/configured-backing".into(),
        1,
        &provider.entry,
        NativeMaskStorageProfile::Standard,
    );
    let source =
        ambient_clock_mask_source("clock/scoped", "weather/scoped", "image/scoped").unwrap();
    let (stage, configuration) = prepare_layout_component_document(
        &offer,
        &source,
        encounter.root(),
        NativeMaskStorageProfile::Standard,
    )
    .unwrap();
    (stage.planned_mask, configuration)
}

#[test]
fn both_checked_contexts_have_complete_owned_payload_descriptors() {
    for encounter in [
        AmbientClockEncounter::Tile,
        AmbientClockEncounter::Fullscreen,
    ] {
        let (plan, configuration) = prepared(encounter);
        assert!(!plan.plan.realization_backs.is_empty());
        assert!(configuration.layout.is_some());
        assert!(native_mask_plan_backing_bytes(&plan).unwrap() > 0);
        let configuration_bytes =
            configured_mask_configuration_backing_bytes(&configuration).unwrap();
        let values = configuration.recipe.owned_backing_bytes().unwrap()
            + configuration.look.owned_backing_bytes().unwrap()
            + configuration
                .layout
                .as_ref()
                .unwrap()
                .owned_backing_bytes()
                .unwrap();
        assert!(configuration_bytes > values);
    }
}

#[test]
fn configured_plan_counts_provenance_spares_and_refuses_other_graph_shapes() {
    let (mut plan, _) = prepared(AmbientClockEncounter::Fullscreen);
    let before = native_mask_plan_backing_bytes(&plan).unwrap();
    let path = &mut plan.plan.realization_backs[0].invocation_path;
    let capacity = path.capacity();
    path.reserve(73);
    let additional = path.capacity() - capacity;
    assert_eq!(
        native_mask_plan_backing_bytes(&plan),
        Some(before + additional)
    );
    let before = native_mask_plan_backing_bytes(&plan).unwrap();
    let capacity = plan.plan.activations.capacity();
    plan.plan.activations.reserve(7);
    let additional = (plan.plan.activations.capacity() - capacity)
        * size_of::<conduit_core::PlannedActivationEntry>();
    assert_eq!(
        native_mask_plan_backing_bytes(&plan),
        Some(before + additional)
    );
    // A second fragment is deliberately outside this descriptor's closed
    // profile, even when its copied fields appear familiar.
    plan.plan.fragments.push(plan.plan.fragments[0].clone());
    assert_eq!(native_mask_plan_backing_bytes(&plan), None);
}

#[test]
fn configuration_counts_owned_canonical_and_invocation_spares() {
    let (_, mut configuration) = prepared(AmbientClockEncounter::Tile);
    let before = configured_mask_configuration_backing_bytes(&configuration).unwrap();
    let old_value_bytes = configuration.recipe.owned_backing_bytes().unwrap();
    let mut canonical =
        alloc::vec::Vec::with_capacity(configuration.recipe.canonical_value().len() + 83);
    canonical.extend_from_slice(configuration.recipe.canonical_value());
    let replacement = conduit_core::StructuredConfigurationValue::new(
        configuration.recipe.profile().clone(),
        canonical,
    )
    .unwrap();
    let new_value_bytes = replacement.owned_backing_bytes().unwrap();
    configuration.recipe = replacement;
    assert_eq!(
        configured_mask_configuration_backing_bytes(&configuration),
        Some(before - old_value_bytes + new_value_bytes)
    );
    let before = configured_mask_configuration_backing_bytes(&configuration).unwrap();
    let capacity = configuration.invocation_path.capacity();
    configuration.invocation_path.reserve(9);
    let additional =
        (configuration.invocation_path.capacity() - capacity) * size_of::<alloc::string::String>();
    assert_eq!(
        configured_mask_configuration_backing_bytes(&configuration),
        Some(before + additional)
    );
}
