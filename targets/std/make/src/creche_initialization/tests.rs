use super::*;
use conduit_host_make::{
    canonical_host_configuration_conduit, check_host_configuration,
    parse_host_configuration_conduit, resolve_host_capability_intent, CapabilityIntent,
    HostCapabilityIntent, MakeCatalog, MakePackageSet,
};

fn source() -> String {
    include_str!("../../../profiles/linux-computer.host.conduit").replace("  limits =", r#"  base = {kind: "creche/encounter", implementation: "std/kernel-creche-prepare@1", initialization: {slot_identity: "authoring/creche", image_manifest: "/explicit/host-image.json", image_manifest_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", artifact: "/explicit/host.bin", package_directory: "/explicit/private-packages", artifact_sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", host_configuration: "/explicit/destination.host.conduit", host_configuration_sha256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc", maximum_artifact_bytes: 1048576, maximum_manifest_bytes: 65536, maximum_evidence_bytes: 4096, maximum_polls: 1000, minimum_poll_interval_ms: 10}}
  limits ="#)
}
fn check(source: &str) -> Result<conduit_host_make::CheckedHostConfiguration, String> {
    let packages =
        MakePackageSet::compose(&[&crate::HostedMakePackage]).map_err(|e| format!("{e:?}"))?;
    let configuration = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(
        configuration,
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .map_err(|e| format!("{e:?}"))
}
#[test]
fn creche_selection_roundtrips_and_every_field_binds_configuration_identity() {
    let checked = check(&source()).unwrap();
    let canonical = canonical_host_configuration_conduit(checked.configuration()).unwrap();
    assert_eq!(
        checked.configuration_id(),
        check(&canonical).unwrap().configuration_id()
    );
    for (old, new) in [
        ("authoring/creche", "authoring/other"),
        (
            "maximum_evidence_bytes: 4096",
            "maximum_evidence_bytes: 8192",
        ),
        ("maximum_polls: 1000", "maximum_polls: 999"),
        ("/explicit/host-image.json", "/explicit/other-image.json"),
        ("/explicit/host.bin", "/explicit/other-host.bin"),
        ("/explicit/private-packages", "/explicit/other-packages"),
        (
            "/explicit/destination.host.conduit",
            "/explicit/other.host.conduit",
        ),
        (
            "maximum_artifact_bytes: 1048576",
            "maximum_artifact_bytes: 2097152",
        ),
        (
            "maximum_manifest_bytes: 65536",
            "maximum_manifest_bytes: 32768",
        ),
        (
            "minimum_poll_interval_ms: 10",
            "minimum_poll_interval_ms: 20",
        ),
    ] {
        assert_ne!(
            checked.configuration_id(),
            check(&source().replace(old, new))
                .unwrap()
                .configuration_id()
        );
    }
    for (from, to) in [('a', 'd'), ('b', 'e'), ('c', 'f')] {
        let changed = source().replace(&from.to_string().repeat(64), &to.to_string().repeat(64));
        assert_ne!(
            checked.configuration_id(),
            check(&changed).unwrap().configuration_id()
        );
    }
}
#[test]
fn creche_selection_refuses_missing_unknown_duplicate_and_invalid_limits() {
    for (old, new) in [
        ("slot_identity: \"authoring/creche\", ", ""),
        ("slot_identity: \"authoring/creche\"", "slot_identity: \"\""),
        ("maximum_polls: 1000", "unknown: 1000"),
        (
            "maximum_polls: 1000",
            "maximum_polls: 1000, maximum_polls: 1000",
        ),
        ("maximum_evidence_bytes: 4096", "maximum_evidence_bytes: 0"),
        (
            "maximum_evidence_bytes: 4096",
            "maximum_evidence_bytes: 67108865",
        ),
        ("maximum_polls: 1000", "maximum_polls: 0"),
        ("maximum_polls: 1000", "maximum_polls: 1000001"),
        (
            "minimum_poll_interval_ms: 10",
            "minimum_poll_interval_ms: 0",
        ),
        (
            "minimum_poll_interval_ms: 10",
            "minimum_poll_interval_ms: 1001",
        ),
    ] {
        assert!(
            check(&source().replace(old, new)).is_err(),
            "accepted {new}"
        );
    }
    assert!(check(&source().replace("authoring/creche", &"x".repeat(257))).is_err());
    let line = source()
        .lines()
        .find(|line| line.contains("creche/encounter"))
        .unwrap()
        .to_owned();
    assert!(check(&source().replace(
        "  limits =",
        &format!("{}\n  limits =", line.replace("4096", "8192"))
    ))
    .is_err());
}
#[test]
fn creche_selection_is_linux_x86_64_only() {
    for replacement in [
        "architecture: \"x86_64\", machine: \"windows-computer\", os: \"windows\"",
        "architecture: \"aarch64\", machine: \"macos-computer\", os: \"macos\"",
    ] {
        assert!(check(&source().replace(
            "architecture: \"x86_64\", machine: \"computer\", os: \"linux\"",
            replacement
        ))
        .is_err());
    }
    assert_eq!(offer().target_patterns, vec![crate::HOSTED_TARGET_ID]);
}
#[test]
fn creche_capability_review_resolves_exact_host_call_resource_and_base() {
    let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let review = resolve_host_capability_intent(
        &HostCapabilityIntent {
            target: crate::HOSTED_TARGET_ID.into(),
            capabilities: vec![CapabilityIntent {
                kind: conduit_body_make::deployment_runtime::CRECHE_KIND.into(),
                pinned_implementation: Some(CRECHE_IMPLEMENTATION.into()),
            }],
        },
        &catalog,
        &packages,
    )
    .unwrap();
    let candidate = &review.capabilities[0].candidates[0];
    assert_eq!(candidate.implementation_id, CRECHE_IMPLEMENTATION);
    assert_eq!(
        candidate.prerequisites.host_calls,
        vec![conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL]
    );
    assert_eq!(candidate.prerequisites.resources, vec![CRECHE_RESOURCE]);
    assert_eq!(candidate.prerequisites.bases.len(), 1);
    assert_eq!(
        candidate.prerequisites.bases[0].base_kind,
        CRECHE_ENCOUNTER_BASE_KIND
    );
    assert_eq!(
        candidate.prerequisites.bases[0]
            .compatible_implementations
            .len(),
        1
    );
    assert_eq!(
        candidate.prerequisites.bases[0].compatible_implementations[0].implementation_id,
        CRECHE_IMPLEMENTATION
    );
}

#[test]
fn creche_capability_review_refuses_a_base_implementation_resource_cycle() {
    let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
    let mut catalog = MakeCatalog::canonical().with_packages(&packages);
    catalog.dependencies.insert(
        PrerequisiteNode::Base(CRECHE_ENCOUNTER_BASE_KIND.into()),
        vec![PrerequisiteNode::Implementation(
            CRECHE_IMPLEMENTATION.into(),
        )],
    );
    catalog.dependencies.insert(
        PrerequisiteNode::Implementation(CRECHE_IMPLEMENTATION.into()),
        vec![PrerequisiteNode::Resource(CRECHE_RESOURCE.into())],
    );
    assert!(matches!(
        resolve_host_capability_intent(
            &HostCapabilityIntent {
                target: crate::HOSTED_TARGET_ID.into(),
                capabilities: vec![CapabilityIntent {
                    kind: conduit_body_make::deployment_runtime::CRECHE_KIND.into(),
                    pinned_implementation: Some(CRECHE_IMPLEMENTATION.into()),
                }],
            },
            &catalog,
            &packages
        ),
        Err(conduit_host_make::CapabilityIntentRefusal::CircularPrerequisite { .. })
    ));
}
