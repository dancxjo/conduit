use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use conduit_core::{
    AdmissionUnit, BoundedDeliveryQueue, DeliveryAdmission, DeliveryContract,
    DeliveryPressurePolicy, DeliveryRefusal, EvolutionSemantics,
};

#[derive(Debug)]
struct ResourceClaim {
    item: u8,
    branch: &'static str,
    releases: Arc<AtomicUsize>,
}

impl ResourceClaim {
    fn new(item: u8, branch: &'static str, releases: &Arc<AtomicUsize>) -> Self {
        Self {
            item,
            branch,
            releases: Arc::clone(releases),
        }
    }
}

impl Drop for ResourceClaim {
    fn drop(&mut self) {
        self.releases.fetch_add(1, Ordering::SeqCst);
    }
}

fn contract(policy: DeliveryPressurePolicy) -> DeliveryContract {
    DeliveryContract::new(
        EvolutionSemantics::CurrentState,
        AdmissionUnit::Value,
        policy,
    )
}

#[test]
fn fanout_branches_keep_independent_pressure_meaning_and_resource_claims() {
    let first_releases = Arc::new(AtomicUsize::new(0));
    let second_releases = Arc::new(AtomicUsize::new(0));
    let mut preserve =
        BoundedDeliveryQueue::new(contract(DeliveryPressurePolicy::PreserveOrder), 1, 4).unwrap();
    let mut newest =
        BoundedDeliveryQueue::new(contract(DeliveryPressurePolicy::CoalesceLatest), 1, 4).unwrap();

    preserve
        .admit(ResourceClaim::new(1, "preserve", &first_releases))
        .unwrap();
    newest
        .admit(ResourceClaim::new(1, "newest", &first_releases))
        .unwrap();

    let refused = preserve
        .admit(ResourceClaim::new(2, "preserve", &second_releases))
        .unwrap_err();
    assert_eq!(refused.reason, DeliveryRefusal::Pressure);
    assert_eq!((refused.value.item, refused.value.branch), (2, "preserve"));

    let superseded = newest
        .admit(ResourceClaim::new(2, "newest", &second_releases))
        .unwrap();
    let DeliveryAdmission::CoalescedWholeValue { superseded } = superseded else {
        panic!("full newest branch must return its superseded owner");
    };
    assert_eq!((superseded.item, superseded.branch), (1, "newest"));

    drop(refused);
    drop(superseded);
    assert_eq!(first_releases.load(Ordering::SeqCst), 1);
    assert_eq!(second_releases.load(Ordering::SeqCst), 1);

    let preserved_first = preserve.pop_front().unwrap();
    let retained_second = newest.pop_front().unwrap();
    assert_eq!(
        (preserved_first.item, preserved_first.branch),
        (1, "preserve")
    );
    assert_eq!(
        (retained_second.item, retained_second.branch),
        (2, "newest")
    );
    assert_eq!(preserve.accounting().admitted, 1);
    assert_eq!(preserve.accounting().refused_pressure, 1);
    assert_eq!(preserve.accounting().coalesced, 0);
    assert_eq!(newest.accounting().admitted, 2);
    assert_eq!(newest.accounting().refused_pressure, 0);
    assert_eq!(newest.accounting().coalesced, 1);

    drop(preserved_first);
    drop(retained_second);
    assert_eq!(first_releases.load(Ordering::SeqCst), 2);
    assert_eq!(second_releases.load(Ordering::SeqCst), 2);
}
