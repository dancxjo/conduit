use std::{fs, path::Path};

#[test]
fn artificial_life_domains_have_one_portable_owner_below_planning_and_hosts() {
    let alife_manifest = include_str!("../Cargo.toml");
    let core_manifest = include_str!("../../../architecture/core/Cargo.toml");
    assert!(alife_manifest.contains("conduit-core ="));
    assert!(!core_manifest.contains("conduit-alife"));

    let core_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../architecture/core/src");
    for entry in fs::read_dir(&core_source).expect("read conduit-core source directory") {
        let path = entry.expect("read conduit-core source entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        for domain in ["reaction_diffusion", "lenia"] {
            assert!(
                !name.starts_with(domain),
                "{domain} domain source returned to conduit-core: {}",
                path.display()
            );
        }
    }
}

#[test]
fn alife_owner_remains_host_neutral() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("alife owner declares dependencies")
        .1
        .split_once("[dev-dependencies]")
        .expect("alife owner declares dev dependencies")
        .0;
    for forbidden in ["conduit-semantic-catalog", "conduit-std-host", "targets/"] {
        assert!(!dependencies.contains(forbidden));
    }
}

#[test]
fn distributed_lenia_probe_uses_the_portable_alife_owner() {
    for (owner, source) in [
        (
            "std proof",
            include_str!("../../../targets/std/src/bin/distributed-lenia-probe.rs"),
        ),
        (
            "ESP32 WROOM firmware",
            include_str!("../../../targets/esp32/firmware/wroom-signal/src/lenia_session.rs"),
        ),
        (
            "Pico W firmware",
            include_str!("../../../targets/rp2040/firmware/pico-w-signal/src/distributed_lenia.rs"),
        ),
    ] {
        assert!(source.contains("use conduit_alife::{"), "{owner}");
        assert!(
            !source.contains("conduit_core::"),
            "{owner} must not restore the removed core compatibility owner"
        );
    }
}

#[test]
fn alife_catalog_owns_the_authored_scalar_field_spelling() {
    let source = include_str!("../src/lenia_catalog.rs");
    assert!(source.contains("insert_value_kind_alias"));
    assert!(source.contains("ScalarField2"));
    assert!(source.contains("SCALAR_FIELD2_INFO_ID"));
}
