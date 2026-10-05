use super::*;
use conduit_kernel::{BoundedValueRef, NodeId, RequestId, ValueRef};

fn interest(request: u32) -> KernelInterest {
    KernelInterest {
        node: NodeId(0),
        request: RequestId(request),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 4,
            },
            4,
        )
        .unwrap(),
    }
}

#[test]
fn inherited_irq_cannot_complete_a_later_admitted_timer() {
    let mailbox = IrqMailbox::new();
    let mut timer = TimerState::new();
    mailbox.publish(); // Firmware IRQ delivered before any hardware arm.
    assert!(mailbox.has_fact());
    let token = timer.arm(interest(1)).unwrap();
    assert_eq!(mailbox.start(token.generation), Err(BaseError::StaleWake));
    assert_eq!(
        timer.wake(mailbox.pop().unwrap().unwrap()),
        Err(BaseError::StaleWake)
    );
    mailbox.start(token.generation).unwrap();
    mailbox.publish();
    assert_eq!(timer.wake(mailbox.pop().unwrap().unwrap()), Ok(interest(1)));
}

#[test]
fn stale_cancel_does_not_retire_the_current_timer() {
    let mut timer = TimerState::new();
    let old = timer.arm(interest(1)).unwrap();
    timer.cancel(old).unwrap();
    let current = timer.arm(interest(2)).unwrap();
    assert_eq!(timer.cancel(old), Err(BaseError::StaleWake));
    assert_eq!(timer.arm(interest(3)), Err(BaseError::SlotFull));
    assert_eq!(timer.wake(current.generation), Ok(interest(2)));
    assert_eq!(
        timer.wake(current.generation),
        Err(BaseError::DuplicateWake)
    );
}

#[test]
fn cancelled_irq_retains_its_identity_across_rearm() {
    let mailbox = IrqMailbox::new();
    let mut timer = TimerState::new();
    let old = timer.arm(interest(1)).unwrap();
    mailbox.start(old.generation).unwrap();
    mailbox.publish();
    timer.cancel(old).unwrap();
    mailbox.retire();
    let current = timer.arm(interest(2)).unwrap();
    assert_eq!(mailbox.start(current.generation), Err(BaseError::StaleWake));
    assert_eq!(
        timer.wake(mailbox.pop().unwrap().unwrap()),
        Err(BaseError::StaleWake)
    );
    mailbox.start(current.generation).unwrap();
    mailbox.publish();
    assert_eq!(timer.wake(mailbox.pop().unwrap().unwrap()), Ok(interest(2)));
}

#[test]
fn cancellation_before_idle_and_late_delivery_remain_terminal() {
    let mailbox = IrqMailbox::new();
    let mut timer = TimerState::new();
    let token = timer.arm(interest(1)).unwrap();
    assert_eq!(timer.cancel(token), Ok(interest(1)));
    assert_eq!(timer.cancel(token), Err(BaseError::TimerCancelled));
    assert_eq!(timer.wake(token.generation), Err(BaseError::TimerCancelled));
    mailbox.publish(); // A late IRQ with no physical arm has no token.
    assert_eq!(
        timer.wake(mailbox.pop().unwrap().unwrap()),
        Err(BaseError::StaleWake)
    );
}

#[test]
fn overflow_preserves_first_fact_and_refuses_new_hardware_arm() {
    let mailbox = IrqMailbox::new();
    mailbox.start(7).unwrap();
    assert_eq!(mailbox.start(8), Err(BaseError::SlotFull));
    mailbox.publish();
    mailbox.publish();
    mailbox.retire();
    assert_eq!(mailbox.start(8), Err(BaseError::RingFull));
    assert_eq!(mailbox.pop(), Err(BaseError::RingFull));
    assert_eq!(mailbox.pop(), Ok(Some(7)));
    assert_eq!(mailbox.pop(), Ok(None));
    assert!(!mailbox.has_fact());
    mailbox.start(8).unwrap();
}
