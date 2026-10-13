use std::fs;

#[test]
fn bitmap_face_meaning_has_one_host_neutral_owner() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("face owner declares dependencies")
        .1
        .split_once("[dev-dependencies]")
        .expect("face owner declares dev dependencies")
        .0;
    for forbidden in ["conduit-semantic-catalog", "conduit-std-host", "targets/"] {
        assert!(!dependencies.contains(forbidden));
    }

    let former_owner = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../catalog/src/graphics_face.rs"
    );
    let source = fs::read_to_string(former_owner).expect("read former bitmap catalog owner");
    for forbidden in [
        "pub const BITMAP_FACE_KIND",
        "pub const BITMAP_FACE_REVISION",
        "install_bitmap_face_catalog",
    ] {
        assert!(
            !source.contains(forbidden),
            "portable bitmap catalog truth returned to std ownership: {forbidden}"
        );
    }
}

#[test]
fn bitmap_face_identity_and_bound_remain_exact() {
    assert_eq!(conduit_face::BITMAP_FACE_KIND, "face/bitmap");
    assert_eq!(conduit_face::BITMAP_FACE_REVISION, "conduit.face/bitmap@1");
    let definition = conduit_face::bitmap_face_definition();
    assert_eq!(definition.inputs.len(), 1);
    assert!(definition.outputs.is_empty());
    assert_eq!(
        definition.inputs[0].value_kind.as_str(),
        conduit_face::GRAY8_BITMAP_INFO_KIND
    );
}
