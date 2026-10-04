//! Production-kernel Host Calls and acknowledged output pressure.
use super::*;
use conduit_kernel::{
    BoundedValueRef, CordEndpoint, CordId, FixedHostCallBindings, FixedRoutes, FixedValueStore,
    HostCallBinding, HostCallDisposition, HostCallOutcome, HostedSignLog, PortId, RemoteEndpointId,
    RouteRange, RouteTarget, ValueStorage,
    scheduler::{
        CanonicalValue, CordCapacity, CordSpec, FixedScheduler, HostCallBack, NodeSpec,
        SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
};
const OUTPUT: RemoteEndpointId = RemoteEndpointId(0);
enum Back {
    Expression(HostCallBack),
    Source { remaining: u16 },
}
impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Expression(back) => back.step(io, bytes),
            Self::Source { remaining } => {
                if *remaining == 0 {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_canonical(PortId(0), CanonicalValue::new(&[41]).unwrap())
                    .unwrap();
                *remaining -= 1;
                StepOutcome::Progress
            }
        }
    }
    fn cancel(&mut self) {
        if let Self::Expression(back) = self {
            <HostCallBack as StepBack<1>>::cancel(back);
        }
    }
}
type Kernel = FixedScheduler<Back, FixedValueStore<4, 1>, HostedSignLog, 2, 2, 1, 2, 2, 2, 2, 1>;
fn kernel() -> Kernel {
    let mut routes = FixedRoutes::<2, 2>::new(1);
    routes
        .install(
            NodeId(1),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(0), PortId(0)),
            }],
        )
        .unwrap();
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 1, len: 1 },
            &[RouteTarget {
                cord: CordId(1),
                sink: CordEndpoint::Remote(OUTPUT),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 1,
        pressure_policy: Default::default(),
    };
    let cords = [
        CordSpec::local(
            CordId(0),
            (NodeId(1), PortId(0)),
            (NodeId(0), PortId(0)),
            capacity(0),
        ),
        CordSpec::remote_egress(CordId(1), (NodeId(0), PortId(0)), OUTPUT, capacity(1)),
    ];
    let mut calls = FixedHostCallBindings::<2>::new(1);
    calls
        .install(
            NodeId(0),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 1,
                maximum_output_bytes: 1,
            },
        )
        .unwrap();
    calls.seal().unwrap();
    FixedScheduler::new_with_host_calls(
        [
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 4,
            },
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 4,
            },
        ],
        cords,
        routes,
        calls,
        [
            Back::Expression(HostCallBack::new(1)),
            Back::Source { remaining: 128 },
        ],
        FixedValueStore::new(4).unwrap(),
        HostedSignLog::new_with_remote_storage(
            4096,
            (4096 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
            1024,
            conduit_kernel::remote_sign_storage_bytes(1024).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn dispatch(kernel: &mut Kernel, host: &mut ExpressionHostCall) -> bool {
    let Some(request) = kernel.next_host_request() else {
        return false;
    };
    let output = host
        .invoke(
            request.node,
            request.call,
            request.request,
            kernel.host_value(request.input.value).unwrap(),
        )
        .unwrap();
    let value = kernel.store_host_value(output).unwrap();
    kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value, 1).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    true
}
#[test]
fn native_expression_kernel_retains_pressure_and_reuses_storage() {
    let (fragment, lowered, active, placement) = selected();
    let mut host = ExpressionHostCall::prepare(&fragment, &lowered, &active, &placement).unwrap();
    assert_eq!(host.node, NodeId(0));
    let mut kernel = kernel();
    let mut calls = 0;
    for _ in 0..1000 {
        kernel.step().unwrap();
        calls += usize::from(dispatch(&mut kernel, &mut host));
        assert!(kernel.values().used_items() <= 4);
        assert!(kernel.values().used_bytes() <= 4);
    }
    assert!(calls < 128);
    let mut received = 0;
    let mut drained = false;
    for _ in 0..4096 {
        let status = kernel.step().unwrap();
        calls += usize::from(dispatch(&mut kernel, &mut host));
        if let Some(output) = kernel.remote_egress_offer(OUTPUT, CordId(1)).unwrap() {
            assert_eq!(kernel.host_value(output.value).unwrap(), &[42]);
            kernel
                .remote_egress_accept(OUTPUT, CordId(1), output.sequence)
                .unwrap();
            kernel
                .remote_egress_delivered(OUTPUT, CordId(1), output.sequence)
                .unwrap();
            received += 1;
        }
        if status == SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(
        drained,
        "calls={calls} received={received} pending={}",
        kernel.pending_host_call_count()
    );
    assert_eq!(calls, 128);
    assert_eq!(received, 128);
    assert_eq!(kernel.values().used_items(), 0);
}
