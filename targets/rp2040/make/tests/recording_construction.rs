//! Ordinary host.conduit construction; no flash resource admission is claimed.
use conduit_host_make::*;
use conduit_host_rp2040::{CheckedRecordingConstruction, RECORDING_IMPLEMENTATION};
const SOURCE: &str = include_str!("../../profiles/pico-w-house-yellow.host.conduit");
fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage])
        .map_err(|e| format!("{e:?}"))?;
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let parsed = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(parsed, &catalog, &packages).map_err(|e| format!("{e:?}"))
}
#[test]
fn yellow_declares_exact_finite_flash_construction_separate_from_ram() {
    let selected = checked(SOURCE).unwrap();
    let construction =
        CheckedRecordingConstruction::from_configuration(&selected, RECORDING_IMPLEMENTATION)
            .unwrap();
    assert_eq!(
        (construction.prefix_bytes, construction.slots),
        (8192, 1136)
    );
    assert_eq!(construction.end_bytes().unwrap(), 589824);
    assert!(CheckedRecordingConstruction::from_configuration(
        &selected,
        "esp32/nor-record-append@1"
    )
    .is_err());
    let reopened =
        checked(&canonical_host_configuration_conduit(selected.configuration()).unwrap()).unwrap();
    assert_eq!(selected.configuration_id(), reopened.configuration_id());
    assert_eq!(selected.configuration().resources.len(), 1);
    assert_eq!(selected.configuration().resources[0].bytes, 16384);
    for (from, to) in [
        ("prefix_bytes: 8192", "prefix_bytes: 8193"),
        ("slots: 1136", "slots: 0"),
        ("prefix_bytes: 8192", "prefix_bytes: 4294967040"),
    ] {
        assert!(checked(&SOURCE.replace(from, to))
            .and_then(|c| CheckedRecordingConstruction::from_configuration(
                &c,
                RECORDING_IMPLEMENTATION
            ))
            .is_err());
    }
}

fn service_source(read_operations: &str) -> String {
    use conduit_host_rp2040::RECORD_SERVICE_IMPLEMENTATION;
    SOURCE
        .replace(RECORDING_IMPLEMENTATION, RECORD_SERVICE_IMPLEMENTATION)
        .replace(
            "slots: 1136",
            &format!("slots: 1136, read_operations: {read_operations}"),
        )
}
#[test]
fn explicit_service_roundtrips_with_read_ceiling_and_unchanged_memory_declarations() {
    use conduit_host_rp2040::{CheckedRecordServiceConstruction, RECORD_SERVICE_IMPLEMENTATION};
    let original = checked(SOURCE).unwrap();
    let selected = checked(&service_source("8192")).unwrap();
    let construction = CheckedRecordServiceConstruction::from_configuration(
        &selected,
        RECORD_SERVICE_IMPLEMENTATION,
    )
    .unwrap();
    assert_eq!(construction.maximum_read_operations, 8192);
    assert_eq!(
        construction.residence,
        CheckedRecordingConstruction::from_configuration(&original, RECORDING_IMPLEMENTATION)
            .unwrap()
    );
    assert_eq!(
        selected.configuration().resources,
        original.configuration().resources
    );
    assert_eq!(
        selected.configuration().limits,
        original.configuration().limits
    );
    assert!(
        CheckedRecordingConstruction::from_configuration(&selected, RECORDING_IMPLEMENTATION)
            .is_err()
    );
    assert!(CheckedRecordServiceConstruction::from_configuration(
        &original,
        RECORD_SERVICE_IMPLEMENTATION
    )
    .is_err());
    let reopened =
        checked(&canonical_host_configuration_conduit(selected.configuration()).unwrap()).unwrap();
    assert_eq!(reopened.configuration_id(), selected.configuration_id());
    assert_eq!(
        CheckedRecordServiceConstruction::from_configuration(
            &reopened,
            RECORD_SERVICE_IMPLEMENTATION
        )
        .unwrap(),
        construction
    );
    assert_eq!(
        conduit_mcu_source_build::native_build::checked_record_service(
            &selected,
            RECORD_SERVICE_IMPLEMENTATION,
            8192,
            589824
        )
        .unwrap(),
        Some(construction)
    );
    assert!(
        conduit_mcu_source_build::native_build::checked_record_service(
            &selected,
            RECORD_SERVICE_IMPLEMENTATION,
            8193,
            589824
        )
        .is_err()
    );
    assert!(
        conduit_mcu_source_build::native_build::checked_record_service(
            &selected,
            RECORD_SERVICE_IMPLEMENTATION,
            8192,
            589824 - 1
        )
        .is_err()
    );
}
#[test]
fn service_default_is_one_and_only_finite_u32_read_width_is_supported() {
    use conduit_host_rp2040::{CheckedRecordServiceConstruction, RECORD_SERVICE_IMPLEMENTATION};
    let contract = conduit_mcu_source_build::recording::service_initialization_contract();
    let read = contract
        .fields
        .iter()
        .find(|field| field.key == "read_operations")
        .unwrap();
    assert_eq!(read.default_value, ConfigurationValue::U64(1));
    // Existing Host initialization requires every declared field explicitly;
    // its default metadata does not insert an omitted construction request.
    assert!(
        checked(&SOURCE.replace(RECORDING_IMPLEMENTATION, RECORD_SERVICE_IMPLEMENTATION)).is_err()
    );
    let defaulted = checked(&service_source("1")).unwrap();
    assert_eq!(
        CheckedRecordServiceConstruction::from_configuration(
            &defaulted,
            RECORD_SERVICE_IMPLEMENTATION
        )
        .unwrap()
        .maximum_read_operations,
        1
    );
    let maximum = checked(&service_source("4294967295")).unwrap();
    assert_eq!(
        CheckedRecordServiceConstruction::from_configuration(
            &maximum,
            RECORD_SERVICE_IMPLEMENTATION
        )
        .unwrap()
        .maximum_read_operations,
        u32::MAX
    );
    for bound in [
        "0",
        "4294967296",
        "18446744073709551616",
        "-1",
        "true",
        "\"8192\"",
    ] {
        assert!(
            checked(&service_source(bound))
                .and_then(|c| CheckedRecordServiceConstruction::from_configuration(
                    &c,
                    RECORD_SERVICE_IMPLEMENTATION
                ))
                .is_err(),
            "accepted read bound {bound}"
        );
    }
    // The old append provider has not gained a read field or hidden default.
    assert!(checked(
        &service_source("8192").replace(RECORD_SERVICE_IMPLEMENTATION, RECORDING_IMPLEMENTATION)
    )
    .is_err());
}
#[test]
fn service_reuses_exact_residence_geometry_and_absence_selects_nothing() {
    use conduit_host_rp2040::{CheckedRecordServiceConstruction, RECORD_SERVICE_IMPLEMENTATION};
    let source = service_source("8192");
    for (from, to) in [
        ("prefix_bytes: 8192", "prefix_bytes: 8193"),
        ("slots: 1136", "slots: 0"),
        ("prefix_bytes: 8192", "prefix_bytes: 4294967040"),
        ("slots: 1136", "slots: 8388607"),
    ] {
        assert!(checked(&source.replace(from, to))
            .and_then(|c| CheckedRecordServiceConstruction::from_configuration(
                &c,
                RECORD_SERVICE_IMPLEMENTATION
            ))
            .is_err());
    }
    let absent = SOURCE
        .lines()
        .filter(|line| !line.contains(conduit_mcu_source_build::recording::BASE_KIND))
        .collect::<Vec<_>>()
        .join("\n");
    let absent = checked(&absent).unwrap();
    assert_eq!(
        conduit_mcu_source_build::native_build::checked_record_service(
            &absent,
            RECORD_SERVICE_IMPLEMENTATION,
            8192,
            589824
        )
        .unwrap(),
        None
    );
}
