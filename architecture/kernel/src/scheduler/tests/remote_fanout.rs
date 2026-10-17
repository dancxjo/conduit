use super::*;
use crate::SignError;
const TARGETS: [(RemoteEndpointId, CordId); 2] = [
    (RemoteEndpointId(0), CordId(0)),
    (RemoteEndpointId(1), CordId(1)),
];
type FanoutScheduler =
    FixedScheduler<Driver, FixedValueStore<2, 4>, FixedSignLog<16>, 2, 2, PORTS, 2, 2, 1>;
fn scheduler(remote_items: u16) -> FanoutScheduler {
    scheduler_with_policy(remote_items, AssignedPressurePolicy::default())
}
fn scheduler_with_policy(remote_items: u16, policy: AssignedPressurePolicy) -> FanoutScheduler {
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
        pressure_policy: policy,
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

#[test]
fn exhausted_sign_sequence_refuses_fanout_before_payload_or_closure_mutation() {
    for close in [false, true] {
        let mut scheduler = scheduler(16);
        scheduler.signs.next_sequence = u32::MAX - 1;
        let before = scheduler.signs.entries;
        let result = if close {
            scheduler.close_remote_input_fanout(&TARGETS)
        } else {
            scheduler
                .admit_remote_input_fanout(&TARGETS, 0, b"seed")
                .map(|_| ())
        };
        assert_eq!(
            result,
            Err(SchedulerError::Sign(SignError::SequenceOverflow))
        );
        assert_eq!(scheduler.signs.entries, before);
        assert_eq!(scheduler.signs.next_sequence, u32::MAX - 1);
        assert_eq!(scheduler.signs.remote_len, 0);
        assert_eq!(scheduler.values().used_items(), 0);
        for (_, cord) in TARGETS {
            assert_eq!(scheduler.cord_usage(cord).unwrap(), (0, 0));
            let state = &scheduler.cords[usize::from(cord.0)];
            assert_eq!(state.next_remote_sequence, 0);
            assert!(!state.producer_closed);
        }
    }
}

#[test]
fn empty_remote_payload_obeys_item_pressure_without_allocating_or_partial_fanout() {
    for fanout in [false, true] {
        let mut scheduler = scheduler(16);
        scheduler
            .admit_remote_input_fanout(&TARGETS, 0, b"seed")
            .unwrap();
        scheduler.step().unwrap();
        let signs = scheduler.signs.entries;
        let values = scheduler
            .values
            .slots
            .map(|slot| (slot.generation, slot.references, slot.len, slot.bytes));
        let outcome = if fanout {
            scheduler.admit_remote_input_fanout(&TARGETS, 1, b"")
        } else {
            scheduler.admit_remote_input(TARGETS[1].0, TARGETS[1].1, 1, b"")
        };
        assert_eq!(outcome, Ok(RemoteIngressOutcome::Full { sequence: 1 }));
        assert_eq!(scheduler.signs.entries, signs);
        assert_eq!(
            scheduler.values.slots.map(|slot| (
                slot.generation,
                slot.references,
                slot.len,
                slot.bytes
            )),
            values
        );
        assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
        assert_eq!(scheduler.cord_usage(CordId(1)).unwrap(), (1, 4));
        assert_eq!(scheduler.cords[0].next_remote_sequence, 1);
        assert_eq!(scheduler.cords[1].next_remote_sequence, 1);
    }
}

#[test]
fn empty_remote_payload_is_admitted_with_item_room_and_pressured_when_slots_fill() {
    let mut scheduler = scheduler(16);
    assert_eq!(
        scheduler.admit_remote_input_fanout(&TARGETS, 0, b""),
        Ok(RemoteIngressOutcome::Accepted { sequence: 0 })
    );
    for (_, cord) in TARGETS {
        assert_eq!(scheduler.cord_usage(cord).unwrap(), (1, 0));
    }
    assert_eq!(scheduler.values().used_items(), 1);
    assert_eq!(scheduler.values().used_bytes(), 0);
    assert_eq!(
        scheduler.admit_remote_input_fanout(&TARGETS, 1, b""),
        Ok(RemoteIngressOutcome::Full { sequence: 1 })
    );
}

#[test]
fn empty_remote_fanout_preserves_admitted_coalescing_policy() {
    let mut scheduler = scheduler_with_policy(16, AssignedPressurePolicy::CoalesceLatest);
    scheduler
        .admit_remote_input_fanout(&TARGETS, 0, b"seed")
        .unwrap();
    assert_eq!(
        scheduler.admit_remote_input_fanout(&TARGETS, 1, b""),
        Ok(RemoteIngressOutcome::Accepted { sequence: 1 })
    );
    for (_, cord) in TARGETS {
        assert_eq!(scheduler.cord_usage(cord).unwrap(), (1, 0));
    }
    assert_eq!(scheduler.values().used_items(), 1);
    assert_eq!(scheduler.values().used_bytes(), 0);
}

#[test]
fn remote_fanout_pressure_cannot_hide_invalid_or_stale_sibling_targets() {
    for scenario in 0..3 {
        for reverse in [false, true] {
            let mut scheduler = scheduler(16);
            scheduler
                .admit_remote_input_fanout(&TARGETS, 0, b"seed")
                .unwrap();
            scheduler.step().unwrap();
            let other = match scenario {
                0 => TARGETS[1], // Duplicate of the pressured branch.
                1 => (RemoteEndpointId(99), CordId(0)),
                _ => {
                    scheduler.cords[0].next_remote_sequence = 0;
                    TARGETS[0]
                }
            };
            let targets = if reverse {
                [other, TARGETS[1]]
            } else {
                [TARGETS[1], other]
            };
            let signs = scheduler.signs.entries;
            let values = scheduler
                .values
                .slots
                .map(|slot| (slot.generation, slot.references, slot.len, slot.bytes));
            let expected = if scenario == 2 {
                SchedulerError::RemoteSequenceRejected
            } else {
                SchedulerError::InvalidRemoteCordAccess
            };
            assert_eq!(
                scheduler.admit_remote_input_fanout(&targets, 1, b"next"),
                Err(expected)
            );
            assert_eq!(scheduler.signs.entries, signs);
            assert_eq!(
                scheduler.values.slots.map(|slot| (
                    slot.generation,
                    slot.references,
                    slot.len,
                    slot.bytes
                )),
                values
            );
            assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
            assert_eq!(scheduler.cord_usage(CordId(1)).unwrap(), (1, 4));
        }
    }
}

#[test]
fn remote_fanout_rejects_more_targets_than_admitted_cords_even_under_pressure() {
    let mut scheduler = scheduler(16);
    scheduler
        .admit_remote_input_fanout(&TARGETS, 0, b"seed")
        .unwrap();
    assert_eq!(
        scheduler.admit_remote_input_fanout(&[TARGETS[0], TARGETS[1], TARGETS[0]], 1, b"next"),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
}
