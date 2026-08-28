#[test]
fn universal_planner_has_no_human_media_production_dependency_or_api() {
    let manifest = include_str!("../Cargo.toml");
    let production_dependencies = manifest
        .split_once("[dependencies]")
        .expect("planner declares production dependencies")
        .1
        .split_once("[dev-dependencies]")
        .expect("planner declares development dependencies")
        .0;
    assert!(!production_dependencies.contains("conduit-human"));

    let source = include_str!("../src/lib.rs");
    for forbidden in [
        "human_media",
        "plan_media_acquisition",
        "select_acquired_media",
    ] {
        assert!(
            !source.contains(forbidden),
            "human media API returned to universal planner: {forbidden}"
        );
    }
}
