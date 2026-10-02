use super::*;

#[test]
fn native_make_uses_the_admitted_heap_arena_ceiling() {
    let profile = check_host_configuration(
        parse_host_configuration_conduit(include_str!(
            "../../../profiles/conduitos-native.host.conduit"
        ))
        .unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
    .into_profile();
    let (checked, _) = build_default_host_image(
        profile,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "test-source".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    assert_eq!(runtime_arena_ceiling(&checked.manifest), 16 * 1024 * 1024);
    assert_ne!(
        runtime_arena_ceiling(&checked.manifest),
        checked.manifest.bounds.static_memory_bytes
    );
}

#[test]
fn headless_make_retains_its_static_arena_ceiling() {
    let packages = conduit_workspace_make::package_set();
    let profile = check_host_configuration(
        parse_host_configuration_conduit(include_str!(
            "../../../profiles/conduitos-aarch64-virt.host.conduit"
        ))
        .unwrap(),
        &conduit_workspace_make::catalog(),
        &packages,
    )
    .unwrap()
    .into_profile();
    let (checked, _) = build_default_host_image(
        profile,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "test-source".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    assert_eq!(checked.manifest.bounds.heap_arena_bytes, 0);
    assert_eq!(
        runtime_arena_ceiling(&checked.manifest),
        checked.manifest.bounds.static_memory_bytes
    );
}

#[test]
fn aarch64_architecture_build_is_typed_as_a_proof_appliance() {
    let record = execute_architecture_proof(
        ConduitosArch::Aarch64,
        &GlobalOpts {
            dry_run: true,
            ..GlobalOpts::default()
        },
    )
    .unwrap();
    assert_eq!(
        record.artifact_role,
        ArtifactRole::ArchitectureProofAppliance
    );
}
