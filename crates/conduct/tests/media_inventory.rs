use std::path::PathBuf;
use std::process::Command;

use conduit_compile::{InstalledProfile, compile_source};
use conduit_media::register_deterministic_media_providers;
use conduit_runtime::Registry;

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn typed_media_contracts_are_sealed_into_the_exact_compile_catalog() {
    let source = include_str!("../../../examples/media-audio-frame.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_media_providers(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();

    let literal = installed
        .input
        .catalog
        .external_leaf_contracts
        .iter()
        .find(|contract| contract.id == "conduit.media/audio-frame/literal")
        .expect("the exact input seals the domain contract");
    assert!(literal.config.is_empty());
    assert_eq!(literal.outputs.len(), 1);
    assert_eq!(
        literal.outputs[0].value_type.id,
        "conduit.media/audio-frame"
    );
    assert!(installed.input.catalog.types.iter().any(|pin| {
        pin.id == "conduit.media/audio-frame"
            && pin.semantic_hash == literal.outputs[0].value_type.semantic_hash
    }));

    let document = compile_source(source, &installed.input).unwrap();
    assert!(document.nodes.iter().any(|node| {
        node.contract.id == "conduit.media/audio-frame/literal"
            && node.implementation.id == "conduit.media/audio-literal-deterministic"
    }));
}

#[test]
fn standalone_and_composed_media_panels_run_through_the_canonical_cli() {
    for (path, expected) in [
        (
            "examples/media-audio-frame.panel",
            "audio:s16le:48000:stereo:192",
        ),
        (
            "examples/media-frame-compose.panel",
            "audio:s16le:48000:stereo:192video:rgb24:2x2",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(workspace_file(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}
