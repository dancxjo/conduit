use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, StepBack, StepInputBytes,
    StepIo, StepOutcome,
};
use conduit_kernel::{
    CordId, Failure, FailureCode, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent,
    KernelEventKind, NodeId, PortId,
};

struct RefusingBack(Failure);

impl StepBack<1> for RefusingBack {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Fail(self.0)
    }
}

#[test]
fn identical_detail_codes_do_not_erase_distinct_back_failures() {
    type Play =
        FixedScheduler<RefusingBack, FixedValueStore<1, 1>, FixedSignLog<8>, 1, 1, 1, 1, 1, 1>;
    for code in [
        FailureCode::InvalidInput,
        FailureCode::InvalidPort,
        FailureCode::InvalidLifecycle,
        FailureCode::StorageExhausted,
        FailureCode::StateCapacityExhausted,
        FailureCode::WorkBudgetExhausted,
        FailureCode::IdentityCapacityExhausted,
        FailureCode::HostCallDenied,
        FailureCode::HostCallFailed,
        FailureCode::Cancelled,
    ] {
        let failure = Failure { code, detail: 42 };
        let mut routes = FixedRoutes::<1, 1>::new(1);
        routes.seal().unwrap();
        let mut play = Play::new_with_active_counts(
            1,
            0,
            [NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 1,
            }],
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
            [RefusingBack(failure)],
            FixedValueStore::new(1).unwrap(),
            FixedSignLog::new((8 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
        )
        .unwrap();
        assert_eq!(play.step(), Err(SchedulerError::BackFailed(failure)));
        assert!(play
            .signs()
            .events()
            .any(|event| event.kind == KernelEventKind::BackFailed));
    }
}
