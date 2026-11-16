use super::super::image_test_fixture::{bounds, fixture};
use super::*;
use crate::kernel_multivalue::advertisement;
use conduit_core::{BootId, HostId, OfferGeneration};

#[test]
fn selected_bounds_are_bound_once_without_enlarging_resource_pools() {
    let (host, _) = fixture();
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    let pools = ledger.pools.clone();
    ledger.bind_image_bounds(&bounds()).unwrap();
    assert_eq!(ledger.pools, pools);
    assert!(ledger.bind_image_bounds(&bounds()).is_err());
    assert!(ledger.is_idle());
}

#[test]
fn complete_workset_scalar_sum_refuses_before_any_reservation() {
    let (host, plan) = fixture();
    let fragment = &plan.fragments[0];
    let mut limits = bounds();
    limits.active_instances = fragment.placements.len().try_into().unwrap();
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    ledger.bind_image_bounds(&limits).unwrap();
    let error = ledger
        .preflight_image_workset([fragment, fragment])
        .unwrap_err();
    assert!(error.contains("aggregate active_instances"), "{error}");
    assert!(ledger.is_idle());
}

#[test]
fn legacy_ledger_has_no_selected_image_admission_contract() {
    let host = advertisement(
        HostId::from("legacy"),
        BootId::from("legacy-boot"),
        OfferGeneration(1),
    );
    let ledger = KernelResourceLedger::new(&host).unwrap();
    ledger.preflight_image_workset(core::iter::empty()).unwrap();
}

#[test]
fn zero_unused_line_capacity_is_absence_and_a_real_requirement_refuses() {
    let (host, _) = fixture();
    let mut limits = bounds();
    limits.line_sessions = 0;
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    ledger.bind_image_bounds(&limits).unwrap();
    Shape::default().check(&limits).unwrap();
    let error = Shape {
        line_sessions: 1,
        ..Shape::default()
    }
    .check(&limits)
    .unwrap_err();
    assert!(
        error.contains("line_sessions requires 1, admitted 0"),
        "{error}"
    );
    assert!(ledger.is_idle());
}
