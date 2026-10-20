use super::*;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs,
};

fn manifest(source: &str) -> BuildManifest {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    build_default_host_image(
        checked.into_profile(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "headless-test".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
    .manifest
}

fn selected(manifest: &BuildManifest, role: ArtifactRole) -> bool {
    let mut command = Command::new("cargo");
    configure_symbols(&mut command, manifest, role);
    command.get_envs().any(|(key, value)| {
        key == "CARGO_PROFILE_RELEASE_STRIP" && value == Some(std::ffi::OsStr::new("debuginfo"))
    })
}

#[test]
fn only_exact_empty_x86_product_retains_final_elf_symbols() {
    let empty = manifest(include_str!(
        "../../../profiles/conduitos-x86_64-pc.host.conduit"
    ));
    let native = manifest(include_str!(
        "../../../profiles/conduitos-native.host.conduit"
    ));
    assert!(selected(&empty, ArtifactRole::ProductHost));
    assert!(!selected(&empty, ArtifactRole::ArchitectureProofAppliance));
    assert!(!selected(&native, ArtifactRole::ProductHost));
    let mut other_target = empty.clone();
    other_target.target = "conduitos/ia32/pc".into();
    assert!(!selected(&other_target, ArtifactRole::ProductHost));
    // Each individual nonempty resolved fact must defeat the selection, even
    // when all the other facts remain empty. No profile-name inference.
    let nonempty = serde_json::to_value(&native).unwrap();
    for field in [
        "presenters",
        "facilities",
        "resource_budgets",
        "base_selections",
        "driver_selections",
        "host_calls",
    ] {
        assert!(!nonempty[field].as_array().unwrap().is_empty());
        let mut value = serde_json::to_value(&empty).unwrap();
        value[field] = nonempty[field].clone();
        let changed: BuildManifest = serde_json::from_value(value).unwrap();
        assert!(!selected(&changed, ArtifactRole::ProductHost), "{field}");
    }
}
