//! Actual existing firmware Make composition, executed on the building Host.
//! No device, Ready provider, membership or Start authority is constructed.
use std::{env, fs};

// Reuse the unchanged firmware builder, including its existing local style.
#[allow(dead_code, clippy::int_plus_one, clippy::needless_borrow)]
mod actual_make {
    include!("../../firmware/host/build.rs");

    pub fn prepare(
        checked: &conduit_host_make::CheckedHostConfiguration,
        catalog: &conduit_host_make::MakeCatalog,
        image_id: &str,
        source_identity: &str,
        out: &std::path::Path,
        renderer: conduit_host_rp2040::CheckedRendererConstruction,
    ) -> Vec<conduit_embedded_build::PreparedResidentExpression> {
        prepare_dht_workset(checked, catalog, image_id, source_identity, out, renderer)
    }
}
#[path = "../../../../architecture/plot/tests/common/compiled_rust.rs"]
mod compiled_rust;

#[test]
#[ignore = "requires existing public Make IMAGE and captured checked Source paths"]
fn actual_yellow_three_fragment_make_emits_canonical_seal_recipes() {
    use conduit_host_make::*;
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let source = fs::read_to_string(env::var("CONDUIT_HOST_CONFIGURATION").unwrap()).unwrap();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let renderer = conduit_host_rp2040::CheckedRendererConstruction::from_configuration(
        &checked,
        conduit_host_rp2040::RENDERER_IMPLEMENTATION,
    )
    .unwrap();
    let image: HostImage = serde_json::from_slice(
        &fs::read(env::var("CONDUIT_HOST_IMAGE_DESCRIPTION").unwrap()).unwrap(),
    )
    .unwrap();
    let out =
        std::env::temp_dir().join(format!("conduit-yellow-seal-recipe-{}", std::process::id()));
    fs::create_dir(&out).unwrap();
    let resident = actual_make::prepare(
        &checked,
        &catalog,
        &image.manifest.image_id,
        &image.manifest.source_identity,
        &out,
        renderer,
    );
    assert!(!resident.is_empty());
    let identity = fs::read_to_string(out.join("continuation_identity.rs")).unwrap();
    assert!(identity.contains("pub const WORKSET_DIGEST:"));
    assert!(identity.contains("pub const SOURCE_DIGEST:"));
    let mut entries = 0;
    let mut emitted = String::new();
    for file in fs::read_dir(&out).unwrap() {
        let file = file.unwrap();
        let name = file.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("workset_") && name.ends_with(".rs") {
            let text = fs::read_to_string(file.path()).unwrap();
            if text.contains("FRAGMENT_SEAL_RECIPE") {
                assert!(text.contains("pub const PLOT_PLAN_SEAL:"),
                    "actual entry must pass empty-tail eligibility before emitting static pins");
                assert!(text.contains("FRAGMENT_RECIPE_DIGEST"));
                assert!(text.contains("FRAGMENT_PLACEMENT_IDS"));
                emitted.push_str(&format!("mod entry_{entries} {{ {text} }}\n"));
                entries += 1;
            }
        }
    }
    assert_eq!(
        entries, 3,
        "all three ordinary sampling/Mask/output entries must emit recipes"
    );
    compiled_rust::verify_with_libraries(
        &emitted,
        "fn main() {}",
        "actual-three-fragment-seal-recipe",
        &["conduit_core", "conduit_seven_segment_mask"],
    );
    fs::remove_dir_all(&out).unwrap();
}
