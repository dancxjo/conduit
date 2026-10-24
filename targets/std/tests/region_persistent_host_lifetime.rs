//! Public service over one actual StdHost, not a private worker or Host shell.
//! Finite pure transfer fixture: no callback/Host Call/Watch authority is modeled.
#![cfg(target_os = "linux")]
#[path = "support/region_lifetime_fixture.rs"]
mod fixture;
use conduit_core::*;
use conduit_std_host::{
    InitializedRegionService, RegionAdvance, RegionExecution, RegionServiceConfig,
    RegionServiceRetirement,
};
use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
fn advance(execution: &mut RegionExecution<'_>) {
    // A harness failure bound, not a semantic deadline or hard-termination claim.
    let until = Instant::now() + Duration::from_secs(5);
    let mut rounds = 0;
    while rounds < 4 {
        if execution.advance().unwrap() == RegionAdvance::RoundComplete {
            rounds += 1;
        }
        assert!(Instant::now() < until, "finite transfer failed to progress");
        std::thread::yield_now();
    }
}
fn retired(retired: &RegionServiceRetirement, plan: &Plan, play: &ActivePlayIdentity) {
    assert!(retired.host_error.is_none(), "{:?}", retired.host_error);
    assert!(retired.controller_error.is_none());
    assert_eq!(retired.active_play.as_ref(), Some(play));
    assert_eq!(retired.plan.as_ref(), Some(plan));
    assert_eq!(retired.workers.len(), 2);
    for worker in &retired.workers {
        assert_eq!(worker.plan_id.as_ref(), Some(&plan.plan_id));
        assert!(worker.failure.is_none());
        assert_eq!(worker.cancellation, Some(Ok(())));
        assert!(!worker.original_nodes.is_empty());
    }
}
#[test]
fn persistent_host_retires_a_and_admits_b_without_reviving_old_plan_or_backing() {
    let template = fixture::template();
    let mut host = fixture::host(&template);
    let original = host.advertisement().clone();
    let counts = Arc::new(fixture::Counts::default());
    let registry = fixture::registry(&template, &counts);
    let config = RegionServiceConfig::new(2);
    let mut a = InitializedRegionService::initialize(&mut host, config).unwrap();
    let offers_a = a.advertisement().unwrap().clone();
    let provider_a = offers_a
        .bases
        .iter()
        .find(|base| &base.base_id == a.base_id())
        .unwrap()
        .clone();
    let available_a = a.current_resource_observations().unwrap();
    assert_eq!(available_a.len(), 2);
    assert!(available_a.iter().all(|o| o.unreserved_units > 0));
    let draft_a = fixture::plan(&template, &offers_a);
    let plan_a = a
        .select_plan_regions(draft_a.clone(), fixture::groups(&draft_a), 4096)
        .unwrap();
    a.prepare(plan_a.clone(), &registry).unwrap();
    assert!(a
        .current_resource_observations()
        .unwrap()
        .iter()
        .all(|o| o.unreserved_units == 0));
    let admitted_a = a.admission_observations().to_vec();
    let mut execution_a = a.into_execution().unwrap();
    let play_a = execution_a.active_play().clone();
    advance(&mut execution_a);
    assert_eq!(counts.consumed.load(Ordering::SeqCst), 1);
    let retirement_a = execution_a.shutdown();
    retired(&retirement_a, &plan_a, &play_a);
    assert_eq!(
        counts.dropped.load(Ordering::SeqCst),
        2,
        "actual A Backs destroyed once before quota reuse"
    );
    assert_eq!(retirement_a.admission_observations, admitted_a);
    assert_eq!(host.advertisement().host_id, original.host_id);
    assert_eq!(host.advertisement().boot_id, original.boot_id);
    assert!(host.advertisement().offer_generation > offers_a.offer_generation);
    assert!(host.advertisement().bases.is_empty());
    assert!(host.advertisement().resources.is_empty());

    let mut b = InitializedRegionService::initialize(&mut host, config).unwrap();
    let offers_b = b.advertisement().unwrap().clone();
    let provider_b = offers_b
        .bases
        .iter()
        .find(|base| &base.base_id == b.base_id())
        .unwrap()
        .clone();
    assert_eq!(
        provider_a.base_id, provider_b.base_id,
        "Base name may be reused"
    );
    assert_ne!(
        provider_a.provider_instance_id,
        provider_b.provider_instance_id
    );
    assert!(provider_b.provider_generation > provider_a.provider_generation);
    assert_eq!(
        provider_b.enforcement_class,
        BaseEnforcementClass::Cooperative
    );
    let available_b = b.current_resource_observations().unwrap();
    for (old, fresh) in available_a.iter().zip(&available_b) {
        assert_eq!(
            old.pool_id, fresh.pool_id,
            "pool name is stable, generation is not"
        );
        assert_eq!(old.unreserved_units, fresh.unreserved_units);
        assert!(fresh.offer_generation > old.offer_generation);
        assert_ne!(old.sign_id, fresh.sign_id);
    }
    assert_eq!(
        b.select_plan_regions(plan_a.clone(), fixture::groups(&plan_a), 4096)
            .unwrap_err(),
        "region selection Plan is stale for this initialized Host"
    );
    assert_eq!(b.prepared_region_count(), 0);
    assert!(b.active_play().is_none());
    assert_eq!(
        counts.dropped.load(Ordering::SeqCst),
        2,
        "stale selection constructs no new Back"
    );
    let draft_b = fixture::plan(&template, &offers_b);
    let plan_b = b
        .select_plan_regions(draft_b.clone(), fixture::groups(&draft_b), 4096)
        .unwrap();
    assert_eq!(plan_a.source_document_id, plan_b.source_document_id);
    assert_ne!(plan_a.plan_id, plan_b.plan_id);
    b.prepare(plan_b.clone(), &registry).unwrap();
    let mut execution_b = b.into_execution().unwrap();
    let play_b = execution_b.active_play().clone();
    assert_eq!(play_b.host_id, play_a.host_id);
    assert_eq!(play_b.boot_id, play_a.boot_id);
    assert!(play_b.play_sequence > play_a.play_sequence);
    assert_ne!(play_b.active_play_id, play_a.active_play_id);
    advance(&mut execution_b);
    assert_eq!(
        counts.consumed.load(Ordering::SeqCst),
        2,
        "B progresses with fresh actual Backs"
    );
    let retirement_b = execution_b.shutdown();
    retired(&retirement_b, &plan_b, &play_b);
    assert_eq!(
        counts.dropped.load(Ordering::SeqCst),
        4,
        "A and B each dispose exactly two Backs"
    );
    assert_eq!(
        retirement_a.active_play.as_ref(),
        Some(&play_a),
        "retained A receipt never changes"
    );
    assert!(host.advertisement().offer_generation > offers_b.offer_generation);
    println!("persistent StdHost A={} B={} retired Backs=4; cooperative pure transfer, no late callback/HostCall/Watch proof", play_a.active_play_id.as_str(), play_b.active_play_id.as_str());
}
