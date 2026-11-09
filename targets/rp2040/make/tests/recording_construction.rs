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
