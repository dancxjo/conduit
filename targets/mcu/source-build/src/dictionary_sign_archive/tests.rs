use super::*;
use conduit_host_make::*;
const SOURCE: &str = r#"host archive-test {
 schema = 1
 target = {architecture: "x86_64", machine: "computer", os: "linux"}
 base = {kind: "kernel/sign-archive", implementation: "kernel/dictionary-prefix-sign-archive@1", initialization: {maximum_retained_events: 32768, maximum_canonical_bytes: 2097152, template_events: 128, template_count: 4, segment_runs: 48, request_lanes: 2, remote_streams: 2, maximum_archive_backing_bytes: 8192, maximum_transfer_overlap_bytes: 12288}}
 limits = {static_memory_bytes: 229376, heap_arena_bytes: 110592, queue_items: 20, buffered_bytes: 24576, active_instances: 8, operation_slots: 16, timer_slots: 4, line_sessions: 2, evidence_items: 64}
}"#;
struct Package(HostBounds);
impl HostMakePackage for Package {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Anchor(MakeAnchor {
            package_id: "test-fore@1".into(),
            package_revision: 1,
            targets: vec![TargetDescriptor {
                label: "Test".into(),
                family: "std".into(),
                architecture: "x86_64".into(),
                machine: "computer".into(),
                board: None,
                os: Some("linux".into()),
                host_core: "host-core/test@1".into(),
                presenter: None,
                host_calls: vec![],
                toolchain_identity: "test".into(),
                builder_adapter: "test".into(),
                strategy: MakeStrategy::DeterministicSpecializedBuild,
                deployment_adapter: None,
                outputs: vec![SporeOutputKind::NativeBundle],
                default_output: SporeOutputKind::NativeBundle,
                post_build_actions: vec![],
                make_descriptors: vec![],
                maxima: self.0.clone(),
            }],
            offers: vec![],
            catalog: Default::default(),
        })
    }
}

struct Contribution(MakeContribution);
impl HostMakePackage for Contribution {
    fn contribution(&self) -> MakeContribution {
        self.0.clone()
    }
}
fn checked(source: &str, register: bool, wrong: bool) -> Result<CheckedHostConfiguration, String> {
    let config = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    let anchor = Package(parse_host_configuration_conduit(SOURCE).unwrap().limits);
    let extension =
        conduit_host_make::dictionary_sign_archive::DictionarySignArchivePackage::for_targets(
            vec!["std/*/*".into()],
        )
        .unwrap();
    let mut contribution = extension.contribution();
    if wrong {
        let MakeContribution::Extension(e) = &mut contribution else {
            panic!()
        };
        e.offers[0].implementation_id = "test/other-archive@1".into();
        e.catalog.initialization_contracts =
            [("test/other-archive@1".into(), initialization_contract())].into();
    }
    let extension = Contribution(contribution);
    let selected: Vec<&dyn HostMakePackage> = if register {
        vec![&anchor, &extension]
    } else {
        vec![&anchor]
    };
    let packages = MakePackageSet::compose(&selected).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(
        config,
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .map_err(|e| format!("{e:?}"))
}
#[test]
fn exact_extension_is_explicit_and_absence_returns_no_archive_construction() {
    assert!(
        checked(SOURCE, false, false).is_err(),
        "not registered globally"
    );
    let c = checked(SOURCE, true, false).unwrap();
    let before = c.configuration().limits.clone();
    let bounds = CheckedDictionarySignArchiveConstruction::from_configuration(&c)
        .unwrap()
        .unwrap();
    assert_eq!(bounds.maximum_canonical_bytes, 2097152);
    assert_eq!(bounds.template_events, 128);
    assert_eq!(c.configuration().limits, before);
    assert_eq!(before.evidence_items, 64);
    assert_eq!(before.buffered_bytes, 24576);
    let absent = SOURCE
        .lines()
        .filter(|l| !l.trim_start().starts_with("base ="))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        CheckedDictionarySignArchiveConstruction::from_configuration(
            &checked(&absent, false, false).unwrap()
        )
        .unwrap(),
        None
    );
}
#[test]
fn other_registered_implementation_is_not_the_exact_archive_code() {
    let wrong = SOURCE.replace(IMPLEMENTATION, "test/other-archive@1");
    let c = checked(&wrong, true, true).unwrap();
    assert!(CheckedDictionarySignArchiveConstruction::from_configuration(&c).is_err());
    assert!(checked(
        &SOURCE.replace(
            "template_count: 4, segment_runs: 48, request_lanes: 2",
            "template_count: 0, segment_runs: 48, request_lanes: 2"
        ),
        true,
        false
    )
    .is_err());
}

#[test]
fn emission_preserves_checked_dimensions_and_separate_logical_and_physical_limits() {
    let c = checked(SOURCE, true, false).unwrap();
    let selected = CheckedDictionarySignArchiveConstruction::from_configuration(&c)
        .unwrap()
        .unwrap();
    assert_eq!(selected.cfg_selection(), "dictionary_sign_archive");
    let output = selected.rust_module();
    for constant in [
        "DICTIONARY_ARCHIVE_EVENTS: usize = 128",
        "DICTIONARY_ARCHIVE_TEMPLATES: usize = 4",
        "DICTIONARY_ARCHIVE_SEGMENTS: usize = 48",
        "DICTIONARY_ARCHIVE_REQUEST_LANES: usize = 2",
        "DICTIONARY_ARCHIVE_REMOTE_STREAMS: usize = 2",
        "retained_events: 32768",
        "retained_bytes: 2097152",
        "archive_backing_bytes: 8192",
        "transfer_overlap_bytes: 12288",
    ] {
        assert!(output.contains(constant), "missing {constant}");
    }
    let zero = SOURCE
        .replace("request_lanes: 2", "request_lanes: 0")
        .replace("remote_streams: 2", "remote_streams: 0");
    let selected = CheckedDictionarySignArchiveConstruction::from_configuration(
        &checked(&zero, true, false).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert!(selected
        .rust_module()
        .contains("DICTIONARY_ARCHIVE_REQUEST_LANES: usize = 0"));
    assert!(selected
        .rust_module()
        .contains("DICTIONARY_ARCHIVE_REMOTE_STREAMS: usize = 0"));
}
