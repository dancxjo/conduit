//! Production HostCallBack lifecycle with exact selected owner and finite stores.
use super::*;
use alloc::vec::Vec;
use conduit_kernel::{scheduler::*, *};
enum Driver {
    Call(HostCallBack),
    Source(Option<ValueRef>),
    Sink(Vec<u8>),
}
impl StepBack<1> for Driver {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Call(back) => back.step(io, bytes),
            Self::Source(value) => {
                if let Some(value) = value.take() {
                    io.send(PortId(0), value).unwrap();
                    StepOutcome::Progress
                } else {
                    StepOutcome::Complete
                }
            }
            Self::Sink(result) => {
                if io.input(PortId(0)).is_some() {
                    result.extend_from_slice(bytes.input(PortId(0)).unwrap());
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
    fn cancel(&mut self) {
        if let Self::Call(back) = self {
            <HostCallBack as StepBack<1>>::cancel(back)
        }
    }
}
#[test]
fn actual_back_requests_exact_owner_and_commits_completion() {
    let (fragment, lowered, active, placement) = super::tests::selected();
    let mut owner = super::tests::bind(&fragment, &lowered, &active, &placement).unwrap();
    let contract = HistoryReadContract::prepare().unwrap();
    let input = codec::encode_request(&contract, 17).unwrap();
    let mut values = FixedValueStore::<8, 2304>::new(18432).unwrap();
    let value = values.store(&input).unwrap();
    let mut routes = FixedRoutes::<3, 2>::new(1);
    for (source, cord, sink) in [(1, 0, 0), (0, 1, 2)] {
        routes
            .install(
                NodeId(source),
                PortId(0),
                RouteRange {
                    start: cord,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord),
                    sink: CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<3>::new(1);
    bindings
        .install(
            NodeId(0),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 2304,
                maximum_output_bytes: 2304,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let node = |cord| NodeSpec {
        input_cords: [cord],
        maximum_step_fuel: 8,
    };
    let cord = |id, source, sink| {
        CordSpec::local(
            CordId(id),
            (NodeId(source), PortId(0)),
            (NodeId(sink), PortId(0)),
            CordCapacity {
                slot_start: id,
                item_capacity: 1,
                byte_capacity: 2304,
                pressure_policy: Default::default(),
            },
        )
    };
    let signs = FixedSignLog::<64>::new((64 * core::mem::size_of::<KernelEvent>()) as u32).unwrap();
    let mut kernel = FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2, 3, 1>::new_with_host_calls(
        [node(Some(CordId(0))), node(None), node(Some(CordId(1)))],
        [cord(0, 1, 0), cord(1, 0, 2)],
        routes,
        bindings,
        [
            Driver::Call(HostCallBack::new(2304)),
            Driver::Source(Some(value)),
            Driver::Sink(Vec::new()),
        ],
        values,
        signs,
    )
    .unwrap();
    let mut dispatched = false;
    let mut drained = false;
    for _ in 0..32 {
        let status = kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            assert!(!dispatched);
            dispatched = true;
            let output = owner
                .invoke(
                    request.node,
                    request.call,
                    request.request,
                    kernel.host_value(request.input.value).unwrap(),
                )
                .unwrap()
                .to_vec();
            let value = kernel.store_host_value(&output).unwrap();
            kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(BoundedValueRef::new(value, 2304).unwrap()),
                        failure: None,
                    },
                )
                .unwrap();
        }
        if status == SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(dispatched);
    assert!(drained);
    let Driver::Sink(result) = &kernel.drivers()[2] else {
        panic!("sink")
    };
    assert_eq!(
        codec::decode_result(&contract, result).unwrap(),
        17u32.to_le_bytes()
    );
    assert!(kernel
        .signs()
        .events()
        .any(|event| event.kind == KernelEventKind::HostCallCompleted));
}
