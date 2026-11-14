//! Exact opt-in preparation, execution and native-call policies; physical capacity stays fixed.
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
fn selected_profile_changes_only_explicit_delegation_and_call_policy() {
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
    assert!(original.configuration().execution.is_none());
    assert_eq!(
        selected.configuration().execution.as_ref(),
        Some(&conduit_body::HostExecutionDelegation {
            authorizer: conduit_body::HostExecutionAuthorizer::SeedOwner,
            maximum_active_worksets: 1,
            continuation: None,
        })
    );
    let original_bus =
        conduit_host_rp2040::CheckedI2cConstruction::from_configuration(&original).unwrap();
    let selected_bus =
        conduit_host_rp2040::CheckedI2cConstruction::from_configuration(&selected).unwrap();
    assert_eq!(original_bus.maximum_call_operations, 0);
    assert_eq!(selected_bus.maximum_call_operations, 1024);
    let mut stripped = selected.configuration().clone();
    stripped.preparation = None;
    stripped.execution = None;
    let original_base = original
        .configuration()
        .bases
        .iter()
        .find(|base| base.kind == "bus/i2c")
        .unwrap();
    let selected_base = stripped
        .bases
        .iter_mut()
        .find(|base| base.kind == "bus/i2c")
        .unwrap();
    let original_value =
        original_base.initialization.as_ref().unwrap().values["maximum_call_operations"].clone();
    selected_base
        .initialization
        .as_mut()
        .unwrap()
        .values
        .insert("maximum_call_operations".into(), original_value);
    assert_eq!(&stripped, original.configuration());
    assert_ne!(selected.configuration_id(), original.configuration_id());
    let text = canonical_host_configuration_conduit(selected.configuration()).unwrap();
    let reopened = checked(&text);
    assert_eq!(reopened.configuration_id(), selected.configuration_id());
    let pool = conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(&selected).unwrap();
    assert_eq!((pool.capacity_units, pool.bookkeeping_bytes), (5000, 16384));
}
