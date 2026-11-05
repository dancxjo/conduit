//! Source-declared finite work capacity; no firmware or physical admission claim.
use conduit_host_make::*;
use conduit_host_rp2040::CheckedResidentWorkPool;
const GREEN: &str = include_str!("../../profiles/pico-w-house-green.host.conduit");
const NEED: &str = "  need = {id: \"pool/resident-step-work\", class: \"kernel/cooperative-step-work@1\", slots: 5000, bytes: 16384}\n";
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
fn finite_resident_work_budget_follows_checked_host_source() {
    let original = checked(GREEN);
    let pool = CheckedResidentWorkPool::from_configuration(&original).unwrap();
    assert_eq!(pool.id, "pool/resident-step-work");
    assert_eq!((pool.capacity_units, pool.bookkeeping_bytes), (5000, 16384));
    assert_eq!(
        original.configuration().resources[0].class,
        conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID
    );
    let changed = checked(&GREEN.replace("slots: 5000", "slots: 1000"));
    assert_eq!(
        CheckedResidentWorkPool::from_configuration(&changed)
            .unwrap()
            .capacity_units,
        1000
    );
    assert_ne!(original.configuration_id(), changed.configuration_id());
}
#[test]
fn absence_ambiguity_and_bookkeeping_beyond_the_arena_refuse() {
    let missing = checked(&GREEN.replace(NEED, ""));
    assert!(CheckedResidentWorkPool::from_configuration(&missing).is_err());
    let duplicate = checked(&GREEN.replace(
        NEED,
        &format!(
            "{NEED}{}",
            NEED.replace("pool/resident-step-work", "pool/second-work")
        ),
    ));
    assert!(CheckedResidentWorkPool::from_configuration(&duplicate).is_err());
    let oversized = checked(&GREEN.replace("bytes: 16384}", "bytes: 65536}"));
    assert!(CheckedResidentWorkPool::from_configuration(&oversized).is_err());
}
