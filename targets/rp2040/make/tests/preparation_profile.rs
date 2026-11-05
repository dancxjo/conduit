//! Opt-in delegation changes policy/identity, never physical grants or capacity.
use conduit_host_make::*;
const DEFAULT: &str = include_str!("../../profiles/pico-w-house-green.host.conduit");
const SELECTED: &str = include_str!("../../profiles/pico-w-house-green-delegated.host.conduit");

fn checked(source: &str) -> CheckedHostConfiguration {
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap()
}

#[test]
fn selected_preparation_keeps_default_equipment_and_all_construction_budgets() {
    let original = checked(DEFAULT);
    let selected = checked(SELECTED);
    assert!(original.configuration().preparation.is_none());
    let policy = selected.configuration().preparation.unwrap();
    assert_eq!(
        policy.authorizer,
        conduit_body::HostPreparationAuthorizer::SeedOwner
    );
    assert_eq!(policy.maximum_assignment_bytes, 4096);
    assert_eq!(policy.maximum_runtime_state_bytes, 16384);
    let mut stripped = selected.configuration().clone();
    stripped.preparation = None;
    assert_eq!(&stripped, original.configuration());
    assert_ne!(selected.configuration_id(), original.configuration_id());
    let text = canonical_host_configuration_conduit(selected.configuration()).unwrap();
    let reopened = checked(&text);
    assert_eq!(reopened.configuration_id(), selected.configuration_id());
    let pool = conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(&selected).unwrap();
    assert_eq!((pool.capacity_units, pool.bookkeeping_bytes), (5000, 16384));
}
