use crate::{
    build_host_image, canonical_profile_json, check_host_configuration, parse_host_configuration,
    target_descriptors, BuildInputs, ConfigurationDiagnostic, FabricationCatalog,
};

const HOSTED: &str = r#"
schema = 1
name = "linux"
[target]
architecture = "x86_64"
machine = "workstation"
os = "linux"
[[bases]]
kind = "storage/protected-file"
implementation = "hosted/protected-file@1"
[limits]
static_memory_bytes = 1048576
heap_arena_bytes = 1048576
queue_items = 64
buffered_bytes = 65536
active_instances = 8
operation_slots = 8
timer_slots = 8
line_sessions = 2
evidence_items = 64
"#;

#[test]
fn toml_lowers_into_existing_profile_with_provenance() {
    let checked = check_host_configuration(
        parse_host_configuration(HOSTED).unwrap(),
        &FabricationCatalog::canonical(),
    )
    .unwrap();
    assert_eq!(checked.profile().target.key(), "std/x86_64/workstation");
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
fn selection_order_does_not_change_profile_identity() {
    let first = HOSTED.replace("[[bases]]", "[[bases]]\nkind = \"clock/monotonic\"\nimplementation = \"hosted/monotonic-clock@1\"\n[[bases]]");
    let second = first.replace(
        "kind = \"clock/monotonic\"\nimplementation = \"hosted/monotonic-clock@1\"\n[[bases]]\nkind = \"storage/protected-file\"\nimplementation = \"hosted/protected-file@1\"",
        "kind = \"storage/protected-file\"\nimplementation = \"hosted/protected-file@1\"\n[[bases]]\nkind = \"clock/monotonic\"\nimplementation = \"hosted/monotonic-clock@1\"",
    );
    let catalog = FabricationCatalog::canonical();
    let a = check_host_configuration(parse_host_configuration(&first).unwrap(), &catalog).unwrap();
    let b = check_host_configuration(parse_host_configuration(&second).unwrap(), &catalog).unwrap();
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
        (HOSTED.replace("static_memory_bytes = 1048576", "static_memory_bytes = 999999999999"), "LimitExceeded"),
        (HOSTED.replace("queue_items = 64", "queue_items = 0"), "UnboundedCapacity"),
        (HOSTED.replace("[[bases]]", "[[bases]]\nkind = \"storage/protected-file\"\nimplementation = \"hosted/serial@1\"\n[[bases]]"), "DuplicateContradictoryBase"),
    ];
    for (source, expected) in cases {
        let diagnostics = check_host_configuration(
            parse_host_configuration(&source).unwrap(),
            &FabricationCatalog::canonical(),
        )
        .unwrap_err();
        let rendered = format!("{diagnostics:?}");
        assert!(
            rendered.contains(expected),
            "expected {expected}: {rendered}"
        );
    }
    let duplicate_resource = HOSTED.replace("[limits]", "[[resources]]\nid = \"r\"\nclass = \"memory\"\nslots = 1\nbytes = 1\n[[resources]]\nid = \"r\"\nclass = \"memory\"\nslots = 2\nbytes = 2\n[limits]");
    let diagnostics = check_host_configuration(
        parse_host_configuration(&duplicate_resource).unwrap(),
        &FabricationCatalog::canonical(),
    )
    .unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, ConfigurationDiagnostic::DuplicateResource { .. })));
}

#[test]
fn three_checked_in_configurations_build_with_exact_provenance() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../profiles/host-configurations");
    for name in [
        "linux-workstation.host.toml",
        "pico-w.host.toml",
        "browser-page.host.toml",
    ] {
        let source = std::fs::read_to_string(root.join(name)).unwrap();
        let checked = check_host_configuration(
            parse_host_configuration(&source).unwrap(),
            &FabricationCatalog::canonical(),
        )
        .unwrap();
        let descriptor = target_descriptors()
            .into_iter()
            .find(|item| {
                item.architecture == checked.configuration().target.architecture
                    && item.machine == checked.configuration().target.machine
                    && item.board.map(str::to_owned) == checked.configuration().target.board
                    && item.os.map(str::to_owned) == checked.configuration().target.os
            })
            .unwrap();
        let expected = checked.configuration_id().to_owned();
        let (image, _) = build_host_image(
            checked.into_profile(),
            &FabricationCatalog::canonical(),
            &BuildInputs {
                source_identity: "test-source".into(),
                toolchain_identity: "test-toolchain".into(),
                toolchain_available: true,
                maxima: descriptor.maxima,
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
        assert!(!image.manifest.base_selections.is_empty());
    }
}
