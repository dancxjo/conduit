//! Checked build material initializes software; physical possession remains absent.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
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
#[allow(
    dead_code,
    reason = "the hardware consumer also uses sign and native-call pins"
)]
#[path = "../../../../targets/esp32/proof/tm1637/src/appliance_layout.rs"]
mod firmware_layout;

#[test]
fn firmware_sampler_layout_matches_the_current_checked_source() {
    let source = conduit_environment_runtime::construction::compile_default().unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    assert!(firmware_layout::layout(recipe.spec().source).is_ok());
}

#[test]
fn indexed_make_source_reconstructs_the_exact_checked_appliance_without_a_plan() {
    let source = conduit_environment_runtime::construction::compile_default().unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    // SAFETY: the bytes were produced from this independently checked Source
    // by the trusted Make encoder immediately above, not an incoming document.
    let view = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            recipe.planning_image(),
        )
    }
    .unwrap();
    assert_eq!(view.gear_count(), source.expanded.gears.len());
    let (identity, identity_memory) = allocation_probe::observe(|| view.identity().unwrap());
    assert_eq!(identity.expanded_plot_id, source.expanded.expanded_plot_id);
    assert!(identity_memory.peak_bytes <= 1024);
    eprintln!(
        "indexed_source_identity_peak={}",
        identity_memory.peak_bytes
    );
    let (clone, graph_memory) = allocation_probe::observe(|| source.expanded.clone());
    drop(clone);
    let (metadata, metadata_memory) = allocation_probe::observe(|| view.metadata().unwrap());
    let mut largest_gear_peak = 0;

    assert_eq!(
        metadata.identity().expanded_plot_id,
        source.expanded.expanded_plot_id
    );
    let gears = (0..view.gear_count())
        .map(|i| {
            let (gear, memory) = allocation_probe::observe(|| view.gear(i).unwrap());
            largest_gear_peak = largest_gear_peak.max(memory.peak_bytes);
            assert_eq!(gear, source.expanded.gears[i]);
            gear
        })
        .collect();
    assert_eq!(metadata.into_expanded(gears).unwrap(), source.expanded);
    assert!(largest_gear_peak < graph_memory.peak_bytes);
    assert!(metadata_memory.peak_bytes < graph_memory.peak_bytes);
    eprintln!(
        "indexed_source_metadata_peak={} largest_gear_peak={} whole_graph_clone_peak={}",
        metadata_memory.peak_bytes, largest_gear_peak, graph_memory.peak_bytes
    );
    assert!(view.gear(view.gear_count()).is_err());
    eprintln!(
        "indexed_appliance_source_bytes={}",
        recipe.planning_image().len()
    );
}
