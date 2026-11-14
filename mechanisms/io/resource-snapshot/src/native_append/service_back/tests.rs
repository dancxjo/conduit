//! Production scheduler, real request/completion ownership and transactional commit.
use super::*;
use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, HostCallRequest, NodeSpec, SchedulerError,
        SchedulerStatus,
    },
    BoundedValueRef, CordEndpoint, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostCallBinding, HostCallOutcome, NodeId, RemoteEndpointId, RouteRange,
    RouteTarget,
};
use std::{cell::Cell, rc::Rc};

fn shape() -> RecordServiceShape {
    RecordServiceShape {
        append: RecordServiceOperation {
            input: PortId(0),
            output: PortId(0),
            call: HostCallId(1),
            maximum_input_bytes: 8,
            maximum_output_bytes: 8,
        },
        read: RecordServiceOperation {
            input: PortId(1),
            output: PortId(1),
            call: HostCallId(0),
            maximum_input_bytes: 8,
            maximum_output_bytes: 8,
        },
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct View {
    pending: Option<(RequestId, usize)>,
    read_len: Option<usize>,
    next: u32,
}
struct Observed<const READ_BYTES: usize> {
    back: RecordServiceBack<2, READ_BYTES>,
    reject: Rc<Cell<bool>>,
    view: Rc<Cell<View>>,
}
impl<const READ_BYTES: usize> Observed<READ_BYTES> {
    fn observe(&self) {
        self.view.set(View {
            pending: self.back.pending,
            read_len: self.back.read_len,
            next: self.back.next_request,
        });
    }
}
impl<const READ_BYTES: usize> StepBack<2> for Observed<READ_BYTES> {
    fn terminal_transductions(
        &self,
    ) -> [Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; 2] {
        <RecordServiceBack<2, READ_BYTES> as StepBack<2>>::terminal_transductions(&self.back)
    }
    fn step(&mut self, io: &mut StepIo<2>, bytes: &StepInputBytes<'_, 2>) -> StepOutcome {
        let outcome = self.back.step(io, bytes);
        self.observe();
        // Deliberately violate the outcome protocol AFTER staging, so the real
        // kernel rejects the transaction without invoking step_committed.
        if self.reject.replace(false) {
            StepOutcome::Await
        } else {
            outcome
        }
    }
    fn step_committed(&mut self) {
        <RecordServiceBack<2, READ_BYTES> as StepBack<2>>::step_committed(&mut self.back);
        self.observe();
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        <RecordServiceBack<2, READ_BYTES> as StepBack<2>>::prepared_output(&self.back, port)
    }
    fn cancel(&mut self) {
        <RecordServiceBack<2, READ_BYTES> as StepBack<2>>::cancel(&mut self.back);
        self.observe();
    }
}
type Kernel<const READ_BYTES: usize> = FixedScheduler<
    Observed<READ_BYTES>,
    FixedValueStore<16, READ_BYTES>,
    FixedSignLog<512>,
    1,
    4,
    2,
    4,
    2,
    2,
    2,
    1,
>;
struct Fixture<const READ_BYTES: usize> {
    kernel: Kernel<READ_BYTES>,
    reject: Rc<Cell<bool>>,
    view: Rc<Cell<View>>,
    sequences: [u64; 2],
}
impl<const READ_BYTES: usize> Fixture<READ_BYTES> {
    fn new() -> Self {
        Self::with_back(RecordServiceBack::new(shape()).unwrap())
    }
    fn with_back(service: RecordServiceBack<2, READ_BYTES>) -> Self {
        let reject = Rc::new(Cell::new(false));
        let view = Rc::new(Cell::new(View::default()));
        let back = Observed {
            back: service,
            reject: reject.clone(),
            view: view.clone(),
        };
        let mut routes = FixedRoutes::<2, 2>::new(2);
        for port in 0..2_u16 {
            routes
                .install(
                    NodeId(0),
                    PortId(port),
                    RouteRange {
                        start: port,
                        len: 1,
                    },
                    &[RouteTarget {
                        cord: CordId(2 + port),
                        sink: CordEndpoint::Remote(RemoteEndpointId(2 + port)),
                    }],
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let cords = core::array::from_fn(|i| {
            let capacity = CordCapacity {
                slot_start: i as u16,
                item_capacity: 1,
                byte_capacity: READ_BYTES as u32,
                pressure_policy: Default::default(),
            };
            if i < 2 {
                CordSpec::remote_ingress(
                    CordId(i as u16),
                    RemoteEndpointId(i as u16),
                    (NodeId(0), PortId(i as u16)),
                    capacity,
                )
            } else {
                CordSpec::remote_egress(
                    CordId(i as u16),
                    (NodeId(0), PortId((i - 2) as u16)),
                    RemoteEndpointId(i as u16),
                    capacity,
                )
            }
        });
        let mut bindings = FixedHostCallBindings::<2>::new(2);
        for operation in &back.back.operations {
            bindings
                .install(
                    NodeId(0),
                    HostCallBinding {
                        call: operation.call,
                        maximum_input_bytes: operation.maximum_input_bytes,
                        maximum_output_bytes: operation.maximum_output_bytes,
                    },
                )
                .unwrap();
        }
        bindings.seal().unwrap();
        let signs = FixedSignLog::<512>::new_with_remote_storage(
            (512 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
            512,
            conduit_kernel::remote_sign_storage_bytes(512).unwrap(),
        )
        .unwrap();
        let kernel = Kernel::new_with_host_calls(
            [NodeSpec {
                input_cords: [Some(CordId(0)), Some(CordId(1))],
                maximum_step_fuel: 8,
            }],
            cords,
            routes,
            bindings,
            [back],
            FixedValueStore::new((16 * READ_BYTES) as u32).unwrap(),
            signs,
        )
        .unwrap();
        Self {
            kernel,
            reject,
            view,
            sequences: [0; 2],
        }
    }
    fn input(&mut self, operation: usize) {
        self.kernel
            .admit_remote_input(
                RemoteEndpointId(operation as u16),
                CordId(operation as u16),
                self.sequences[operation],
                b"in",
            )
            .unwrap();
        self.sequences[operation] += 1;
    }
    fn step(&mut self) {
        let status = self.kernel.step().unwrap();
        assert!(
            matches!(status, SchedulerStatus::Progress { .. }),
            "{status:?}, view={:?}, closed={:?}",
            self.view.get(),
            self.kernel.drivers()[0].back.closed
        );
    }
    fn request(&mut self, operation: usize, id: u32) -> HostCallRequest {
        self.step();
        let request = self.kernel.next_host_request().unwrap();
        assert_eq!(
            request.call,
            if operation == 0 {
                HostCallId(1)
            } else {
                HostCallId(0)
            }
        );
        assert_eq!(request.request, RequestId(id));
        assert!(self.kernel.next_host_request().is_none());
        request
    }
    fn complete(&mut self, request: HostCallRequest, bytes: &[u8]) {
        let value = self.kernel.store_host_value(bytes).unwrap();
        self.kernel
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(value, bytes.len() as u32).unwrap()),
                    failure: None,
                },
            )
            .unwrap();
    }
    fn output(&mut self, operation: usize) -> Option<std::vec::Vec<u8>> {
        let id = (operation + 2) as u16;
        self.kernel
            .remote_egress_offer(RemoteEndpointId(id), CordId(id))
            .unwrap()
            .map(|offer| self.kernel.host_value(offer.value).unwrap().to_vec())
    }
    fn drain(&mut self, operation: usize) {
        let id = (operation + 2) as u16;
        let offer = self
            .kernel
            .remote_egress_offer(RemoteEndpointId(id), CordId(id))
            .unwrap()
            .unwrap();
        self.kernel
            .remote_egress_accept(RemoteEndpointId(id), CordId(id), offer.sequence)
            .unwrap();
        self.kernel
            .remote_egress_delivered(RemoteEndpointId(id), CordId(id), offer.sequence)
            .unwrap();
    }
}
#[test]
fn read_pressure_preserves_read_and_continues_append_without_another_read() {
    let mut f = Fixture::<8>::new();
    f.input(1);
    let request = f.request(1, 0);
    f.complete(request, b"old");
    f.step();
    f.step(); // fill read output
    f.input(1);
    let request = f.request(1, 1);
    f.complete(request, b"new");
    f.step(); // own fixed response slot
    assert_eq!(f.view.get().read_len, Some(3));
    f.input(1); // further read must remain queued
    for id in 2..5 {
        f.input(0);
        let request = f.request(0, id);
        f.complete(request, b"appended");
        f.step();
        assert_eq!(f.output(0).unwrap(), b"appended");
        f.drain(0);
        assert_eq!(f.output(1).unwrap(), b"old");
        assert_eq!(f.view.get().read_len, Some(3));
    }
    f.drain(1);
    f.step();
    assert_eq!(f.output(1).unwrap(), b"new");
    assert_eq!(f.view.get().read_len, None);
    let request = f.request(1, 5);
    assert_eq!(request.request, RequestId(5));
}
#[test]
fn continuously_ready_inputs_alternate_and_append_pressure_preserves_completion() {
    let mut f = Fixture::<8>::new();
    for id in 0..6 {
        let operation = id as usize % 2;
        if id == 0 {
            f.input(0);
            f.input(1);
        }
        let request = f.request(operation, id);
        f.complete(request, b"ok");
        f.step();
        if operation == 1 {
            f.step();
        }
        assert_eq!(f.output(operation).unwrap(), b"ok");
        f.drain(operation);
        f.input(operation);
    }
    let request = f.request(0, 6);
    f.complete(request, b"first");
    f.step();
    f.input(0);
    // Read is next in the fair input rotation; consume it first.
    let read = f.request(1, 7);
    f.complete(read, b"read");
    f.step();
    f.step();
    let request = f.request(0, 8);
    f.complete(request, b"second");
    let before = f.view.get();
    // SchedulerStatus::Progress records an invoked logical node even when its
    // Back returns Await. This first invocation observes pressure and sleeps;
    // the following decision must be Idle without consuming the completion.
    assert_eq!(
        f.kernel.step().unwrap(),
        SchedulerStatus::Progress { node: NodeId(0) }
    );
    assert_eq!(f.view.get(), before);
    assert_eq!(f.view.get().pending, Some((RequestId(8), 0)));
    assert_eq!(f.kernel.pending_host_call_count(), 1);
    assert!(f.kernel.next_host_request().is_none());
    assert_eq!(f.kernel.step().unwrap(), SchedulerStatus::Idle);
    assert_eq!(f.output(0).unwrap(), b"first");
    f.drain(0);
    f.step();
    assert_eq!(f.output(0).unwrap(), b"second");
}
#[test]
fn rejected_request_and_read_staging_do_not_publish_private_state() {
    let mut f = Fixture::<8>::new();
    f.input(1);
    f.reject.set(true);
    assert_eq!(f.kernel.step(), Err(SchedulerError::FalseProgress));
    assert_eq!(f.view.get(), View::default());
    assert!(f.kernel.next_host_request().is_none());
    let request = f.request(1, 0);
    f.complete(request, b"kept");
    f.reject.set(true);
    assert_eq!(f.kernel.step(), Err(SchedulerError::FalseProgress));
    assert_eq!(f.view.get().read_len, None);
    assert_eq!(f.view.get().pending, Some((RequestId(0), 1)));
    assert!(f.output(1).is_none());
    assert_eq!(f.kernel.pending_host_call_count(), 1);
    f.step();
    assert_eq!(f.view.get().read_len, Some(4));
    f.reject.set(true);
    assert_eq!(f.kernel.step(), Err(SchedulerError::FalseProgress));
    assert_eq!(f.view.get().read_len, Some(4));
    assert!(f.output(1).is_none());
    f.step();
    assert_eq!(f.output(1).unwrap(), b"kept");
}
#[test]
fn cancellation_discards_private_response_and_rejects_late_completion() {
    let mut f = Fixture::<8>::new();
    f.input(1);
    let request = f.request(1, 0);
    f.complete(request, b"held");
    f.step();
    assert_eq!(f.view.get().read_len, Some(4));
    f.kernel.cancel().unwrap();
    assert_eq!(f.view.get().read_len, None);
    let mut f = Fixture::<8>::new();
    f.input(0);
    let pending = f.request(0, 0);
    f.kernel.cancel().unwrap();
    assert_eq!(f.view.get().read_len, None);
    assert_eq!(f.view.get().pending, None);
    assert_eq!(
        f.kernel.complete_host_call(
            pending.node,
            pending.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Cancelled,
                output: None,
                failure: None
            }
        ),
        Err(SchedulerError::HostCallCompletionRejected)
    );
    assert!(f.kernel.next_host_request().is_none());
}
#[test]
fn constructor_refuses_aliases_out_of_table_and_unfunded_response() {
    let mut s = shape();
    s.read.call = s.append.call;
    assert!(matches!(
        RecordServiceBack::<2, 8>::new(s),
        Err(RecordServiceBackRefusal::InvalidShape)
    ));
    let mut s = shape();
    s.read.input = PortId(2);
    assert!(matches!(
        RecordServiceBack::<2, 8>::new(s),
        Err(RecordServiceBackRefusal::InvalidShape)
    ));
    assert!(matches!(
        RecordServiceBack::<2, 7>::new(shape()),
        Err(RecordServiceBackRefusal::InsufficientReadBacking)
    ));
}

#[path = "tests/cancellation.rs"]
mod cancellation;

#[path = "tests/terminal.rs"]
mod terminal;
