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
