use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver, RemoteIngressOutcome,
    SchedulerStatus,
};
use conduit_kernel::state_delay::{operation::StateOperation, StateDelay};
use conduit_kernel::{
    CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId,
    RemoteEndpointId, RouteRange, RouteTarget, ValueStorage,
};

type Play = FixedScheduler<
    OperationDriver<StateOperation<1>, 1>,
    FixedValueStore<4, 1>,
    FixedSignLog<512>,
    1,
    2,
    1,
    2,
    1,
    1,
>;

fn play() -> Play {
    with_state(StateDelay::externally_continued(0, 1, &[0]).unwrap())
}

fn with_state(state: StateDelay<1>) -> Play {
    let mut routes = FixedRoutes::<1, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(1),
                sink: CordEndpoint::Remote(RemoteEndpointId(1)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let signs = FixedSignLog::<512>::new_with_remote_storage(
        (512 * core::mem::size_of::<KernelEvent>()) as u32,
        512,
        conduit_kernel::remote_sign_storage_bytes(512).unwrap(),
    )
    .unwrap();
    Play::new(
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_work: 4,
        }],
        [
            CordSpec::remote_ingress(
                CordId(0),
                RemoteEndpointId(0),
                (NodeId(0), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            ),
            CordSpec::remote_egress(
                CordId(1),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(1),
                CordCapacity {
                    slot_start: 1,
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            ),
        ],
        routes,
        [
            OperationDriver::new(StateOperation::new(state, PortId(0), PortId(0)).unwrap())
                .unwrap(),
        ],
        FixedValueStore::<4, 1>::new(4).unwrap(),
        signs,
    )
    .unwrap()
}

fn idle(play: &mut Play) {
    for _ in 0..8 {
        match play.step().unwrap() {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => return,
            other => panic!("waiting is not terminal: {other:?}"),
        }
    }
    panic!("bounded test failed to reach input wait");
}

fn deliver(play: &mut Play, sequence: u64, expected: u8) {
    let offer = play
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .unwrap();
    assert_eq!(offer.sequence, sequence);
    assert_eq!(play.host_value(offer.value).unwrap(), &[expected]);
    play.remote_egress_accept(RemoteEndpointId(1), CordId(1), sequence)
        .unwrap();
    play.remote_egress_delivered(RemoteEndpointId(1), CordId(1), sequence)
        .unwrap();
}

#[test]
fn input_wait_continued_operation_and_explicit_closure_are_distinct() {
    let mut play = play();
    idle(&mut play);
    deliver(&mut play, 0, 0);
    idle(&mut play);
    assert_eq!(play.drivers()[0].operation().state().generation(), 0);
    for sequence in 0..16 {
        assert!(matches!(
            play.admit_remote_input(
                RemoteEndpointId(0),
                CordId(0),
                sequence,
                &[(sequence % 2) as u8]
            )
            .unwrap(),
            RemoteIngressOutcome::Accepted { .. }
        ));
        idle(&mut play);
        deliver(&mut play, sequence + 1, (sequence % 2) as u8);
        idle(&mut play);
        assert_eq!(
            play.drivers()[0].operation().state().generation(),
            sequence + 1
        );
        assert_eq!(play.values().used_items(), 0);
    }
    play.close_remote_input(RemoteEndpointId(0), CordId(0))
        .unwrap();
    let mut completed = false;
    for _ in 0..8 {
        if play.step().unwrap() == SchedulerStatus::Complete {
            completed = true;
            break;
        }
    }
    assert!(
        completed,
        "only explicit input closure completes this specimen"
    );
    assert!(
        play.try_retire().is_ok(),
        "completed drained execution retires"
    );
}

#[test]
fn output_pressure_does_not_drop_the_queued_next_state() {
    let mut play = play();
    idle(&mut play); // retain the initial output in its capacity-one Cord
    play.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[1])
        .unwrap();
    assert_eq!(
        play.admit_remote_input(RemoteEndpointId(0), CordId(0), 1, &[0])
            .unwrap(),
        RemoteIngressOutcome::Full { sequence: 1 }
    );
    idle(&mut play);
    deliver(&mut play, 0, 0);
    idle(&mut play);
    deliver(&mut play, 1, 1);
    idle(&mut play);
    assert_eq!(play.drivers()[0].operation().state().current(), &[1]);
    assert_eq!(play.drivers()[0].operation().state().generation(), 1);
    assert_eq!(play.values().used_items(), 0);
    play.cancel().unwrap();
    assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
}

#[test]
fn a_larger_transition_allowance_executes_the_same_input_without_hiding_exhaustion() {
    for allowance in [1, 2] {
        let mut play = with_state(StateDelay::new(0, 1, allowance, &[0]).unwrap());
        idle(&mut play);
        deliver(&mut play, 0, 0);
        for sequence in 0..2 {
            play.admit_remote_input(RemoteEndpointId(0), CordId(0), sequence, &[1])
                .unwrap();
            if sequence == allowance {
                assert_eq!(
                    play.step(),
                    Err(conduit_kernel::scheduler::SchedulerError::OperationFailed(
                        2
                    ))
                );
                assert_eq!(play.drivers()[0].operation().state().generation(), 1);
                assert_eq!(play.drivers()[0].operation().state().current(), &[1]);
            } else {
                idle(&mut play);
                deliver(&mut play, sequence + 1, 1);
                idle(&mut play);
                assert_eq!(
                    play.drivers()[0].operation().state().generation(),
                    sequence + 1
                );
            }
        }
    }
}

#[test]
fn derived_emission_capacity_is_checked_before_operation_start() {
    let state = StateDelay::<65>::externally_continued(0, 65, &[0]).unwrap();
    assert!(matches!(
        StateOperation::new(state, PortId(0), PortId(0)),
        Err(conduit_kernel::state_delay::StateError::InvalidBounds)
    ));
}

#[test]
fn retirement_refuses_input_wait_and_preserves_state_after_explicit_cancellation() {
    let mut play = play();
    idle(&mut play);
    deliver(&mut play, 0, 0);
    idle(&mut play);
    let mut play = match play.try_retire() {
        Ok(_) => panic!("input wait is not retirement"),
        Err(play) => play,
    };
    play.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[1])
        .unwrap();
    idle(&mut play);
    deliver(&mut play, 1, 1);
    idle(&mut play);
    play.cancel().unwrap();
    let retired = match play.try_retire() {
        Ok(retired) => retired,
        Err(_) => panic!("cancelled drained execution should retire"),
    };
    assert!(retired.cancelled);
    assert_eq!(retired.values.used_items(), 0);
    let [driver] = retired.drivers;
    let state = driver.into_operation().into_state();
    assert_eq!(state.generation(), 1);
    assert_eq!(state.current(), &[1]);
    let (moved, evidence) = match state.try_transfer::<2>(7, 2) {
        Ok(result) => result,
        Err(_) => panic!("retained State fits the larger storage"),
    };
    assert_eq!(moved.generation(), 1);
    assert_eq!(moved.current(), &[1]);
    assert_eq!(evidence.destination_slot, 7);
}

#[test]
fn cancelling_a_pressured_update_retires_only_the_last_committed_state() {
    let mut play = play();
    idle(&mut play); // the initial output occupies the sole output slot
    play.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[1])
        .unwrap();
    idle(&mut play); // the next output cannot commit yet
    play.cancel().unwrap();
    let retired = match play.try_retire() {
        Ok(retired) => retired,
        Err(_) => panic!("cancelled execution should retire"),
    };
    let [driver] = retired.drivers;
    let state = driver.into_operation().into_state();
    assert_eq!(
        state.current(),
        &[0],
        "a pressured candidate was never published"
    );
    assert_eq!(state.generation(), 0);
}
