use super::*;
use conduit_host_make::{
    canonical_host_configuration_conduit, check_host_configuration,
    parse_host_configuration_conduit, MakeCatalog, MakePackageSet,
};
fn source() -> String {
    include_str!("../../../profiles/linux-computer.host.conduit").replace("  limits =", r#"  base = {kind: "language/source-compiler", implementation: "std/bounded-source-check@1", initialization: {executable: "/usr/bin/conduit", executable_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", maximum_executable_bytes: 536870912, address_space_bytes: 16777216, cpu_seconds: 2, deadline_milliseconds: 5000, maximum_startup_bytes: 1048576, preparation_bytes: 33554432, retained_bytes: 1048576, request_bytes: 16384, reply_bytes: 65536}}
  limits ="#)
}
fn check(source: &str) -> Result<CheckedHostConfiguration, String> {
    let packages =
        MakePackageSet::compose(&[&crate::HostedMakePackage]).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(
        parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?,
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .map_err(|e| format!("{e:?}"))
}
#[test]
fn compiler_source_roundtrips_with_exact_finite_equipment_intent() {
    let checked = check(&source()).unwrap();
    let selected = CheckedSourceCheckInitialization::from_configuration(&checked).unwrap();
    assert_eq!(selected.executable, PathBuf::from("/usr/bin/conduit"));
    assert_eq!(selected.executable_sha256, [0xaa; 32]);
    assert_eq!(selected.preparation_bytes, 33554432);
    assert_eq!(
        checked.configuration_id(),
        check(&canonical_host_configuration_conduit(checked.configuration()).unwrap())
            .unwrap()
            .configuration_id()
    );
    for (old, new) in [
        ("cpu_seconds: 2", "cpu_seconds: 3"),
        ("/usr/bin/conduit", "/opt/conduit"),
        ("retained_bytes: 1048576", "retained_bytes: 2097152"),
    ] {
        assert_ne!(
            checked.configuration_id(),
            check(&source().replace(old, new))
                .unwrap()
                .configuration_id()
        );
    }
}
#[test]
fn compiler_fields_refuse_unbounded_missing_and_unrelated_input() {
    for (old, new) in [
        ("cpu_seconds: 2", "cpu_seconds: 0"),
        (
            "maximum_executable_bytes: 536870912",
            "maximum_executable_bytes: 0",
        ),
        (
            "maximum_executable_bytes: 536870912",
            "maximum_executable_bytes: 536870913",
        ),
        (
            "deadline_milliseconds: 5000",
            "deadline_milliseconds: 30001",
        ),
        ("reply_bytes: 65536", "reply_bytes: 65537"),
        ("request_bytes: 16384", "request_bytes: 262145"),
        ("cpu_seconds: 2, ", ""),
        ("cpu_seconds: 2", "source: \"plot\""),
        ("cpu_seconds: 2", "cpu_seconds: 2, cpu_seconds: 2"),
    ] {
        assert!(
            check(&source().replace(old, new)).is_err(),
            "accepted {new}"
        );
    }
}
#[test]
fn compiler_selection_refuses_invalid_artifact_and_inconsistent_memory() {
    for (old, new) in [
        ("/usr/bin/conduit", "conduit"),
        (
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bad-digest",
        ),
        ("preparation_bytes: 33554432", "preparation_bytes: 16777216"),
        ("preparation_bytes: 33554432", "preparation_bytes: 67108865"),
    ] {
        let checked = check(&source().replace(old, new)).unwrap();
        assert!(
            CheckedSourceCheckInitialization::from_configuration(&checked).is_err(),
            "accepted {new}"
        );
    }
}
#[test]
fn compiler_is_not_implicitly_available_on_other_platforms() {
    let source = source().replace(
        "machine: \"computer\", os: \"linux\"",
        "machine: \"windows-computer\", os: \"windows\"",
    );
    assert!(check(&source).is_err());
}

#[test]
fn make_image_retains_compiler_selection_and_refuses_changed_equipment() {
    let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let checked = check(&source()).unwrap();
    let (image, _) = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &conduit_host_make::BuildInputs {
            source_identity: "fixture/checked-compiler-source".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    conduit_host_make::verify_configuration_image(&checked, &image, &catalog).unwrap();
    let selection = image
        .payload
        .base_selections
        .iter()
        .find(|b| b.kind == SOURCE_CHECK_BASE_KIND)
        .unwrap();
    assert_eq!(selection.driver, SOURCE_CHECK_IMPLEMENTATION);
    assert_eq!(
        image.payload.source_configuration_id.as_deref(),
        Some(checked.configuration_id())
    );
    let changed = check(&source().replace("cpu_seconds: 2", "cpu_seconds: 3")).unwrap();
    assert!(conduit_host_make::verify_configuration_image(&changed, &image, &catalog).is_err());
}

#[test]
fn compiler_capability_intent_resolves_exact_call_resource_and_base() {
    let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let review = conduit_host_make::resolve_host_capability_intent(
        &conduit_host_make::HostCapabilityIntent {
            target: crate::HOSTED_TARGET_ID.into(),
            capabilities: vec![conduit_host_make::CapabilityIntent {
                kind: SOURCE_CHECK_KIND.into(),
                pinned_implementation: Some(SOURCE_CHECK_IMPLEMENTATION.into()),
            }],
        },
        &catalog,
        &packages,
    )
    .unwrap();
    let candidate = &review.capabilities[0].candidates[0];
    assert_eq!(candidate.implementation_id, SOURCE_CHECK_IMPLEMENTATION);
    assert_eq!(candidate.prerequisites.host_calls, vec![SOURCE_CHECK_CALL]);
    assert_eq!(
        candidate.prerequisites.resources,
        vec![SOURCE_CHECK_RESOURCE]
    );
    assert_eq!(candidate.prerequisites.bases.len(), 1);
    assert_eq!(
        candidate.prerequisites.bases[0].base_kind,
        SOURCE_CHECK_BASE_KIND
    );
}
