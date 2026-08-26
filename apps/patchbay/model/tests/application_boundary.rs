#[test]
fn production_model_dependencies_exclude_the_concrete_std_host() {
    let manifest = include_str!("../Cargo.toml");
    let production_dependencies = manifest
        .split_once("[dependencies]")
        .expect("model manifest must declare production dependencies")
        .1
        .split_once("[dev-dependencies]")
        .expect("model manifest must fence test-only dependencies")
        .0;

    assert!(
        !production_dependencies.contains("conduit-std-host"),
        "Patchbay's reusable model must receive Host truth through its application adapter"
    );
}
