use super::*;
use conduit_host_make::{
    canonical_host_configuration_conduit, check_host_configuration,
    parse_host_configuration_conduit, MakeCatalog, MakePackageSet,
};
fn source(write: bool) -> String {
    let kind = if write {
        DURABLE_TEXT_SAVE_KIND
    } else {
        DURABLE_TEXT_LOAD_KIND
    };
    include_str!("../../../profiles/linux-computer.host.conduit").replace("  limits =", &format!(r#"  base = {{kind: "{DURABLE_TEXT_BASE_KIND}", implementation: "{}", initialization: {{maximum_text_bytes: 4096, preparation_bytes: 65536, retained_bytes: 16384}}}}
  capability = {{kind: "{kind}", pinned_implementation: "{}"}}
  need = {{id: "recording-generation", class: "{DURABLE_TEXT_RESOURCE}", slots: 1, bytes: 4096}}
  limits ="#, implementation(write), implementation(write)))
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
fn text_file_save_and_load_intent_roundtrip_without_content_or_directory_authority() {
    for write in [true, false] {
        let checked = check(&source(write)).unwrap();
        let selected = CheckedTextFileInitialization::from_configuration(&checked).unwrap();
        assert_eq!(selected.configuration_id, checked.configuration_id());
        assert_eq!(selected.write, write);
        assert_eq!(selected.driver, implementation(write));
        assert_eq!(selected.maximum_text_bytes, 4096);
        assert_eq!(
            (selected.preparation_bytes, selected.retained_bytes),
            (65536, 16384)
        );
        let canonical = canonical_host_configuration_conduit(checked.configuration()).unwrap();
        assert_eq!(
            check(&canonical).unwrap().configuration_id(),
            checked.configuration_id()
        );
        assert!(!canonical.contains("directory"));
        assert!(!canonical.contains("reference"));
        let changed =
            check(&source(write).replace("maximum_text_bytes: 4096", "maximum_text_bytes: 2048"))
                .unwrap();
        assert_ne!(checked.configuration_id(), changed.configuration_id());
    }
}
#[test]
fn text_file_intent_refuses_unbounded_missing_or_unknown_fields() {
    for (from, to) in [
        ("maximum_text_bytes: 4096", "maximum_text_bytes: 0"),
        ("maximum_text_bytes: 4096", "maximum_text_bytes: 4097"),
        ("maximum_text_bytes: 4096, ", ""),
        ("preparation_bytes: 65536", "preparation_bytes: 0"),
        ("retained_bytes: 16384", "retained_bytes: 0"),
        ("retained_bytes: 16384", "retained_bytes: 4294967296"),
        ("retained_bytes: 16384", "retained_bytes: 16384, fd: 3"),
    ] {
        assert!(
            check(&source(true).replace(from, to)).is_err(),
            "accepted {to}"
        );
    }
    for (from, to) in [
        ("retained_bytes: 16384", "retained_bytes: 65537"),
        ("preparation_bytes: 65536", "preparation_bytes: 67108865"),
    ] {
        let checked = check(&source(true).replace(from, to)).unwrap();
        assert!(CheckedTextFileInitialization::from_configuration(&checked).is_err());
    }
}
#[test]
fn text_file_intent_requires_exact_capability_driver_and_linux_target() {
    let missing = source(true)
        .lines()
        .filter(|line| !line.trim_start().starts_with("capability ="))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(CheckedTextFileInitialization::from_configuration(&check(&missing).unwrap()).is_err());
    let mismatch = source(true).replace(&format!("kind: \"{DURABLE_TEXT_SAVE_KIND}\", pinned_implementation: \"{DURABLE_TEXT_SAVE_IMPLEMENTATION}\""),&format!("kind: \"{DURABLE_TEXT_LOAD_KIND}\", pinned_implementation: \"{DURABLE_TEXT_LOAD_IMPLEMENTATION}\""));
    assert!(check(&mismatch).is_err());
    let base = source(true)
        .lines()
        .find(|l| l.contains(DURABLE_TEXT_BASE_KIND))
        .unwrap()
        .to_owned();
    let duplicate = source(true).replace(&base, &format!("{base}\n{base}"));
    assert!(check(&duplicate)
        .and_then(|c| CheckedTextFileInitialization::from_configuration(&c))
        .is_err());
    let preference = source(true).replace(&format!("implementation: \"{DURABLE_TEXT_SAVE_IMPLEMENTATION}\""),&format!("implementations: [\"{DURABLE_TEXT_SAVE_IMPLEMENTATION}\", \"{DURABLE_TEXT_LOAD_IMPLEMENTATION}\"]"));
    assert!(check(&preference)
        .and_then(|c| CheckedTextFileInitialization::from_configuration(&c))
        .is_err());
    assert!(check(&source(true).replace(
        "machine: \"computer\", os: \"linux\"",
        "machine: \"windows-computer\", os: \"windows\""
    ))
    .is_err());
}
#[test]
fn text_file_image_and_review_retain_exact_operation_dependencies() {
    let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    for write in [true, false] {
        let checked = check(&source(write)).unwrap();
        let (image, _) = conduit_host_make::build_default_host_image(
            checked.profile().clone(),
            &catalog,
            &packages,
            &conduit_host_make::BuildInputs {
                source_identity: "fixture/text-file-intent".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        conduit_host_make::verify_configuration_image(&checked, &image, &catalog).unwrap();
        assert!(image.payload.base_selections.iter().any(|base| base.kind
            == DURABLE_TEXT_BASE_KIND
            && base.driver == implementation(write)));
        let review = conduit_host_make::resolve_host_capability_intent(
            &conduit_host_make::HostCapabilityIntent {
                target: crate::HOSTED_TARGET_ID.into(),
                capabilities: vec![conduit_host_make::CapabilityIntent {
                    kind: if write {
                        DURABLE_TEXT_SAVE_KIND
                    } else {
                        DURABLE_TEXT_LOAD_KIND
                    }
                    .into(),
                    pinned_implementation: Some(implementation(write).into()),
                }],
            },
            &catalog,
            &packages,
        )
        .unwrap();
        let selected = &review.capabilities[0].candidates[0];
        assert_eq!(
            selected.prerequisites.host_calls,
            vec![if write {
                DURABLE_TEXT_PUBLISH_CALL
            } else {
                DURABLE_TEXT_READ_CALL
            }]
        );
        assert_eq!(
            selected.prerequisites.resources,
            vec![DURABLE_TEXT_RESOURCE]
        );
        assert_eq!(selected.prerequisites.drivers, vec![implementation(write)]);
        assert_eq!(selected.prerequisites.bases.len(), 1);
        assert_eq!(
            selected.prerequisites.bases[0].base_kind,
            DURABLE_TEXT_BASE_KIND
        );
    }
}
