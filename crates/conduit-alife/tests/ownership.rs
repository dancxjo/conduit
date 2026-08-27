use std::{fs, path::Path};

#[test]
fn reaction_diffusion_has_one_portable_owner_below_planning_and_hosts() {
    let alife_manifest = include_str!("../Cargo.toml");
    let core_manifest = include_str!("../../conduit-core/Cargo.toml");
    assert!(alife_manifest.contains("conduit-core ="));
    assert!(!core_manifest.contains("conduit-alife"));

    let core_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../conduit-core/src");
    for entry in fs::read_dir(&core_source).expect("read conduit-core source directory") {
        let path = entry.expect("read conduit-core source entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        assert!(
            !name.starts_with("reaction_diffusion"),
            "reaction-diffusion domain source returned to conduit-core: {}",
            path.display()
        );
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
    for forbidden in ["conduit-std-host", "hosts/"] {
        assert!(!dependencies.contains(forbidden));
    }
}
