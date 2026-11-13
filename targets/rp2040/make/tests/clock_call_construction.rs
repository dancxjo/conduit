//! Explicit deadline-call intent is not periodic timing geometry or a grant.
use conduit_host_make::*;
use conduit_host_rp2040::{CheckedClockConstruction, Rp2040MakePackage, CLOCK_IMPLEMENTATION};

const PERIODIC: &str = include_str!("../../profiles/pico-w-house-yellow.host.conduit");

fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
    let packages = MakePackageSet::compose(&[&Rp2040MakePackage]).unwrap();
    let configuration = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
    check_host_configuration(
        configuration,
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .map_err(|e| format!("{e:?}"))
}

fn clock(checked: &CheckedHostConfiguration) -> CheckedClockConstruction {
    CheckedClockConstruction::from_configuration(checked, CLOCK_IMPLEMENTATION).unwrap()
}

#[test]
fn explicit_zero_preserves_periodic_clock_without_selecting_deadline_authority() {
    let selected = clock(&checked(PERIODIC).unwrap());
    assert_eq!(selected.slots, 4);
    assert_eq!(selected.maximum_duration_ms, 60000);
    assert_eq!(selected.maximum_call_operations, 0);
    assert!(selected.deadline_call_operations().is_err());

    // Missing fields remain errors; the field's default hint is not permission
    // for an implicit policy or a change to generic initialization semantics.
    let missing = PERIODIC.replace(", maximum_call_operations: 0", "");
    assert!(checked(&missing).is_err());
}

#[test]
fn positive_deadline_ceiling_is_exact_and_independent_of_slots_or_duration() {
    let periodic = checked(PERIODIC).unwrap();
    for maximum in [1, 1024, u32::MAX] {
        let source = PERIODIC.replace(
            "maximum_call_operations: 0",
            &format!("maximum_call_operations: {maximum}"),
        );
        let selected = checked(&source).unwrap();
        let native = clock(&selected);
        assert_eq!(native.deadline_call_operations().unwrap(), maximum);
        assert_eq!(native.maximum_call_operations, maximum);
        assert_eq!(native.slots, clock(&periodic).slots);
        assert_eq!(
            native.maximum_duration_ms,
            clock(&periodic).maximum_duration_ms
        );
        assert_eq!(
            selected.configuration().limits,
            periodic.configuration().limits
        );
        assert_ne!(selected.configuration_id(), periodic.configuration_id());
    }
}

#[test]
fn wrong_negative_fractional_and_overbound_deadline_ceilings_refuse() {
    for invalid in ["4294967296", "-1", "1.5", "\"unbounded\"", "true"] {
        let source = PERIODIC.replace(
            "maximum_call_operations: 0",
            &format!("maximum_call_operations: {invalid}"),
        );
        assert!(checked(&source).is_err(), "accepted {invalid}");
    }
}
