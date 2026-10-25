//! Checked build material initializes software; physical possession remains absent.
use conduit_environment_make::NativeSoftwareSourceRecipe;
use conduit_environment_runtime::appliance::{
    assigned::{NativePreparationRefusal, PreparedResidentSoftware, ResidentComputation},
    prepare_value_storage,
};

#[test]
fn checked_source_initializes_retained_software_without_a_host_or_plan() {
    let source = conduit_environment_runtime::construction::compile_default().unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let spec = recipe.spec();
    let texts: Vec<_> = recipe.resident_computations().collect();
    let offers = recipe.resident_offers();
    let software =
        PreparedResidentSoftware::prepare_source(&spec, &texts, prepare_value_storage().unwrap())
            .unwrap();
    let ready = software.source_offers(&spec, &offers).unwrap();
    assert_eq!(ready.iter().len(), source.expanded.gears.len() - 2);
    for bytes in ready.iter() {
        let offer: conduit_core::CapabilityOffer = serde_json::from_slice(bytes).unwrap();
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
        assert_eq!(
            offer.implementation.artifact_id.as_str(),
            conduit_environment_runtime::appliance::software::ARTIFACT
        );
    }
    let mut foreign = recipe.spec();
    foreign.source.expanded_plot_digest[0] ^= 1;
    assert!(matches!(
        software.source_offers(&foreign, &offers),
        Err(NativePreparationRefusal::Configuration)
    ));
    // Refusal does not consume the initialized owner or its availability.
    assert!(software.source_offers(&spec, &offers).is_ok());
}

#[test]
fn corrupted_compact_program_refuses_before_software_initialization() {
    let source = conduit_environment_runtime::construction::compile_default().unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let spec = recipe.spec();
    let mut texts: Vec<_> = recipe.resident_computations().collect();
    let original = &texts[0];
    let mut bytes = original.canonical_bytes.unwrap().to_vec();
    bytes[0] ^= 1;
    texts[0] = ResidentComputation {
        node: original.node,
        key: original.key,
        text: original.text,
        canonical_bytes: Some(&bytes),
    };
    assert!(matches!(
        PreparedResidentSoftware::prepare_source(&spec, &texts, prepare_value_storage().unwrap()),
        Err(NativePreparationRefusal::Configuration)
    ));
}

// Independently linked trusted pins must match the Source produced by this
// build. A successful compiler invocation alone does not prove this relation.
#[allow(dead_code, reason = "the hardware consumer also uses sign and native-call pins")]
#[path = "../../../../targets/esp32/proof/tm1637/src/appliance_layout.rs"]
mod firmware_layout;

#[test]
fn firmware_sampler_layout_matches_the_current_checked_source() {
    let source = conduit_environment_runtime::construction::compile_default().unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    assert!(firmware_layout::layout(recipe.spec().source).is_ok());
}
