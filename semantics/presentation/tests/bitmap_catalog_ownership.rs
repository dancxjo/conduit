use std::fs;

#[test]
fn bitmap_presentation_meaning_has_one_host_neutral_owner() {
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

    let former_owner = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../catalog/src/graphics_presentation.rs"
    );
    let source = fs::read_to_string(former_owner).expect("read former bitmap catalog owner");
    for forbidden in [
        "pub const BITMAP_PRESENTATION_KIND",
        "pub const BITMAP_PRESENTATION_REVISION",
        "install_bitmap_presentation_catalog",
    ] {
        assert!(
            !source.contains(forbidden),
            "portable bitmap catalog truth returned to std ownership: {forbidden}"
        );
    }
}

#[test]
fn bitmap_presentation_identity_and_bound_remain_exact() {
    assert_eq!(
        conduit_presentation::BITMAP_PRESENTATION_KIND,
        "presentation/bitmap"
    );
    assert_eq!(
        conduit_presentation::BITMAP_PRESENTATION_REVISION,
        "conduit.presentation/bitmap@1"
    );
    let definition = conduit_presentation::bitmap_presentation_definition();
    assert_eq!(definition.inputs.len(), 1);
    assert!(definition.outputs.is_empty());
    assert_eq!(
        definition.inputs[0].value_kind.as_str(),
        conduit_presentation::GRAY8_BITMAP_INFO_KIND
    );
}
