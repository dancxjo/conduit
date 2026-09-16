use conduit_core::{
    AdmissionUnit, BoundedDeliveryQueue, DeliveryContract, DeliveryPressurePolicy,
    DeliveryQueueState, DeliveryRefusal, DeliveryTerminalRefusal, EvolutionSemantics,
    RejectedDelivery,
};

const ORDERED: DeliveryContract = DeliveryContract::new(
    EvolutionSemantics::Occurrence,
    AdmissionUnit::Value,
    DeliveryPressurePolicy::PreserveOrder,
);

#[test]
fn closure_preserves_owed_delivery_and_refuses_new_ownership() {
    let mut queue = BoundedDeliveryQueue::new(ORDERED, 2, 3).unwrap();
    queue.admit(1).unwrap();
    queue.admit(2).unwrap();
    queue.close().unwrap();

    assert_eq!(queue.state(), DeliveryQueueState::Closed);
    assert_eq!(
        queue.admit(3),
        Err(RejectedDelivery {
            reason: DeliveryRefusal::Closed,
            value: 3,
        })
    );
    assert_eq!(queue.pop_front(), Some(1));
    assert_eq!(queue.pop_front(), Some(2));
    assert_eq!(queue.accounting().delivered, 2);
    assert_eq!(queue.close(), Err(DeliveryTerminalRefusal::AlreadyClosed));
}

#[test]
fn cancellation_is_terminal_and_distinct_from_closure_and_pressure() {
    let mut queue = BoundedDeliveryQueue::new(ORDERED, 1, 3).unwrap();
    queue.admit(1).unwrap();
    assert_eq!(queue.cancel().unwrap().pop_front(), Some(1));
    assert_eq!(queue.accounting().cancelled, 1);
    assert_eq!(
        queue.admit(2),
        Err(RejectedDelivery {
            reason: DeliveryRefusal::Cancelled,
            value: 2,
        })
    );
    assert_eq!(
        queue.close(),
        Err(DeliveryTerminalRefusal::AlreadyCancelled)
    );
    assert_eq!(
        queue.cancel(),
        Err(DeliveryTerminalRefusal::AlreadyCancelled)
    );
}
