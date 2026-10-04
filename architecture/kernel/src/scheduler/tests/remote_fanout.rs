use super::*;
use crate::SignError;
const TARGETS: [(RemoteEndpointId, CordId); 2] = [
    (RemoteEndpointId(0), CordId(0)),
    (RemoteEndpointId(1), CordId(1)),
];
type FanoutScheduler =
    FixedScheduler<Driver, FixedValueStore<2, 4>, FixedSignLog<16>, 2, 2, PORTS, 2, 2, 1>;
fn scheduler(remote_items: u16) -> FanoutScheduler {
    let endpoints = [RemoteEndpointId(0), RemoteEndpointId(1)];
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes.seal().unwrap();
    let signs = FixedSignLog::<16>::new_with_remote_storage(
        (16 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        remote_items,
        crate::remote_sign_storage_bytes(remote_items).unwrap(),
    )
    .unwrap();
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 4,
        pressure_policy: Default::default(),
    };
    FixedScheduler::<_, _, _, 2, 2, PORTS, 2, 2, 1>::new(
        [node([Some(CordId(0)), None]), node([Some(CordId(1)), None])],
        [
            CordSpec::remote_ingress(CordId(0), endpoints[0], (NodeId(0), PortId(0)), capacity(0)),
            CordSpec::remote_ingress(CordId(1), endpoints[1], (NodeId(1), PortId(0)), capacity(1)),
        ],
        routes,
        [
            Driver::Sink {
                seen: [None; 4],
                len: 0,
                stall: false,
            },
            Driver::BlockedSink { cancelled: false },
        ],
        FixedValueStore::<2, 4>::new(8).unwrap(),
        signs,
    )
    .unwrap()
}
#[test]
fn pressured_remote_ingress_branch_refuses_atomic_payload_fanout_without_partial_delivery() {
    let targets = TARGETS;
    let mut scheduler = scheduler(16);

    assert_eq!(
        scheduler.admit_remote_input_fanout(&targets, 0, b"seed"),
        Ok(RemoteIngressOutcome::Accepted { sequence: 0 })
    );
    assert_eq!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress { node: NodeId(0) })
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert_eq!(scheduler.cord_usage(CordId(1)).unwrap(), (1, 4));

    assert_eq!(
        scheduler.admit_remote_input_fanout(&targets, 1, b"next"),
        Ok(RemoteIngressOutcome::Full { sequence: 1 })
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert_eq!(scheduler.cord_usage(CordId(1)).unwrap(), (1, 4));
    assert_eq!(scheduler.values().used_items(), 1);
}

#[test]
fn invalid_or_pressured_closure_leaves_every_ingress_branch_open() {
    let mut invalid = scheduler(16);
    assert_eq!(
        invalid.close_remote_input_fanout(&[TARGETS[0], (RemoteEndpointId(99), CordId(1)),]),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
    assert!(!invalid.cords[0].producer_closed);
    assert!(!invalid.cords[1].producer_closed);
    let mut pressured = scheduler(1);
    assert_eq!(
        pressured.close_remote_input_fanout(&TARGETS),
        Err(SchedulerError::Sign(SignError::RemoteItemCapacityExceeded))
    );
    assert!(!pressured.cords[0].producer_closed);
    assert!(!pressured.cords[1].producer_closed);
    invalid.close_remote_input_fanout(&TARGETS).unwrap();
    assert!(invalid.cords[0].producer_closed && invalid.cords[1].producer_closed);
    let signs = invalid.signs().events().count();
    invalid.close_remote_input_fanout(&TARGETS).unwrap();
    assert_eq!(invalid.signs().events().count(), signs);
}
#[test]
fn abnormal_closure_preserves_one_exact_terminal_on_every_branch() {
    let mut scheduler = scheduler(16);
    let terminal = CanonicalValue::new(b"lost").unwrap();
    scheduler
        .close_remote_input_fanout_abnormal(&TARGETS, terminal)
        .unwrap();
    assert_eq!(scheduler.cords[0].abnormal_terminal, Some(terminal));
    assert_eq!(scheduler.cords[1].abnormal_terminal, Some(terminal));
    assert_eq!(
        scheduler.close_remote_input_fanout(&TARGETS),
        Err(SchedulerError::RemoteDeliveryRejected)
    );
}
