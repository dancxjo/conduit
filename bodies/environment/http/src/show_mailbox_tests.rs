use super::*;
use crate::{tests::allocation_probe, HttpShowTime};
use conduit_core::Quantity;
use conduit_environment_face::{PreparedObservationFace, SensorStatus};

fn owner() -> PreparedObservationFace {
    let mut face = PreparedObservationFace::prepare_observed(crate::tests::basis(), 3, 30).unwrap();
    face.publish(
        Some((
            Quantity::parse_plot_literal("19°C").unwrap(),
            Quantity::parse_plot_literal("55%").unwrap(),
        )),
        SensorStatus::Available,
        1000,
    )
    .unwrap();
    face
}
#[test]
fn finite_show_custody_preserves_explicit_lease_and_one_publisher() {
    let mailbox = HttpShowMailbox::new();
    assert!(matches!(mailbox.claim(0, 30), Err(HttpShowRefusal::Budget)));
    let (mut publisher, _) = mailbox.claim(1, 30).unwrap();
    assert!(matches!(
        mailbox.claim(1, 30),
        Err(HttpShowRefusal::AlreadyClaimed)
    ));
    assert!(mailbox.response(true, 1000, true).unwrap().is_none());
    let owner = owner();
    let identity = owner.current().identity.clone();
    let ((), memory) = allocation_probe::observe(|| {
        let make = || PreparedHttpShow::prepare(owner.current()).unwrap();
        assert_eq!(
            make().time_coordinate(),
            HttpShowTime::ObservedAtBootMs(1000)
        );
        assert_eq!(
            publisher.publish(make(), 1000, 1031),
            Err(HttpShowRefusal::Lifetime)
        );
        assert_eq!(
            publisher.publish(make(), 1001, 1000),
            Err(HttpShowRefusal::Lifetime)
        );
        publisher.publish(make(), 1000, 1030).unwrap();
        assert_eq!(
            publisher.publish(make(), 1000, 1030),
            Err(HttpShowRefusal::Exhausted)
        );
        assert!(mailbox.response(true, 999, true).unwrap().is_none());
        let response = mailbox.response(true, 1029, true).unwrap().unwrap();
        assert!(response.contains(identity.as_str()));
        assert!(response.contains("\"observed_at_boot_ms\":1000"));
        assert!(response.contains("\"sampled_at_boot_ms\":null"));
        assert!(mailbox.response(true, 1030, true).unwrap().is_none());
        assert!(mailbox.response(true, u64::MAX, true).unwrap().is_none());
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    drop(publisher);
    assert!(mailbox.response(true, 1000, true).unwrap().is_none());
    assert!(matches!(
        mailbox.claim(1, 30),
        Err(HttpShowRefusal::AlreadyClaimed)
    ));
}
#[test]
fn withdrawal_fences_late_publication_and_stale_attempt_preserves_current_show() {
    let mailbox = HttpShowMailbox::new();
    let (mut publisher, control) = mailbox.claim(2, 30).unwrap();
    let mut owner = owner();
    let ((), memory) = allocation_probe::observe(|| {
        publisher
            .publish(
                PreparedHttpShow::prepare(owner.current()).unwrap(),
                1000,
                1030,
            )
            .unwrap();
        let before = mailbox.response(true, 1001, true).unwrap().unwrap();
        assert_eq!(
            publisher.publish(
                PreparedHttpShow::prepare(owner.current()).unwrap(),
                1000,
                1030
            ),
            Err(HttpShowRefusal::Stale)
        );
        assert_eq!(mailbox.response(true, 1001, true).unwrap().unwrap(), before);
        owner
            .publish(
                None,
                SensorStatus::Unavailable(conduit_environment_face::SensorFailure::Checksum),
                1001,
            )
            .unwrap();
        publisher
            .publish(
                PreparedHttpShow::prepare(owner.current()).unwrap(),
                1001,
                1030,
            )
            .unwrap();
        let changed = mailbox.response(true, 1002, true).unwrap().unwrap();
        assert!(changed.contains("\"status\":\"checksum\""));
        assert!(changed.contains("\"current\":false"));
        control.withdraw();
        control.withdraw();
        assert!(mailbox.response(true, 1002, true).unwrap().is_none());
        assert_eq!(
            publisher.publish(
                PreparedHttpShow::prepare(owner.current()).unwrap(),
                1002,
                1030
            ),
            Err(HttpShowRefusal::Withdrawn)
        );
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    std::println!(
        "http_show_mailbox_cpu inline_bytes={} pointer_bits={} heap_bytes=0",
        core::mem::size_of::<HttpShowMailbox>(),
        usize::BITS
    );
}

#[test]
fn return_replaces_only_with_fresh_budget_and_fences_stale_operations() {
    let mailbox = HttpShowMailbox::new();
    let (mut publisher, mut control) = mailbox.claim(1, 30).unwrap();
    for generation in 0..4 {
        let mut stale = HttpShowPublisher {
            mailbox: &mailbox,
            generation,
            remaining: 1,
            maximum_lifetime_ms: 30,
        };
        let stale_control = HttpShowControl {
            mailbox: &mailbox,
            generation,
        };
        let returned = mailbox.reunite(publisher, control).ok().unwrap();
        assert!(mailbox.response(true, 1000, true).unwrap().is_none());
        let (returned, error) = returned.replace(0, 30).err().unwrap();
        assert_eq!(error, HttpShowRefusal::Budget);
        let next = returned.replace(1, 30).ok().unwrap();
        publisher = next.0;
        control = next.1;
        let face = owner();
        publisher
            .publish(
                PreparedHttpShow::prepare(face.current()).unwrap(),
                1000,
                1030,
            )
            .unwrap();
        assert_eq!(
            stale.publish(
                PreparedHttpShow::prepare(face.current()).unwrap(),
                1000,
                1030
            ),
            Err(HttpShowRefusal::Scope)
        );
        stale_control.withdraw();
        stale.withdraw();
        drop(stale);
        assert!(mailbox.response(true, 1001, true).unwrap().is_some());
        assert_eq!(
            publisher.publish(
                PreparedHttpShow::prepare(face.current()).unwrap(),
                1000,
                1030
            ),
            Err(HttpShowRefusal::Exhausted)
        );
    }
}
#[test]
fn return_mismatch_busy_and_overflow_preserve_ownership_and_generation() {
    let mailbox = HttpShowMailbox::new();
    let other = HttpShowMailbox::new();
    let (publisher, control) = mailbox.claim(1, 30).unwrap();
    let (other_publisher, other_control) = other.claim(1, 30).unwrap();
    let (publisher, other_control, error) =
        mailbox.reunite(publisher, other_control).err().unwrap();
    assert_eq!(error, HttpShowRefusal::Scope);
    let (publisher, control) = critical_section::with(|cs| {
        let _borrow = mailbox.state.borrow(cs).borrow();
        let (publisher, control, error) = mailbox.reunite(publisher, control).err().unwrap();
        assert_eq!(error, HttpShowRefusal::Busy);
        (publisher, control)
    });
    let returned = mailbox.reunite(publisher, control).ok().unwrap();
    let returned = critical_section::with(|cs| {
        let _borrow = mailbox.state.borrow(cs).borrow();
        let (returned, error) = returned.replace(1, 30).err().unwrap();
        assert_eq!(error, HttpShowRefusal::Busy);
        returned
    });
    let (publisher, control) = returned.replace(1, 30).ok().unwrap();
    drop((publisher, control));
    critical_section::with(|cs| mailbox.state.borrow(cs).borrow_mut().generation = u64::MAX);
    let publisher = HttpShowPublisher {
        mailbox: &mailbox,
        generation: u64::MAX,
        remaining: 1,
        maximum_lifetime_ms: 30,
    };
    let control = HttpShowControl {
        mailbox: &mailbox,
        generation: u64::MAX,
    };
    let (_, _, error) = mailbox.reunite(publisher, control).err().unwrap();
    assert_eq!(error, HttpShowRefusal::GenerationExhausted);
    drop((other_publisher, other_control));
}

#[test]
fn publisher_explicit_withdraw_retains_custody_and_fences_late_material() {
    let mailbox = HttpShowMailbox::new();
    let (publisher, control) = mailbox.claim(2, 100).unwrap();
    publisher.withdraw();
    assert!(mailbox.response(true, 0, true).unwrap().is_none());
    let returned = mailbox.reunite(publisher, control).ok().unwrap();
    let (publisher, _control) = returned.replace(3, 200).ok().unwrap();
    publisher.withdraw();
    assert!(mailbox.response(true, 0, true).unwrap().is_none());
}
