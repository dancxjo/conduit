//! Atomic combined pool admission for exact local Form partitions.
use super::*;
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

fn workload() -> (HostAdvertisement, [conduit_core::Plan; 3]) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    let host = crate::StdHost::new();
    let source = include_str!("../../../forms/clock/main.conduit");
    let plans = ["1s", "2s", "3s"].map(|period| {
        let checked = check_syntax_document(
            &parse_syntax_document(&source.replace("1s", period)),
            &startup,
        )
        .unwrap();
        let expanded = expand_canonical_form(&checked, "clock-demo", &profile).unwrap();
        host.plan_expanded_local(&expanded).unwrap()
    });
    let mut advertisement = host.advertisement().clone();
    for pool in &mut advertisement.resources {
        let per_form: u32 = plans[0].fragments[0]
            .placements
            .iter()
            .flat_map(|placement| &placement.resources)
            .filter(|binding| binding.pool_id == pool.pool_id)
            .map(|binding| binding.units)
            .sum();
        if per_form > 0 {
            pool.capacity_units = per_form * 2;
        }
    }
    (advertisement, plans)
}

#[test]
fn complete_set_reserves_and_releases_each_original_plan_without_growing_live_ledger() {
    let (host, plans) = workload();
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    let empty = ledger.pools.clone();
    let capacity = ledger.allocation_capacity();
    let reservations = ledger
        .prepare_and_reserve_partitions(
            &host,
            &[
                (&plans[0].fragments[0], false),
                (&plans[1].fragments[0], false),
            ],
        )
        .unwrap();
    assert_eq!(reservations.len(), 2);
    for (reservation, plan) in reservations.into_iter().zip(&plans) {
        assert_eq!(reservation.plan_id, plan.plan_id);
        ledger.release(reservation).unwrap();
    }
    assert_eq!(ledger.pools, empty);
    assert_eq!(ledger.allocation_capacity(), capacity);
}

#[test]
fn late_capacity_and_invalid_partition_leave_existing_reservations_unchanged() {
    let (host, plans) = workload();
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    let existing = ledger
        .prepare_and_reserve(&host, &plans[0].fragments[0])
        .unwrap();
    let occupied = ledger.pools.clone();
    let result = ledger.prepare_and_reserve_partitions(
        &host,
        &[
            (&plans[1].fragments[0], false),
            (&plans[2].fragments[0], false),
        ],
    );
    assert!(matches!(result, Err(reason) if reason.contains("above capacity")));
    assert_eq!(ledger.pools, occupied);
    let mut stale = plans[2].fragments[0].clone();
    stale.placements[0].boot_id = "lost-boot".into();
    assert!(ledger
        .prepare_and_reserve_partitions(&host, &[(&plans[1].fragments[0], false), (&stale, false)],)
        .is_err());
    assert_eq!(ledger.pools, occupied);
    ledger.release(existing).unwrap();
    assert!(ledger.pools.iter().all(|pool| pool.used_units == 0));
}

#[test]
fn duplicate_empty_and_over_bound_sets_cannot_reserve() {
    let (host, plans) = workload();
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    let empty = ledger.pools.clone();
    let part = (&plans[0].fragments[0], false);
    for partitions in [
        vec![],
        vec![part, part],
        vec![part; conduit_body::MAX_BODY_FORMS + 1],
    ] {
        assert!(ledger
            .prepare_and_reserve_partitions(&host, &partitions)
            .is_err());
        assert_eq!(ledger.pools, empty);
    }
}
