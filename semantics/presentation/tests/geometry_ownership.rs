use std::{fs, path::Path};

#[test]
fn portable_geometry_has_one_host_neutral_owner() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("presentation owner declares dependencies")
        .1
        .split_once("[dev-dependencies]")
        .expect("presentation owner declares dev dependencies")
        .0;
    for forbidden in ["conduit-semantic-catalog", "conduit-std-host", "targets/"] {
        assert!(!dependencies.contains(forbidden));
    }

    let old_owner = Path::new(env!("CARGO_MANIFEST_DIR")).join("../catalog/src");
    for entry in fs::read_dir(&old_owner).expect("read former owner source directory") {
        let path = entry.expect("read former owner source entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        assert!(
            !name.starts_with("geometry"),
            "portable geometry returned to std ownership: {}",
            path.display()
        );
    }
}

#[test]
fn portable_geometry_identities_and_bounds_remain_exact() {
    assert_eq!(
        conduit_presentation::GEOMETRY_REVISION,
        "conduit.std/geometry-spatial@1"
    );
    assert_eq!(conduit_presentation::POINT2_LITERAL_KIND, "geometry/point2");
    assert_eq!(
        conduit_presentation::APPLY_TRANSFORM2_KIND,
        "geometry/apply-transform2"
    );
    assert_eq!(conduit_presentation::MAXIMUM_GEOMETRY_PATH_POINTS, 64);
    assert_eq!(conduit_presentation::geometry_types().len(), 10);
}
