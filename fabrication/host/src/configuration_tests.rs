use crate::test_packages::test_build_host_image;
use crate::test_packages::test_package_set;
use crate::{
    canonical_profile_json, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, ConfigurationDiagnostic, FabricationCatalog,
};

const HOSTED: &str = r#"
host linux {
  schema = 1
  target = {architecture: "x86_64", machine: "computer", os: "linux"}
  base = {kind: "storage/protected-file", implementation: "hosted/protected-file@1"}
  limits = {static_memory_bytes: 1048576, heap_arena_bytes: 1048576, queue_items: 64, buffered_bytes: 65536, active_instances: 8, operation_slots: 8, timer_slots: 8, line_sessions: 2, evidence_items: 64}
}
"#;

#[test]
fn canonical_source_lowers_into_existing_profile_with_provenance() {
    let checked = check_host_configuration(
        parse_host_configuration_conduit(HOSTED).unwrap(),
        &FabricationCatalog::canonical().with_packages(&test_package_set()),
        &test_package_set(),
    )
    .unwrap();
    assert_eq!(checked.profile().target.key(), "std/x86_64/computer");
    assert_eq!(
        checked.profile().source_configuration_id.as_deref(),
        Some(checked.configuration_id())
    );
    assert_eq!(
        checked.resolved_bases(),
        &[(
            "storage/protected-file".into(),
            "hosted/protected-file@1".into()
        )]
    );
}

#[test]
fn retired_hosted_role_labels_migrate_to_one_canonical_computer() {
    for role in ["workstation", "server"] {
        let source = HOSTED.replace("machine: \"computer\"", &format!("machine: \"{role}\""));
        let checked = check_host_configuration(
            parse_host_configuration_conduit(&source).unwrap(),
            &FabricationCatalog::canonical().with_packages(&test_package_set()),
            &test_package_set(),
        )
        .unwrap();
        assert_eq!(checked.configuration().target.machine, "computer");
        assert_eq!(checked.profile().target.key(), "std/x86_64/computer");
    }
}

#[test]
fn selection_order_does_not_change_profile_identity() {
    let first = HOSTED.replace(
        "  limits =",
        "  base = {kind: \"clock/monotonic\", implementation: \"hosted/monotonic-clock@1\"}\n  limits =",
    );
    let second = first.replace(
        "  base = {kind: \"storage/protected-file\", implementation: \"hosted/protected-file@1\"}\n  base = {kind: \"clock/monotonic\", implementation: \"hosted/monotonic-clock@1\"}",
        "  base = {kind: \"clock/monotonic\", implementation: \"hosted/monotonic-clock@1\"}\n  base = {kind: \"storage/protected-file\", implementation: \"hosted/protected-file@1\"}",
    );
    let packages = test_package_set();
    let catalog = FabricationCatalog::canonical().with_packages(&packages);
    let a = check_host_configuration(
        parse_host_configuration_conduit(&first).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let b = check_host_configuration(
        parse_host_configuration_conduit(&second).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    assert_eq!(a.configuration_id(), b.configuration_id());
    assert_eq!(
        canonical_profile_json(a.profile()).unwrap(),
        canonical_profile_json(b.profile()).unwrap()
    );
}

#[test]
fn rejects_each_required_invalid_class() {
    let cases = [
        (HOSTED.replace("x86_64", "mystery"), "UnknownTarget"),
        (HOSTED.replace("storage/protected-file", "unknown/base"), "UnknownBase"),
        (HOSTED.replace("hosted/protected-file@1", "unknown-driver@1"), "UnknownImplementation"),
        (HOSTED.replace("hosted/protected-file@1", "pico/usb-cdc@1"), "IncompatibleImplementation"),
        (HOSTED.replace("storage/protected-file", "browser/dom"), "UnsupportedBase"),
        (HOSTED.replace("static_memory_bytes: 1048576", "static_memory_bytes: 999999999999"), "LimitExceeded"),
        (HOSTED.replace("queue_items: 64", "queue_items: 0"), "UnboundedCapacity"),
        (HOSTED.replace("  limits =", "  base = {kind: \"storage/protected-file\", implementation: \"hosted/serial@1\"}\n  limits ="), "DuplicateContradictoryBase"),
    ];
    for (source, expected) in cases {
        let diagnostics = check_host_configuration(
            parse_host_configuration_conduit(&source).unwrap(),
            &FabricationCatalog::canonical().with_packages(&test_package_set()),
            &test_package_set(),
        )
        .unwrap_err();
        let rendered = format!("{diagnostics:?}");
        assert!(
            rendered.contains(expected),
            "expected {expected}: {rendered}"
        );
    }
    let duplicate_resource = HOSTED.replace("  limits =", "  need = {id: \"r\", class: \"memory\", slots: 1, bytes: 1}\n  need = {id: \"r\", class: \"memory\", slots: 2, bytes: 2}\n  limits =");
    let diagnostics = check_host_configuration(
        parse_host_configuration_conduit(&duplicate_resource).unwrap(),
        &FabricationCatalog::canonical().with_packages(&test_package_set()),
        &test_package_set(),
    )
    .unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, ConfigurationDiagnostic::DuplicateResource { .. })));

    let wrong_family = HOSTED.replace("machine: \"computer\"", "machine: \"page\"");
    let diagnostics = check_host_configuration(
        parse_host_configuration_conduit(&wrong_family).unwrap(),
        &FabricationCatalog::canonical().with_packages(&test_package_set()),
        &test_package_set(),
    )
    .unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, ConfigurationDiagnostic::UnknownTarget { .. })));
}

#[test]
fn checked_in_configurations_cover_every_catalog_target_with_exact_provenance() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = [
        (
            "targets/browser/profiles/browser-page.host.conduit",
            "sha256:d32fd79e5344a1d2d3156aa962458f794046eb15d88911942762a9c4acc91b1a",
            "sha256:2412a5624c572a8dfaa0a0ae8f7d10416f610b8edf0a118121be1fab0eb6e1d7",
        ),
        (
            "targets/conduitos/profiles/conduitos-aarch64-virt.host.conduit",
            "sha256:9cc965dac8190afab6afc54b1d265ed39e5ac8255642a055c61f87c32f1e848c",
            "sha256:e26eab043c52c45117cad48342b570d63cf05c54410b4b7bf6ff421b10dd40c6",
        ),
        (
            "targets/conduitos/profiles/conduitos-x86_64-pc.host.conduit",
            "sha256:9ba54ff0fb5cf22a4b2b031bf24580a244305fd1036110bd23bf05ab30b76738",
            "sha256:063f375b6a42358e5650bc6c0502315cdc9d0973df6962446c46a15b97e9fc85",
        ),
        (
            "targets/rp2040/profiles/pico-w.host.conduit",
            "sha256:c3067ec55f4936c284666a3ab9c6cd39ace1db6edfb2783c673d6bba4174cf22",
            "sha256:94fe214ac7de69502467762c67388bf02f44d0b731c668fe557249d4dd20b7c9",
        ),
        (
            "targets/std/profiles/linux-computer.host.conduit",
            "sha256:fae0becde708c48b6bb0f3adc795efbbdc39bb6d0e96f87a4762afd3beb20b26",
            "sha256:bfb464f102209ba4ec2c459f47f0027faca9a84c8f70948da83dc861e3337e88",
        ),
    ];
    let packages = test_package_set();
    let catalog = FabricationCatalog::canonical().with_packages(&packages);
    let descriptors = packages.target_descriptors();
    let descriptor_targets = descriptors
        .iter()
        .map(|item| format!("{}/{}/{}", item.family, item.architecture, item.machine))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        descriptor_targets,
        catalog.targets.iter().cloned().collect(),
        "every configuration-supported catalog target needs one descriptor"
    );
    let mut snapshots = Vec::new();
    for (name, expected_configuration_id, expected_profile_id) in fixtures {
        let source = std::fs::read_to_string(root.join(name)).unwrap();
        let checked = check_host_configuration(
            parse_host_configuration_conduit(&source).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        assert_eq!(checked.configuration_id(), expected_configuration_id);
        let target = checked.profile().target.key();
        let bases = checked.resolved_bases().to_vec();
        let bounds = checked.profile().bounds.clone();
        let expected = checked.configuration_id().to_owned();
        let (image, _) = test_build_host_image(
            checked.into_profile(),
            &catalog,
            &BuildInputs {
                source_identity: "test-source".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        assert_eq!(
            image.manifest.source_configuration_id.as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(
            image.payload.source_configuration_id.as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(image.manifest.profile_id, expected_profile_id);
        snapshots.push((
            name,
            target,
            expected,
            image.manifest.profile_id,
            bases,
            bounds,
        ));
    }
    assert!(
        snapshots.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "fixture snapshot order must be canonical"
    );
}
