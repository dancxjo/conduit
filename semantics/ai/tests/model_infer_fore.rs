use conduit_ai::model_infer_catalog::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
#[path = "common/model_signature_fixture.rs"]
mod fixture;
const SOURCE: &str = include_str!("fixtures/onnx-model-fore.conduit");

#[test]
fn ordinary_source_retains_semantic_signature_and_one_opaque_model_gear() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_model_infer_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(|e| format!("{}: {}", e.code, e.message))
        .expect("ordinary model Source checks");
    let authored = expand_canonical_plot_for_authoring(&checked, "tiny-model", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    let gear = &authored.expanded.gears[0];
    assert_eq!(gear.kind_id.as_str(), MODEL_INFER_KIND);
    assert_eq!(
        configured_model_signature(&gear.configuration).unwrap(),
        fixture::signature()
    );
    assert_eq!(gear.inputs.len(), 1);
    assert_eq!(gear.outputs.len(), 1);
}

#[test]
fn required_signature_and_native_field_bounds_are_checked_in_source() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_model_infer_catalog(&mut startup, &mut profile).unwrap();
    let absent = "plot missing {\n model: model/infer\n}\n";
    assert!(check_syntax_document(&parse_syntax_document(absent), &startup).is_err());
    let zero_version = SOURCE.replace("compatibility_version: 1", "compatibility_version: 0");
    assert!(check_syntax_document(&parse_syntax_document(&zero_version), &startup).is_err());
    let unknown = SOURCE.replace(
        "compatibility_version: 1",
        "unknown_field: 1, compatibility_version: 1",
    );
    assert!(check_syntax_document(&parse_syntax_document(&unknown), &startup).is_err());
}
