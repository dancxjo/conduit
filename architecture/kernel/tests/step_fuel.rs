use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, StepBack, StepInputBytes,
    StepIo, StepOutcome,
};
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, KernelEventKind, NodeId,
    PortId,
};

#[derive(Clone, Copy)]
enum Back {
    Cooperative { yields_remaining: u8 },
    Cancellable { cancelled: bool },
    ExceedsGrant,
}

impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Cooperative { yields_remaining } if *yields_remaining > 0 => {
                while io.remaining_fuel() > 0 {
                    io.consume_fuel(1).unwrap();
                }
                *yields_remaining -= 1;
                StepOutcome::Yield
            }
            Self::Cooperative { .. } => StepOutcome::Complete,
            Self::Cancellable { .. } => {
                io.exhaust_fuel();
                StepOutcome::Yield
            }
            Self::ExceedsGrant => {
                let result = io.consume_fuel(io.remaining_fuel().saturating_add(1));
                assert_eq!(result, Err(SchedulerError::StepFuelExceeded));
                StepOutcome::Await
            }
        }
    }

    fn cancel(&mut self) {
        if let Self::Cancellable { cancelled } = self {
            *cancelled = true;
        }
    }
}

type Play = FixedScheduler<Back, FixedValueStore<1, 1>, FixedSignLog<32>, 2, 1, 1, 1, 1, 1>;

fn play(active_nodes: usize, backs: [Back; 2]) -> Play {
    let mut routes = FixedRoutes::<1, 1>::new(1);
    routes.seal().unwrap();
    Play::new_with_active_counts(
        active_nodes,
        0,
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 3,
            },
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 3,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 1,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        backs,
        FixedValueStore::new(1).unwrap(),
        FixedSignLog::new((32 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
    )
    .unwrap()
}

#[test]
fn cooperative_continuations_consume_each_grant_and_remain_fair() {
    let mut play = play(
        2,
        [
            Back::Cooperative {
                yields_remaining: 3,
            },
            Back::Cooperative {
                yields_remaining: 3,
            },
        ],
    );

    play.run(8).unwrap();

    let grants = play
        .signs()
        .events()
        .filter(|event| event.kind == KernelEventKind::StepFuelGranted)
        .map(|event| event.node)
        .collect::<Vec<_>>();
    assert_eq!(
        grants,
        [
            NodeId(0),
            NodeId(1),
            NodeId(0),
            NodeId(1),
            NodeId(0),
            NodeId(1),
            NodeId(0),
            NodeId(1),
        ]
    );
    assert_eq!(
        play.signs()
            .events()
            .filter(|event| event.kind == KernelEventKind::StepYielded)
            .count(),
        6
    );
}

#[test]
fn exceeding_a_plan_owned_grant_is_atomic_and_observable() {
    let mut play = play(1, [Back::ExceedsGrant, Back::ExceedsGrant]);

    assert_eq!(play.step(), Err(SchedulerError::StepFuelExceeded));
    assert!(play
        .signs()
        .events()
        .any(|event| event.kind == KernelEventKind::StepFuelExceeded));
}

#[test]
fn cancellation_is_observed_between_finite_steps() {
    let mut play = play(
        1,
        [
            Back::Cancellable { cancelled: false },
            Back::Cancellable { cancelled: false },
        ],
    );

    play.step().unwrap();
    play.cancel().unwrap();

    assert!(matches!(
        play.drivers()[0],
        Back::Cancellable { cancelled: true }
    ));
    assert!(play
        .signs()
        .events()
        .any(|event| event.kind == KernelEventKind::RunCancelled));
}
