//! Optional checked construction, not initialized radio or network proof.
use conduit_host_make::*;
use conduit_host_rp2040::{retained_wifi_selected, Rp2040MakePackage};
fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
    let packages = MakePackageSet::compose(&[&Rp2040MakePackage]).map_err(|e| format!("{e:?}"))?;
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let parsed = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(parsed, &catalog, &packages).map_err(|e| format!("{e:?}"))
}
#[test]
fn explicit_absence_omits_radio_while_other_profiles_keep_checked_initialization() {
    let offline = include_str!("../../profiles/pico-w-house-yellow.host.conduit");
    assert!(!retained_wifi_selected(&checked(offline).unwrap()).unwrap());
    let radio = include_str!("../../profiles/pico-w-house-green.host.conduit");
    assert!(retained_wifi_selected(&checked(radio).unwrap()).unwrap());
    assert!(
        checked(&radio.replace("private-flash-network-journal", "ambient-credentials"))
            .and_then(|checked| retained_wifi_selected(&checked))
            .is_err()
    );
}
