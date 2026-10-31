//! CPU ownership/execution tests. This mock is never installed by USB acquisition.
use super::*;
use conduit_environment_runtime::environmental_reading::source_observation::{
    SourceObservation, SourceOutcome, SourceSample,
};
use conduit_environment_runtime::environmental_reading::EnvironmentalReading;
pub(super) struct CpuSource {
    counter: u64,
    fail: bool,
}
impl EnvironmentalSourceProvider for CpuSource {
    async fn observe(
        &mut self,
    ) -> Result<SourceObservation, conduit_environment_runtime::environmental_source::Refusal> {
        if self.fail {
            return Err(conduit_environment_runtime::environmental_source::Refusal::Provider);
        }
        self.counter += 1;
        Ok(SourceObservation {
            observed_at_host_boot_ms: 100 + self.counter,
            outcome: SourceOutcome::Available(SourceSample {
                reading: EnvironmentalReading::centi_celsius_centi_pascal(2100, 10000000, None)
                    .unwrap(),
                source_counter: self.counter,
                reported_source_ms: Some(50 + self.counter),
            }),
        })
    }
}
pub(super) fn fixture() -> UsbSourceRoot<CpuSource> {
    let mut installed =
        conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
            host_id: "fixture/native-source-host".into(),
            boot_id: "fixture/native-source-boot".into(),
            offer_generation: OfferGeneration(1),
        });
    installed.set_initial_kernel_play_sequence(9).unwrap();
    UsbSourceRoot::attach(
        Rc::new(RefCell::new(Some(installed))),
        CpuSource {
            counter: 0,
            fail: false,
        },
        "cpu-fixture",
    )
    .unwrap()
}
fn ready<F: std::future::Future>(future: F) -> F::Output {
    use std::{
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };
    struct Noop;
    impl Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(Noop));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("CPU fixture unexpectedly suspended"),
    }
}
#[test]
fn checked_plot_runs_source_through_kernel_commit_and_retains_provider_across_real_plays() {
    let mut root = fixture();
    let advertisement = root.advertisement().unwrap();
    let first = ready(root.sample()).unwrap();
    let second = ready(root.sample()).unwrap();
    assert_eq!(
        first.scope.play(),
        &bind_active_play(
            first.scope.plan(),
            &advertisement.host_id,
            &advertisement.boot_id,
            9
        )
        .active_play_id
    );
    assert_eq!(
        second.scope.play(),
        &bind_active_play(
            second.scope.plan(),
            &advertisement.host_id,
            &advertisement.boot_id,
            10
        )
        .active_play_id
    );
    assert_eq!(first.scope.host(), &advertisement.host_id);
    assert_eq!(first.scope.boot(), &advertisement.boot_id);
    assert_eq!(first.scope.plan(), second.scope.plan());
    assert_ne!(first.scope.play(), second.scope.play());
    assert_eq!(first.session(), second.session());
    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);
    let contract = ObservationContract::prepare().unwrap();
    let a = contract.decode(&first.canonical).unwrap();
    let b = contract.decode(&second.canonical).unwrap();
    assert_eq!(a.observed_at_host_boot_ms, 101);
    assert_eq!(b.observed_at_host_boot_ms, 102);
    assert_eq!(root.provider.as_ref().unwrap().counter, 2);
}
#[test]
fn provider_failure_cannot_publish_or_reacquire_possession() {
    let mut root = fixture();
    root.provider.as_mut().unwrap().fail = true;
    assert!(matches!(ready(root.sample()), Err(Refusal::Acquisition)));
    assert!(root.provider.is_some());
    assert!(root.failed);
    assert!(matches!(ready(root.sample()), Err(Refusal::Acquisition)));
}
#[test]
fn foreign_boot_cannot_reserve_actual_native_capacity() {
    let root = fixture();
    let plan = root.plan().unwrap();
    let mut fragment = plan.fragments[0].clone();
    fragment.boot_id = "fixture/foreign".into();
    assert!(root
        .installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .reserve_native_fragment(&fragment)
        .is_err());
}

#[test]
fn received_fragment_cannot_issue_foreign_source_authority() {
    let root = fixture();
    let plan = root.plan().unwrap();
    let mut fragment = plan.fragments[0].clone();
    let issued = root
        .installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .issue_kernel_play(&fragment)
        .unwrap();
    let lease = root
        .installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .reserve_native_fragment(&fragment)
        .unwrap();
    fragment.boot_id = "fixture/foreign-boot".into();
    assert!(matches!(
        root.issue(&fragment, &issued, &lease),
        Err(Refusal::Authority)
    ));
    root.installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .release_native_fragment(lease)
        .unwrap();
    assert_eq!(root.provider.as_ref().unwrap().counter, 0);
}

#[test]
fn actual_planner_refusal_classification_is_preserved_before_capture() {
    let root = fixture();
    let mut host = root.advertisement().unwrap();
    host.resources.push(resource_offer(
        "fixture/secondary",
        "fixture/finite-unused",
        1,
    ));
    host.resources.sort_by(|a, b| b.pool_id.cmp(&a.pool_id));
    assert!(
        matches!(root.plan_on(&host),Err(Refusal::Planning(error)) if matches!(*error,conduit_planner::PlannerError::InvalidResourceContract(_)))
    );
    assert_eq!(root.provider.as_ref().unwrap().counter, 0);
}

#[test]
fn actual_source_fragment_reservation_refuses_duplicate_capacity_and_releases() {
    let mut root = fixture();
    let plan = root.plan().unwrap();
    let lease = root
        .installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .reserve_native_fragment(&plan.fragments[0])
        .unwrap();
    assert!(matches!(ready(root.sample()), Err(Refusal::Capacity)));
    assert_eq!(root.provider.as_ref().unwrap().counter, 0);
    root.installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .release_native_fragment(lease)
        .unwrap();
    assert!(ready(root.sample()).is_ok());
}

#[test]
fn actual_inventory_is_bounded_unique_and_cannot_change_under_a_live_lease() {
    let root = fixture();
    let installed = root.installed.clone();
    let fragment = root.plan().unwrap().fragments.remove(0);
    let lease = installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .reserve_native_fragment(&fragment)
        .unwrap();
    let prior = root.advertisement().unwrap();
    assert!(UsbSourceRoot::attach(
        installed.clone(),
        CpuSource {
            counter: 0,
            fail: false
        },
        "cpu-fixture"
    )
    .is_err());
    assert_eq!(root.advertisement().unwrap(), prior);
    installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .release_native_fragment(lease)
        .unwrap();
    let mut owners = vec![root];
    for _ in 1..4 {
        owners.push(
            UsbSourceRoot::attach(
                installed.clone(),
                CpuSource {
                    counter: 0,
                    fail: false,
                },
                "cpu-fixture",
            )
            .unwrap(),
        );
    }
    let inventory = owners[0].advertisement().unwrap();
    let source_ids = inventory
        .capabilities
        .iter()
        .filter(|offer| offer.kind_id.as_str() == HOST_CALL)
        .map(|offer| &offer.capability_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(source_ids.len(), 4);
    assert!(UsbSourceRoot::attach(
        installed,
        CpuSource {
            counter: 0,
            fail: false
        },
        "cpu-fixture"
    )
    .is_err());
    assert_eq!(owners[0].advertisement().unwrap(), inventory);
}

#[test]
fn actual_reservation_matches_only_its_exact_fragment_and_offer_generation() {
    let root = fixture();
    let fragment = root.plan().unwrap().fragments.remove(0);
    let lease = root
        .installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .reserve_native_fragment(&fragment)
        .unwrap();
    assert!(lease.matches_fragment(&fragment));
    let mut other = fragment.clone();
    other.fragment_id = "fixture/different-fragment".into();
    assert_eq!(other.plan_id, fragment.plan_id);
    assert!(!lease.matches_fragment(&other));
    let mut other = fragment.clone();
    other.offer_generation.0 += 1;
    assert!(!lease.matches_fragment(&other));
    // Core forbids two fragments for the same Host in one committed Plan.
    let mut other = fragment.clone();
    other.plan_fragments.push(FragmentCommitment {
        host_id: fragment.host_id.clone(),
        fragment_id: "fixture/second-fragment".into(),
    });
    other.plan_fragments.sort();
    assert!(!verify_plan_fragment(&other));
    root.installed
        .borrow_mut()
        .as_mut()
        .unwrap()
        .release_native_fragment(lease)
        .unwrap();
}
