#![cfg(feature = "show-custody")]
//! Finite inert Show custody, independent of HTTP and environmental meaning.
use conduit_face::{ShowCustodyRefusal as R, ShowMailbox};
#[test]
fn finite_lease_revision_and_single_publisher_are_preserved() {
    let mailbox = ShowMailbox::new();
    assert!(matches!(mailbox.claim(0, 10), Err(R::Budget)));
    let (mut publisher, _) = mailbox.claim(2, 10).unwrap();
    assert!(matches!(mailbox.claim(2, 10), Err(R::AlreadyClaimed)));
    assert_eq!(publisher.publish(7u8, 1, 10, 21), Err(R::Lifetime));
    publisher.publish(7, 1, 10, 20).unwrap();
    assert_eq!(publisher.publish(9, 1, 10, 20), Err(R::Stale));
    assert_eq!(mailbox.with_current(9, |value| *value), Ok(None));
    assert_eq!(mailbox.with_current(10, |value| *value), Ok(Some(7)));
    assert_eq!(mailbox.with_current(20, |value| *value), Ok(None));
    publisher.publish(9, 2, 11, 21).unwrap();
    assert_eq!(publisher.publish(10, 3, 12, 22), Err(R::Exhausted));
    assert_eq!(mailbox.with_current(12, |value| *value), Ok(Some(9)));
    drop(publisher);
    assert_eq!(mailbox.with_current(12, |value| *value), Ok(None));
    assert!(matches!(mailbox.claim(2, 10), Err(R::AlreadyClaimed)));
}
#[test]
fn reentrant_write_refuses_and_nested_withdrawal_suppresses_all_reads() {
    let mailbox = ShowMailbox::new();
    let (mut publisher, control) = mailbox.claim(2, 10).unwrap();
    publisher.publish(7u8, 1, 10, 20).unwrap();
    assert_eq!(
        mailbox.with_current(10, |_| {
            assert_eq!(publisher.publish(9, 2, 10, 20), Err(R::Busy));
            assert_eq!(
                mailbox.with_current(10, |_| {
                    control.withdraw();
                    control.withdraw();
                    42
                }),
                Ok(None)
            );
            99
        }),
        Ok(None)
    );
    assert_eq!(mailbox.with_current(10, |value| *value), Ok(None));
    assert_eq!(publisher.publish(9, 2, 10, 20), Err(R::Withdrawn));
}
#[test]
fn publisher_drop_during_a_read_fences_the_result_without_reclaiming_the_slot() {
    let mailbox = ShowMailbox::new();
    let (mut publisher, _) = mailbox.claim(1, 10).unwrap();
    publisher.publish(7u8, 1, 10, 20).unwrap();
    assert_eq!(mailbox.with_current(10, |_| drop(publisher)), Ok(None));
    assert!(matches!(mailbox.claim(1, 10), Err(R::AlreadyClaimed)));
}
