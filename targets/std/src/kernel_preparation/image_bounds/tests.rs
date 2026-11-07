use super::*;
use crate::kernel_multivalue::{advertisement, plan_local, profile_catalog};
use conduit_core::{BootId, HostId, OfferGeneration};

fn bounds() -> HostBounds {
    HostBounds {
        static_memory_bytes: 16 * 1024 * 1024,
        heap_arena_bytes: 64 * 1024 * 1024,
        queue_items: 4096,
        buffered_bytes: 16 * 1024 * 1024,
        active_instances: 512,
        operation_slots: 256,
        timer_slots: 128,
        line_sessions: 64,
        evidence_items: 4096,
    }
}
fn fixture() -> (HostAdvertisement, Plan) {
    let host = advertisement(
        "image-budget-host".into(),
        "image-budget-boot".into(),
        OfferGeneration(1),
    );
    let plot = conduit_plot::parse(
        include_str!("../../../../../proof/fixtures/plots/kernel-multivalue.conduit"),
        &profile_catalog(),
    )
    .unwrap();
    let plan = plan_local(&plot, &host).unwrap();
    (host, plan)
}

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
fn scalar_fit_does_not_invent_complete_memory_accounting() {
    let (host, plan) = fixture();
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    ledger.bind_image_bounds(&bounds()).unwrap();
    let pools = ledger.pools.clone();
    let error = ledger
        .prepare_and_reserve_plans(&host, &[(&plan, false)])
        .unwrap_err();
    assert!(
        error.contains("complete ordinary managed-memory accounting"),
        "{error}"
    );
    assert_eq!(ledger.pools, pools);
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
