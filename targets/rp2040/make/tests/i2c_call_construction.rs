//! Native address policy is explicit construction intent, not readiness/grants.
use conduit_host_make::*;
use conduit_host_rp2040::{CheckedI2cConstruction, Rp2040MakePackage};
const SOURCE: &str = include_str!("../../profiles/pico-w-house-green.host.conduit");
fn checked(source: &str) -> CheckedHostConfiguration {
    let packages = MakePackageSet::compose(&[&Rp2040MakePackage]).unwrap();
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap()
}
#[test]
fn explicit_local_address_scope_is_bound_to_the_checked_host_source() {
    let selected = CheckedI2cConstruction::from_configuration(&checked(SOURCE)).unwrap();
    assert_eq!(selected.call_address_range, Some((112, 118)));
    let absent = SOURCE
        .replace("minimum_call_address: 112", "minimum_call_address: 0")
        .replace("maximum_call_address: 118", "maximum_call_address: 0");
    let absent = CheckedI2cConstruction::from_configuration(&checked(&absent)).unwrap();
    assert_eq!(absent.call_address_range, None);
    assert_ne!(absent.configuration_id, selected.configuration_id);
    for (minimum, maximum) in [(0, 118), (112, 0), (7, 118), (119, 118)] {
        let source = SOURCE
            .replace(
                "minimum_call_address: 112",
                &format!("minimum_call_address: {minimum}"),
            )
            .replace(
                "maximum_call_address: 118",
                &format!("maximum_call_address: {maximum}"),
            );
        assert!(CheckedI2cConstruction::from_configuration(&checked(&source)).is_err());
    }
}
