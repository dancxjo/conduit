use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver, SchedulerError,
};
use conduit_kernel::{
    CordId, Failure, FailureCode, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId,
    Operation, OperationAction, OperationInput, PortId,
};

struct RefusingOperation(Failure);

impl Operation for RefusingOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Fail(self.0)
    }

    fn resume(&mut self, _: OperationInput) -> OperationAction {
        OperationAction::Fail(self.0)
    }

    fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn cancel(&mut self) {}
}

#[test]
fn identical_detail_codes_do_not_erase_distinct_operation_failures() {
    type Play = FixedScheduler<
        OperationDriver<RefusingOperation, 1>,
        FixedValueStore<1, 1>,
        FixedSignLog<8>,
        1,
        1,
        1,
        1,
        1,
        1,
    >;
    for code in [
        FailureCode::InvalidInput,
        FailureCode::InvalidPort,
        FailureCode::InvalidLifecycle,
        FailureCode::StorageExhausted,
        FailureCode::StateCapacityExhausted,
        FailureCode::WorkBudgetExhausted,
        FailureCode::IdentityCapacityExhausted,
        FailureCode::HostOperationDenied,
        FailureCode::HostOperationFailed,
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
                maximum_step_work: 1,
            }],
            [CordSpec::local(
                CordId(0),
                (NodeId(0), PortId(0)),
                (NodeId(0), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            )],
            routes,
            [OperationDriver::new(RefusingOperation(failure)).unwrap()],
            FixedValueStore::new(1).unwrap(),
            FixedSignLog::new((8 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
        )
        .unwrap();
        assert_eq!(play.step(), Err(SchedulerError::OperationFailed(failure)));
    }
}
