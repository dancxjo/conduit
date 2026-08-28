#[test]
fn production_ai_provider_does_not_depend_on_semantic_catalog() {
    let manifest = include_str!("../Cargo.toml");
    let production_dependencies = manifest
        .split_once("[dependencies]")
        .expect("AI manifest declares production dependencies")
        .1
        .split_once("[features]")
        .expect("AI manifest declares features")
        .0;

    assert!(production_dependencies.contains("conduit-web"));
    assert!(!production_dependencies.contains("conduit-semantic-catalog"));
}
