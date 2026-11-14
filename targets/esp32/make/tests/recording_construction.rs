//! Ordinary checked physical NOR construction, never a RAM need or write grant.
use conduit_host_esp32_make::physical::{CheckedRecordingConstruction, RECORDING};
use conduit_host_make::*;
const SOURCE: &str = include_str!("../../profiles/blue-wroom32.host.conduit");
fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
    let packages = MakePackageSet::compose(&[&conduit_host_esp32_make::Esp32MakePackage])
        .map_err(|e| format!("{e:?}"))?;
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let parsed = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(parsed, &catalog, &packages).map_err(|e| format!("{e:?}"))
}
#[test]
fn blue_declares_exact_finite_flash_construction_and_refuses_other_implementations() {
    let selected = checked(SOURCE).unwrap();
    let construction =
        CheckedRecordingConstruction::from_configuration(&selected, RECORDING).unwrap();
    assert_eq!(
        (construction.prefix_bytes, construction.slots),
        (8192, 1264)
    );
    assert_eq!(construction.end_bytes().unwrap(), 655360);
    assert!(CheckedRecordingConstruction::from_configuration(
        &selected,
        "rp2040/nor-record-append@1"
    )
    .is_err());
    let reopened =
        checked(&canonical_host_configuration_conduit(selected.configuration()).unwrap()).unwrap();
    assert_eq!(selected.configuration_id(), reopened.configuration_id());
    for (from, to) in [
        ("prefix_bytes: 8192", "prefix_bytes: 8193"),
        ("slots: 1264", "slots: 0"),
        ("slots: 1264", "slots: 8388607"),
    ] {
        assert!(checked(&SOURCE.replace(from, to))
            .and_then(|c| CheckedRecordingConstruction::from_configuration(&c, RECORDING))
            .is_err());
    }
}

fn service_source(read_operations: &str) -> String {
    use conduit_host_esp32_make::physical::RECORD_SERVICE;
    SOURCE.replace(RECORDING, RECORD_SERVICE).replace(
        "slots: 1264",
        &format!("slots: 1264, read_operations: {read_operations}"),
    )
}
#[test]
fn explicit_service_roundtrips_with_read_ceiling_and_unchanged_memory_declarations() {
    use conduit_host_esp32_make::physical::{CheckedRecordServiceConstruction, RECORD_SERVICE};
    let original = checked(SOURCE).unwrap();
    let selected = checked(&service_source("8192")).unwrap();
    let construction =
        CheckedRecordServiceConstruction::from_configuration(&selected, RECORD_SERVICE).unwrap();
    assert_eq!(construction.maximum_read_operations, 8192);
    assert_eq!(
        construction.residence,
        CheckedRecordingConstruction::from_configuration(&original, RECORDING).unwrap()
    );
    assert_eq!(
        selected.configuration().resources,
        original.configuration().resources
    );
    assert_eq!(
        selected.configuration().limits,
        original.configuration().limits
    );
    assert!(CheckedRecordingConstruction::from_configuration(&selected, RECORDING).is_err());
    assert!(
        CheckedRecordServiceConstruction::from_configuration(&original, RECORD_SERVICE).is_err()
    );
    let reopened =
        checked(&canonical_host_configuration_conduit(selected.configuration()).unwrap()).unwrap();
    assert_eq!(reopened.configuration_id(), selected.configuration_id());
    assert_eq!(
        CheckedRecordServiceConstruction::from_configuration(&reopened, RECORD_SERVICE).unwrap(),
        construction
    );
    assert_eq!(
        conduit_mcu_source_build::native_build::checked_record_service(
            &selected,
            RECORD_SERVICE,
            8192,
            655360
        )
        .unwrap(),
        Some(construction)
    );
    assert!(
        conduit_mcu_source_build::native_build::checked_record_service(
            &selected,
            RECORD_SERVICE,
            8193,
            655360
        )
        .is_err()
    );
    assert!(
        conduit_mcu_source_build::native_build::checked_record_service(
            &selected,
            RECORD_SERVICE,
            8192,
            655360 - 1
        )
        .is_err()
    );
}
#[test]
fn service_default_is_one_and_only_finite_u32_read_width_is_supported() {
    use conduit_host_esp32_make::physical::{CheckedRecordServiceConstruction, RECORD_SERVICE};
    let contract = conduit_mcu_source_build::recording::service_initialization_contract();
    let read = contract
        .fields
        .iter()
        .find(|field| field.key == "read_operations")
        .unwrap();
    assert_eq!(read.default_value, ConfigurationValue::U64(1));
    // Existing Host initialization requires every declared field explicitly;
    // its default metadata does not insert an omitted construction request.
    assert!(checked(&SOURCE.replace(RECORDING, RECORD_SERVICE)).is_err());
    let defaulted = checked(&service_source("1")).unwrap();
    assert_eq!(
        CheckedRecordServiceConstruction::from_configuration(&defaulted, RECORD_SERVICE)
            .unwrap()
            .maximum_read_operations,
        1
    );
    let maximum = checked(&service_source("4294967295")).unwrap();
    assert_eq!(
        CheckedRecordServiceConstruction::from_configuration(&maximum, RECORD_SERVICE)
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
                    RECORD_SERVICE
                ))
                .is_err(),
            "accepted read bound {bound}"
        );
    }
    // The old append provider has not gained a read field or hidden default.
    assert!(checked(&service_source("8192").replace(RECORD_SERVICE, RECORDING)).is_err());
}
#[test]
fn service_reuses_exact_residence_geometry_and_absence_selects_nothing() {
    use conduit_host_esp32_make::physical::{CheckedRecordServiceConstruction, RECORD_SERVICE};
    let source = service_source("8192");
    for (from, to) in [
        ("prefix_bytes: 8192", "prefix_bytes: 8193"),
        ("slots: 1264", "slots: 0"),
        ("prefix_bytes: 8192", "prefix_bytes: 4294967040"),
        ("slots: 1264", "slots: 8388607"),
    ] {
        assert!(checked(&source.replace(from, to))
            .and_then(|c| CheckedRecordServiceConstruction::from_configuration(&c, RECORD_SERVICE))
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
            RECORD_SERVICE,
            8192,
            655360
        )
        .unwrap(),
        None
    );
}
