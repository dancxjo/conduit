use super::*;
use conduit_host_make::*;
const IMPLEMENTATION: &str = "test/root-fore@1";
const SOURCE: &str = r#"host test {
 schema = 1
 target = {architecture: "x86_64", machine: "computer", os: "linux"}
 base = {kind: "line/fore-carrier", implementation: "test/root-fore@1", initialization: {authorizer: "seed-owner", maximum_lines: 2, maximum_frame_bytes: 512, maximum_payload_bytes: 460, maximum_buffered_bytes: 1024, maximum_in_flight_items: 1, maximum_frames: 100}}
 preparation = {authorizer: "seed-owner", maximum_assignment_bytes: 1024, maximum_runtime_state_bytes: 1024}
 execution = {authorizer: "seed-owner", maximum_active_worksets: 1}
 limits = {static_memory_bytes: 65536, heap_arena_bytes: 65536, queue_items: 64, buffered_bytes: 4096, active_instances: 8, operation_slots: 8, timer_slots: 8, line_sessions: 2, evidence_items: 64}
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
            offers: vec![ImplementationOffer {
                base_kind: BASE_KIND.into(),
                implementation_id: IMPLEMENTATION.into(),
                implementation_revision: 1,
                target_patterns: vec!["std/x86_64/computer".into()],
                prerequisites: vec![],
                build_feature: None,
            }],
            catalog: PackageCatalogContribution {
                initialization_contracts: [(IMPLEMENTATION.into(), initialization_contract())]
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
        })
    }
}
fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
    let config = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    let package = Package(parse_host_configuration_conduit(SOURCE).unwrap().limits);
    let packages = MakePackageSet::compose(&[&package]).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(
        config,
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .map_err(|e| format!("{e:?}"))
}
fn construction(source: &str) -> Result<Option<CheckedForeCarrierConstruction>, String> {
    CheckedForeCarrierConstruction::from_configuration(&checked(source)?, IMPLEMENTATION)
}
#[test]
fn explicit_selection_and_absence() {
    let selected = construction(SOURCE).unwrap().unwrap();
    assert_eq!(selected.maximum_payload_bytes, 460);
    assert_eq!(selected.maximum_buffered_bytes, 1024);
    assert!(selected.rust_bounds().contains("maximum_in_flight_items:1"));
    let absent = SOURCE
        .lines()
        .filter(|l| !l.trim_start().starts_with("base ="))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(construction(&absent).unwrap(), None);
    assert!(CheckedForeCarrierConstruction::from_configuration(
        &checked(SOURCE).unwrap(),
        "foreign/root@1"
    )
    .is_err());
}
#[test]
fn missing_policy_fields_and_duplicate_are_refused() {
    for prefix in [" preparation =", " execution ="] {
        let source = SOURCE
            .lines()
            .filter(|l| !l.starts_with(prefix))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(construction(&source).is_err());
    }
    for mutation in [
        SOURCE.replace(
            "authorizer: \"seed-owner\", maximum_lines",
            "authorizer: \"foreign\", maximum_lines",
        ),
        SOURCE.replace("maximum_frames: 100", ""),
        SOURCE.replace("authorizer: \"seed-owner\", maximum_lines", "maximum_lines"),
        SOURCE.replace("maximum_in_flight_items: 1", "maximum_in_flight_items: 2"),
        SOURCE.replace(
            "maximum_frame_bytes: 512",
            "maximum_frame_bytes: 4294967295",
        ),
        SOURCE.replace("initialization: {", "wrong_initialization: {"),
        SOURCE.replace(
            " preparation =",
            &format!(
                "{}\n preparation =",
                SOURCE
                    .lines()
                    .find(|l| l.trim_start().starts_with("base ="))
                    .unwrap()
            ),
        ),
    ] {
        assert!(construction(&mutation).is_err());
    }
}
#[test]
fn frame_and_original_host_caps_are_enforced() {
    for (old, new) in [
        ("maximum_payload_bytes: 460", "maximum_payload_bytes: 461"),
        (
            "maximum_buffered_bytes: 1024",
            "maximum_buffered_bytes: 511",
        ),
        ("maximum_lines: 2", "maximum_lines: 3"),
        (
            "maximum_buffered_bytes: 1024",
            "maximum_buffered_bytes: 4097",
        ),
        ("heap_arena_bytes: 65536", "heap_arena_bytes: 512"),
    ] {
        assert!(construction(&SOURCE.replace(old, new)).is_err(), "{new}");
    }
}
