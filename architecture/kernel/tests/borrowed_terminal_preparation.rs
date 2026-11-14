#![cfg(feature = "alloc")]
use conduit_kernel::{scheduler::*, *};
#[derive(Clone, Copy)]
struct Back(bool);
impl StepBack<1> for Back {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Await
    }
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; 1] {
        [self.0.then(law)]
    }
}
fn law() -> AssignedTerminalTransduction {
    AssignedTerminalTransduction {
        input: PortId(0),
        output: PortId(0),
        normal_close: AssignedNormalCloseTransduction::Consume,
        abnormal: AssignedAbnormalTransduction::NotAccepted,
        cancellation: AssignedCancellationTransduction::NotCancellable,
    }
}
type Kernel = FixedScheduler<Back, FixedValueStore<1, 4>, FixedSignLog<8>, 2, 1, 1, 1, 2, 1, 1, 1>;
fn prepare(
    expected: &[[Option<AssignedTerminalTransduction>; 1]; 2],
    actual: &[[Option<AssignedTerminalTransduction>; 1]; 2],
) -> Result<PreparedKernelStorage<Back, Kernel, 2>, KernelPreparationError> {
    let mut routes = FixedRoutes::<2, 1>::new(1);
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<1>::new(1);
    calls.seal().unwrap();
    Kernel::prepare_boxed_with_terminal_transductions_ref(
        1,
        1,
        [
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 3,
            },
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 3,
            },
        ],
        [CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        calls,
        FixedValueStore::new(4).unwrap(),
        FixedSignLog::new((8 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
        usize::MAX,
        expected,
        actual,
    )
}
#[test]
fn borrowed_matrix_is_installed_and_mismatches_including_inactive_actual_refuse() {
    let expected = [[Some(law())], [None]];
    let prepared = prepare(&expected, &expected).unwrap();
    let mut kernel = prepared.bind([Back(true), Back(false)]);
    // Exact borrowed laws agree with the original actual drivers at bind.
    kernel.bind_terminal_transductions(expected).unwrap();
    let mut wrong = expected;
    wrong[0][0] = None;
    assert!(prepare(&expected, &wrong).is_err());
    let mut inactive = expected;
    inactive[1][0] = Some(law());
    assert!(prepare(&expected, &inactive).is_err());
    assert!(prepare(&inactive, &inactive).is_err());
}
