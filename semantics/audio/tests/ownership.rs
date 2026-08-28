use std::fs;

#[test]
fn portable_audio_meaning_has_one_host_neutral_owner() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("audio owner declares dependencies")
        .1;
    for forbidden in [
        "conduit-midi",
        "conduit-semantic-catalog",
        "conduit-std-host",
        "conduit-synth",
        "targets/",
    ] {
        assert!(!dependencies.contains(forbidden));
    }

    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core_source = workspace.join("architecture/core/src");
    for former_module in ["audio_info.rs", "audio_render_demand.rs", "sound_info.rs"] {
        assert!(
            !core_source.join(former_module).exists(),
            "portable audio domain returned to conduit-core: {former_module}"
        );
    }

    let core_manifest = fs::read_to_string(workspace.join("architecture/core/Cargo.toml"))
        .expect("read core manifest");
    assert!(
        !core_manifest.contains("conduit-audio"),
        "conduit-core must not depend on its higher-level audio domain owner"
    );
}
