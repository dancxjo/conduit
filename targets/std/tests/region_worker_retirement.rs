#![cfg(target_os = "linux")]
//! Persistent actual StdHost/worker retirement proof with fixture Backs.
//! Cord delivery is not a Host Call/callback API; this does not claim that proof.
use conduit_composite::{
    KernelOperationBudget, KernelOperationFactory, KernelOperationRegistry, PreparedWorkerBack,
};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, CordId, HostedValueStore, PortId,
};
use conduit_std_host::{
    InitializedRegionService, RegionServiceConfig, RegionWorkerCommand as Command,
    RegionWorkerError as Error, RegionWorkerReply as Reply, StdHost,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[allow(dead_code)]
#[path = "../../../architecture/composite/tests/support/region_fixture.rs"]
mod fixture;
fn host() -> StdHost {
    let mut fragment = fixture::plan().fragments.remove(0);
    fragment.execution_regions.clear();
    let mut advertisement = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    for placement in &mut fragment.placements {
        placement.capability_id = format!("{}/offer", placement.placement_id.as_str()).into();
        advertisement.capabilities.push(capability_offer_from_parts! {
            semantic_contract: placement.semantic_contract.clone(), startup_parameters: vec![], shorthand: None,
            capability_id: placement.capability_id.clone(), kind_id: placement.kind_id.clone(), kind_contract_revision: placement.kind_contract_revision.clone(),
            implementation: ImplementationOffer { execution_profile_id: placement.execution_profile_id.clone(), implementation_id: placement.implementation_id.clone(), artifact_id: placement.artifact_id.clone() },
            inputs: placement.inputs.clone(), outputs: placement.outputs.clone(), host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![], limits: placement.limits.clone(),
        });
    }
    StdHost::from_advertisement(advertisement).unwrap()
}
#[derive(Default)]
struct Counts {
    steps: AtomicUsize,
    cancels: AtomicUsize,
    drops: AtomicUsize,
}
struct Back {
    source: bool,
    count: Arc<Counts>,
    panic_step: bool,
    panic_cancel: bool,
}
impl StepBack<16> for Back {
    fn step(&mut self, io: &mut StepIo<16>, _: &StepInputBytes<'_, 16>) -> StepOutcome {
        let previous = self.count.steps.fetch_add(1, Ordering::SeqCst);
        assert!(
            !(self.panic_step && previous != 0),
            "intentional A step failure"
        );
        if self.source && previous == 0 {
            io.send_canonical(PortId(0), CanonicalValue::new(&[42]).unwrap())
                .unwrap();
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
    fn cancel(&mut self) {
        self.count.cancels.fetch_add(1, Ordering::SeqCst);
        assert!(!self.panic_cancel, "intentional A cancellation failure");
    }
}
impl Drop for Back {
    fn drop(&mut self) {
        self.count.drops.fetch_add(1, Ordering::SeqCst);
    }
}
struct Factory {
    id: ImplementationId,
    counts: [Arc<Counts>; 2],
    panic_step: bool,
    panic_cancel: bool,
}
impl KernelOperationFactory for Factory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.id
    }
    fn budget(&self, _: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: 1,
            maximum_value_bytes: 1,
            host_requests: 0,
            sign_items: 16,
        })
    }
    fn prepare(
        &self,
        p: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<16> + Send>, String> {
        let source = p.inputs.is_empty();
        Ok(Box::new(Back {
            source,
            count: self.counts[usize::from(!source)].clone(),
            panic_step: source && self.panic_step,
            panic_cancel: source && self.panic_cancel,
        }))
    }
    fn prepare_worker(
        &self,
        p: &PlannedGear,
        v: &mut HostedValueStore,
    ) -> Result<PreparedWorkerBack, String> {
        PreparedWorkerBack::new(self.prepare(p, v)?, 0)
    }
}
fn plan(service: &InitializedRegionService<'_>) -> Plan {
    let mut fragment = fixture::plan().fragments.remove(0);
    fragment.execution_regions.clear();
    let generation = service.advertisement().unwrap().offer_generation;
    fragment.offer_generation = generation;
    for p in &mut fragment.placements {
        p.offer_generation = generation;
        p.capability_id = format!("{}/offer", p.placement_id.as_str()).into();
    }
    seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
}
fn registry(
    plan: &Plan,
    counts: &[Arc<Counts>; 2],
    fail_step: bool,
    fail_cancel: bool,
) -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(Factory {
            id: plan.fragments[0].placements[0].implementation_id.clone(),
            counts: counts.clone(),
            panic_step: fail_step,
            panic_cancel: fail_cancel,
        })
        .unwrap();
    registry
}
fn select(service: &InitializedRegionService<'_>, plan: Plan) -> Plan {
    let groups = plan.fragments[0]
        .placements
        .iter()
        .map(|p| vec![p.placement_id.clone()])
        .collect();
    service.select_plan_regions(plan, groups, 4096).unwrap()
}
fn send(
    service: &mut InitializedRegionService<'_>,
    lane: usize,
    command: impl Fn() -> Command<'static>,
) {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match service.dispatch(lane, command()) {
            Ok(()) => return,
            Err(Error::Busy) => std::thread::yield_now(),
            Err(e) => panic!("dispatch {e:?}"),
        };
        assert!(Instant::now() < until);
    }
}
fn receive(service: &mut InitializedRegionService<'_>, lane: usize) -> Result<Reply, Error> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match service.poll(lane, &mut []) {
            Ok(Some(r)) => return r,
            Ok(None) | Err(Error::Busy) => std::thread::yield_now(),
            Err(e) => return Err(e),
        };
        assert!(Instant::now() < until);
    }
}
#[test]
fn actual_host_retires_poisoned_failed_cancel_before_fresh_play_without_sibling_effects() {
    exercise(true);
}
#[test]
fn actual_host_retires_cancel_panic_before_fresh_play_without_reusing_old_plan() {
    exercise(false);
}
fn exercise(panic_step: bool) {
    let mut host = host();
    let a = [Arc::new(Counts::default()), Arc::new(Counts::default())];
    let mut service =
        InitializedRegionService::initialize(&mut host, RegionServiceConfig::new(2)).unwrap();
    let selected = select(&service, plan(&service));
    let old_plan = selected.clone();
    let original_cord = CordId(0);
    service
        .prepare(selected.clone(), &registry(&selected, &a, panic_step, true))
        .unwrap();
    let old_play = service.start().unwrap().clone();
    send(&mut service, 0, || Command::Step);
    assert!(matches!(receive(&mut service, 0), Ok(Reply::Step(_))));
    send(&mut service, 0, || Command::Offer(original_cord));
    let until = Instant::now() + Duration::from_secs(5);
    let mut offered = [0u8; 1];
    let old_sequence = loop {
        match service.poll(0, &mut offered) {
            Ok(Some(Ok(Reply::Offer {
                sequence: Some(sequence),
                bytes: 1,
            }))) => break sequence,
            Ok(None) | Err(Error::Busy) => std::thread::yield_now(),
            other => panic!("expected retained A offer: {other:?}"),
        }
        assert!(Instant::now() < until);
    };
    assert_eq!(offered, [42]);
    if panic_step {
        send(&mut service, 0, || Command::Step);
        assert!(matches!(
            receive(&mut service, 0),
            Err(Error::Panicked | Error::Poisoned)
        ));
        assert!(matches!(
            service.dispatch(
                0,
                Command::Delivered {
                    cord: original_cord,
                    sequence: old_sequence
                }
            ),
            Err(Error::Poisoned | Error::Stopped)
        ));
    }
    let retirement = service.shutdown();
    assert!(
        retirement.host_error.is_none(),
        "{:?}",
        retirement.host_error
    );
    assert_eq!(retirement.active_play.as_ref(), Some(&old_play));
    assert_eq!(retirement.workers[0].failure, Some(Error::Panicked));
    assert_eq!(
        retirement.workers[0].cancellation,
        Some(Err(Error::Panicked))
    );
    assert_eq!(retirement.workers[1].failure, None);
    assert_eq!(retirement.workers[1].cancellation, Some(Ok(())));
    for c in &a {
        assert_eq!(c.cancels.load(Ordering::SeqCst), 1);
        assert_eq!(c.drops.load(Ordering::SeqCst), 1);
    }
    assert_eq!(
        a[1].steps.load(Ordering::SeqCst),
        0,
        "sibling never executed"
    );
    let b = [Arc::new(Counts::default()), Arc::new(Counts::default())];
    let mut next =
        InitializedRegionService::initialize(&mut host, RegionServiceConfig::new(2)).unwrap();
    let rejected = [Arc::new(Counts::default()), Arc::new(Counts::default())];
    assert!(
        next.prepare(
            old_plan.clone(),
            &registry(&old_plan, &rejected, false, false)
        )
        .is_err(),
        "retired physical Plan cannot reenter the new Base generation"
    );
    assert!(next.prepared_plan().is_none());
    for c in &rejected {
        assert_eq!(c.steps.load(Ordering::SeqCst), 0);
        assert_eq!(c.cancels.load(Ordering::SeqCst), 0);
        assert_eq!(c.drops.load(Ordering::SeqCst), 1);
    }
    for c in &b {
        assert_eq!(c.steps.load(Ordering::SeqCst), 0);
    }
    let selected = select(&next, plan(&next));
    next.prepare(selected.clone(), &registry(&selected, &b, false, false))
        .unwrap();
    let new_play = next.start().unwrap().clone();
    assert_ne!(new_play.active_play_id, old_play.active_play_id);
    assert_ne!(new_play.plan_id, old_play.plan_id);
    // A delivery without an outstanding B offer is refused, without stepping B.
    // This does not assert that raw cord+sequence values are cross-owner capabilities.
    send(&mut next, 0, || Command::Delivered {
        cord: original_cord,
        sequence: old_sequence,
    });
    assert!(matches!(receive(&mut next, 0), Err(Error::Transport(_))));
    for c in &b {
        assert_eq!(c.steps.load(Ordering::SeqCst), 0);
    }
    send(&mut next, 0, || Command::Step);
    assert!(matches!(receive(&mut next, 0), Ok(Reply::Step(_))));
    assert_eq!(b[0].steps.load(Ordering::SeqCst), 1);
    assert_eq!(b[1].steps.load(Ordering::SeqCst), 0);
    let retired = next.shutdown();
    assert_eq!(retired.active_play.as_ref(), Some(&new_play));
    assert!(retired.host_error.is_none());
    assert!(retired
        .workers
        .iter()
        .all(|w| w.failure.is_none() && w.cancellation == Some(Ok(()))));
    for c in &b {
        assert_eq!(c.cancels.load(Ordering::SeqCst), 1);
        assert_eq!(c.drops.load(Ordering::SeqCst), 1);
    }
    for c in &a {
        assert_eq!(c.cancels.load(Ordering::SeqCst), 1);
        assert_eq!(c.drops.load(Ordering::SeqCst), 1);
    }
    println!(
        "CONDUIT_STD_WORKER_RETIREMENT {}",
        serde_json::json!({
            "schema": "conduit.std/worker-retirement-fixture@1",
            "actual_std_host": true,
            "fixture_backs": true,
            "step_panic": panic_step,
            "cancel_panic": true,
            "retired_play": old_play,
            "fresh_play": new_play,
            "retained_cord": original_cord.0,
            "retained_sequence": old_sequence,
            "stale_plan_refused": true,
            "delivery_without_current_offer_refused": true,
            "cancel_attempts_per_admitted_back": 1,
            "drops_per_admitted_back": 1,
            "sibling_steps": 0,
            "proof_class": "hosted-cooperative-fixture",
            "not_established": ["cross-owner callback or Host Call replay", "physical authority", "hard termination"]
        })
    );
}
