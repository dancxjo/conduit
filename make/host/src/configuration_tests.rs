use crate::test_packages::test_build_host_image;
use crate::test_packages::test_package_set;
use crate::{
    canonical_profile_json, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, ConfigurationDiagnostic, MakeCatalog,
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
        &MakeCatalog::canonical().with_packages(&test_package_set()),
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
            &MakeCatalog::canonical().with_packages(&test_package_set()),
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
    let catalog = MakeCatalog::canonical().with_packages(&packages);
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
            &MakeCatalog::canonical().with_packages(&test_package_set()),
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
        &MakeCatalog::canonical().with_packages(&test_package_set()),
        &test_package_set(),
    )
    .unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, ConfigurationDiagnostic::DuplicateResource { .. })));

    let wrong_family = HOSTED.replace("machine: \"computer\"", "machine: \"page\"");
    let diagnostics = check_host_configuration(
        parse_host_configuration_conduit(&wrong_family).unwrap(),
        &MakeCatalog::canonical().with_packages(&test_package_set()),
        &test_package_set(),
    )
    .unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, ConfigurationDiagnostic::UnknownTarget { .. })));
}

#[test]
fn zero_heap_is_an_exact_absence_not_unbounded_capacity() {
    let source = HOSTED.replace("heap_arena_bytes: 1048576", "heap_arena_bytes: 0");
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &MakeCatalog::canonical().with_packages(&test_package_set()),
        &test_package_set(),
    )
    .unwrap();
    assert_eq!(checked.profile().bounds.heap_arena_bytes, 0);
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
            "sha256:19a7cdcadd4372e51ef3ec477159b18d66dc70011fac2df78c5f077690635cd5",
            "sha256:7041516dd03cffea8701c7bd6b3651d2cd585e8950b22f36fc4a8dfd066a1aef",
        ),
        (
            "targets/conduitos/profiles/conduitos-native.host.conduit",
            "sha256:4d95e55fcbc813f344adef10c62518c3a9dea35c36e867c2de9c1a4f70ea5e03",
            "sha256:0cff773abd0b177ddcbac249c7a9ce22ed4e9fdb9151a5683c03d0f8acf4f5cd",
        ),
        (
            "targets/conduitos/profiles/conduitos-x86_64-pc.host.conduit",
            "sha256:9a31125f3588aa00afa3abbe75ddfbe5214b97b3b66bc590e581faad2dabefe2",
            "sha256:1ce15b181e243033995f418e5ce265e2c4d64f689ad605355b33b902331be82f",
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
    let catalog = MakeCatalog::canonical().with_packages(&packages);
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
