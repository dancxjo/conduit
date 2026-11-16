//! Borrowed-input reference transfer across committed Host Call rearming.
use crate::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, HostCallRequest, NodeSpec, StepBack,
        StepInputBytes, StepIo, StepOutcome,
    },
    BoundedValueRef, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallBinding, HostCallDisposition, HostCallId, HostCallOutcome, NodeId, PortId,
    RemoteEndpointId, RequestId, ValueStorage,
};
struct FixtureBack {
    chain: bool,
    cancel_second: bool,
    phase: u8,
    staged: u8,
}
impl StepBack<1> for FixtureBack {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        self.staged = self.phase;
        if self.phase == 3 {
            return StepOutcome::Await;
        }
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        if self.phase == 0 {
            io.borrow_input_for_call(PortId(0)).unwrap();
            io.request_host_call(
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(value, 8).unwrap(),
            )
            .unwrap();
            self.staged = 1;
            return StepOutcome::Progress;
        }
        if self.cancel_second && self.phase == 2 {
            io.cancel_host_call(RequestId(1)).unwrap();
            self.staged = 4;
            return StepOutcome::Progress;
        }
        let Some((_, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        assert_eq!(
            outcome.disposition,
            if self.phase == 4 {
                HostCallDisposition::Cancelled
            } else {
                HostCallDisposition::Completed
            }
        );
        io.consume_host_completion().unwrap();
        if self.chain && self.phase == 1 {
            io.borrow_input_for_call(PortId(0)).unwrap();
            io.request_host_call(
                RequestId(1),
                HostCallId(1),
                BoundedValueRef::new(value, 8).unwrap(),
            )
            .unwrap();
            self.staged = 2;
        } else {
            io.consume(PortId(0)).unwrap();
            self.staged = 3;
        }
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        self.phase = self.staged;
    }
    fn accepts_input_while_host_call_pending(&self) -> bool {
        self.cancel_second
    }
    fn cancel(&mut self) {}
}
type Kernel =
    FixedScheduler<FixtureBack, FixedValueStore<2, 8>, FixedSignLog<64>, 1, 1, 1, 1, 0, 0, 2, 1>;
fn kernel(chain: bool) -> Kernel {
    let mut calls = FixedHostCallBindings::<2>::new(2);
    for index in 0..2 {
        calls
            .install(
                NodeId(0),
                HostCallBinding {
                    call: HostCallId(index),
                    maximum_input_bytes: 8,
                    maximum_output_bytes: 0,
                },
            )
            .unwrap();
    }
    calls.seal().unwrap();
    let mut routes = FixedRoutes::<0, 0>::new(1);
    routes.seal().unwrap();
    let signs = FixedSignLog::<64>::new_with_remote_storage(
        (64 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        16,
        crate::remote_sign_storage_bytes(16).unwrap(),
    )
    .unwrap();
    Kernel::new_with_active_counts_and_host_calls(
        1,
        1,
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 32,
        }],
        [CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 8,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        calls,
        [FixtureBack {
            chain,
            cancel_second: false,
            phase: 0,
            staged: 0,
        }],
        FixedValueStore::new(16).unwrap(),
        signs,
    )
    .unwrap()
}
fn next(kernel: &mut Kernel) -> HostCallRequest {
    for _ in 0..4 {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            return request;
        }
    }
    panic!("missing fixture request");
}
fn complete(kernel: &mut Kernel, request: HostCallRequest) {
    kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
}
fn run(chain: bool) {
    let mut kernel = kernel(chain);
    kernel
        .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, b"original")
        .unwrap();
    let first = next(&mut kernel);
    assert_eq!(
        kernel.values().reference_count(first.input.value).unwrap(),
        2
    );
    complete(&mut kernel, first);
    if chain {
        let second = next(&mut kernel);
        assert_eq!(second.input, first.input);
        complete(&mut kernel, second);
    }
    kernel.step().unwrap();
    assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert_eq!(kernel.pending_host_call_count(), 0);
    assert_eq!(kernel.drivers()[0].phase, 3);
    assert_eq!(
        kernel.values().used_bytes(),
        0,
        "no input references may survive the completed fixture"
    );
    assert!(kernel.host_value(first.input.value).is_err());
}
#[test]
fn one_borrowed_call_releases_original() {
    run(false);
}
#[test]
fn chained_borrowed_calls_release_original() {
    run(true);
}

#[test]
fn chained_borrow_retains_input_until_cancellation_is_acknowledged() {
    let mut kernel = kernel(true);
    kernel.drivers[0].cancel_second = true;
    kernel
        .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, b"original")
        .unwrap();
    let first = next(&mut kernel);
    complete(&mut kernel, first);
    let second = next(&mut kernel);
    assert_eq!(second.input, first.input);
    assert_eq!(
        kernel.values().reference_count(second.input.value).unwrap(),
        2
    );
    // Closing the source wakes the Back without replacing its borrowed input.
    kernel
        .close_remote_input(RemoteEndpointId(0), CordId(0))
        .unwrap();
    kernel.step().unwrap();
    let cancellation = kernel.next_host_cancellation().unwrap();
    assert_eq!(
        (cancellation.node, cancellation.request),
        (second.node, second.request)
    );
    assert!(kernel.next_host_cancellation().is_none());
    assert_eq!(kernel.pending_host_call_count(), 1);
    assert_eq!(kernel.host_value(second.input.value).unwrap(), b"original");
    assert_eq!(
        kernel.values().reference_count(second.input.value).unwrap(),
        2
    );
    kernel.step().unwrap();
    assert_eq!(kernel.pending_host_call_count(), 1);
    assert_eq!(kernel.host_value(second.input.value).unwrap(), b"original");
    kernel
        .complete_host_call(
            second.node,
            second.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Cancelled,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    kernel.step().unwrap();
    assert_eq!(kernel.pending_host_call_count(), 0);
    assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert_eq!(kernel.values().used_bytes(), 0);
    assert!(kernel.host_value(second.input.value).is_err());
}
