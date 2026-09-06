use super::*;

#[derive(Clone, Copy)]
enum AliasDriver {
    Source(Option<ValueRef>),
    Consumer {
        port: u16,
        pending: bool,
        consume: bool,
    },
}
impl StepOperation<2> for AliasDriver {
    fn step(&mut self, io: &mut StepIo<2>, _: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Source(value) => {
                let Some(value) = value.take() else {
                    return StepOutcome::Complete;
                };
                io.send(PortId(0), value).unwrap();
                StepOutcome::Progress
            }
            Self::Consumer {
                port,
                pending,
                consume,
            } => {
                if *pending {
                    if io.host_completion().is_none() {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion().unwrap();
                    *pending = false;
                    *port += 1;
                    return if *port == 2 {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Progress
                    };
                }
                let Some(value) = io.input(PortId(*port)) else {
                    return StepOutcome::Await;
                };
                if *consume {
                    io.consume(PortId(*port)).unwrap();
                }
                io.request_host_operation(
                    RequestId(u32::from(*port)),
                    HostOperationId(0),
                    BoundedValueRef::new(value, 4).unwrap(),
                )
                .unwrap();
                *pending = true;
                StepOutcome::Progress
            }
        }
    }
}

fn exercise(consume: bool) {
    let mut values = FixedValueStore::<2, 4>::new(8).unwrap();
    let value = values.store(&[42]).unwrap();
    let mut routes = FixedRoutes::<4, 2>::new(2);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 2 },
            &[
                RouteTarget {
                    cord: CordId(0),
                    sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
                },
                RouteTarget {
                    cord: CordId(1),
                    sink: crate::CordEndpoint::local(NodeId(1), PortId(1)),
                },
            ],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut bindings = FixedHostOperationBindings::<2>::new(1);
    bindings
        .install(
            NodeId(1),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 4,
                maximum_output_bytes: 4,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let charge = core::mem::size_of::<crate::KernelEvent>() as u32;
    let mut scheduler =
        FixedScheduler::<_, _, _, 2, 2, 2, 2, 4, 2, 2, 1>::new_with_host_operations(
            [node([None, None]), node([Some(CordId(0)), Some(CordId(1))])],
            [cord(0, 0, 0, 1, 0), cord(1, 0, 0, 1, 1)],
            routes,
            bindings,
            [
                AliasDriver::Source(Some(value)),
                AliasDriver::Consumer {
                    port: 0,
                    pending: false,
                    consume,
                },
            ],
            values,
            FixedSignLog::<64>::new(charge * 64).unwrap(),
        )
        .unwrap();
    let mut requests = 0;
    for _ in 0..20 {
        match scheduler.step() {
            Err(error) if !consume => {
                assert_eq!(error, SchedulerError::InvalidHostOperationAccess);
                assert!(scheduler.next_host_request().is_none());
                return;
            }
            Err(error) => {
                panic!("consumed reference must transfer while its queued alias remains: {error:?}")
            }
            Ok(SchedulerStatus::Complete) => {
                assert!(consume);
                assert_eq!(requests, 2);
                assert_eq!(scheduler.values().used_items(), 0);
                return;
            }
            _ => {}
        }
        if let Some(request) = scheduler.next_host_request() {
            assert!(consume);
            assert_eq!(request.input.value, value);
            assert_eq!(request.request, RequestId(requests));
            assert_eq!(scheduler.host_value(value).unwrap(), &[42]);
            requests += 1;
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .unwrap();
        }
    }
    panic!("finite alias proof did not terminate");
}
#[test]
fn consumed_reference_can_transfer_while_another_input_keeps_its_alias() {
    exercise(true);
}
#[test]
fn unconsumed_queued_reference_cannot_be_borrowed_for_host_work() {
    exercise(false);
}
