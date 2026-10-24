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
