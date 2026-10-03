use super::leaf;
use crate::machine_membrane::{RegisterRefusal, register_call::RegisterHostCall};
use conduit_kernel::{
    BoundedValueRef, CordEndpoint, CordId, FixedHostCallBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostCallBinding, HostCallDisposition, HostCallId, HostCallOutcome, NodeId,
    PortId, RouteRange, RouteTarget, ValueStorage,
    scheduler::{
        CanonicalValue, CordCapacity, CordSpec, FixedScheduler, HostCallBack, NodeSpec,
        SchedulerError, SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
};

enum Back {
    Source { bytes: [u8; 16], remaining: u16 },
    Register(HostCallBack),
    Sink { received: u16, blocked: bool },
}

impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Register(back) => back.step(io, bytes),
            Self::Source { bytes, remaining } => {
                if *remaining == 0 {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_canonical(PortId(0), CanonicalValue::new(bytes).unwrap())
                    .unwrap();
                *remaining -= 1;
                StepOutcome::Progress
            }
            Self::Sink { received, blocked } => {
                if *blocked {
                    return StepOutcome::Await;
                }
                if io.input(PortId(0)).is_some() {
                    assert_eq!(bytes.input(PortId(0)).unwrap(), &[42, 0, 0, 0]);
                    io.consume(PortId(0)).unwrap();
                    *received += 1;
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }
    fn cancel(&mut self) {
        if let Self::Register(back) = self {
            <HostCallBack as StepBack<1>>::cancel(back);
        }
    }
}

type Kernel =
    FixedScheduler<Back, FixedValueStore<4, 16>, FixedSignLog<2048>, 3, 2, 1, 2, 3, 2, 3, 1>;

fn kernel(input: [u8; 16], count: u16, blocked: bool) -> Kernel {
    let mut routes = FixedRoutes::<3, 2>::new(1);
    let cords = core::array::from_fn(|index| {
        let index = index as u16;
        routes
            .install(
                NodeId(index),
                PortId(0),
                RouteRange {
                    start: index,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index),
                    sink: CordEndpoint::local(NodeId(index + 1), PortId(0)),
                }],
            )
            .unwrap();
        CordSpec::local(
            CordId(index),
            (NodeId(index), PortId(0)),
            (NodeId(index + 1), PortId(0)),
            CordCapacity {
                slot_start: index,
                item_capacity: 1,
                byte_capacity: if index == 0 { 16 } else { 4 },
                pressure_policy: Default::default(),
            },
        )
    });
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<3>::new(1);
    bindings
        .install(
            NodeId(1),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 16,
                maximum_output_bytes: 4,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    FixedScheduler::new_with_host_calls(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 4,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 4,
            },
            NodeSpec {
                input_cords: [Some(CordId(1))],
                maximum_step_fuel: 4,
            },
        ],
        cords,
        routes,
        bindings,
        [
            Back::Source {
                bytes: input,
                remaining: count,
            },
            Back::Register(HostCallBack::new(16)),
            Back::Sink {
                received: 0,
                blocked,
            },
        ],
        FixedValueStore::new(64).unwrap(),
        FixedSignLog::new((2048 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32)
            .unwrap(),
    )
    .unwrap()
}

fn write_request() -> [u8; 16] {
    // Request construction executes the checked portable plot, not a private
    // Rust device codec. Rust decoding below the boundary is the primitive ABI.
    use conduit_core::{
        ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoValue, kind_id,
    };
    use conduit_plot::{
        PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog,
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    };
    let source = include_str!("../../plots/machine/register32.conduit");
    let (startup, _) = crate::machine_membrane::register_call::catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "machine-register-write-request",
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    let ConfigurationValue::Text(program) = &expanded.gears[0].configuration[0].value else {
        panic!("checked program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let fields = [("offset", 0_u32), ("value", 42)].map(|(name, value)| {
        StructuredFieldValue::new(
            name,
            StructuredInfoValue::leaf(
                StructuredInfoType::leaf(kind_id("value/u32")).unwrap(),
                value.to_le_bytes().to_vec(),
            )
            .unwrap(),
        )
        .unwrap()
    });
    let input = StructuredInfoValue::record(program.input_type.clone(), fields.to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    PreparedPortableExpressionEvaluator::new(&program)
        .unwrap()
        .evaluate(&input)
        .unwrap()
        .try_into()
        .unwrap()
}

#[test]
fn exact_host_call_runs_plot_requests_through_the_production_kernel_with_reuse() {
    let mut registers = [0];
    let mut host = RegisterHostCall::new(NodeId(1), leaf(&mut registers, true));
    let mut kernel = kernel(write_request(), 128, false);
    let mut calls = 0;
    let mut drained = false;
    for _ in 0..4096 {
        let status = kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            assert_eq!(kernel.pending_host_call_count(), 1);
            let output = host
                .invoke(
                    request.node,
                    request.call,
                    kernel.host_value(request.input.value).unwrap(),
                )
                .unwrap();
            let output = kernel.store_host_value(&output).unwrap();
            kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(BoundedValueRef::new(output, 4).unwrap()),
                        failure: None,
                    },
                )
                .unwrap();
            calls += 1;
        }
        assert!(kernel.values().used_items() <= 4);
        assert!(kernel.values().used_bytes() <= 64);
        if status == SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(drained);
    assert_eq!(calls, 128);
    assert!(matches!(
        kernel.drivers()[2],
        Back::Sink { received: 128, .. }
    ));
    assert_eq!(kernel.values().used_items(), 0);
    assert_eq!(registers, [42]);
}

#[test]
fn denied_register_calls_keep_the_native_refusal_in_kernel_failure() {
    let mut registers = [9];
    let mut host = RegisterHostCall::new(NodeId(1), leaf(&mut registers, false));
    let mut kernel = kernel(write_request(), 1, false);
    let mut failure = None;
    for _ in 0..32 {
        match kernel.step() {
            Err(SchedulerError::BackFailed(observed)) => {
                failure = Some(observed);
                break;
            }
            Ok(_) => (),
            other => panic!("unexpected outcome {other:?}"),
        }
        if let Some(request) = kernel.next_host_request() {
            let refusal = host
                .invoke(
                    request.node,
                    request.call,
                    kernel.host_value(request.input.value).unwrap(),
                )
                .unwrap_err();
            kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Denied,
                        output: None,
                        failure: Some(refusal.failure()),
                    },
                )
                .unwrap();
        }
    }
    assert_eq!(failure, Some(RegisterRefusal::ReadOnly.failure()));
    assert_eq!(registers, [9]);
    assert!(matches!(
        kernel.drivers()[2],
        Back::Sink { received: 0, .. }
    ));
}

#[test]
fn register_abi_and_exact_binding_refuse_before_effect_and_cancel_revokes() {
    let mut registers = [9];
    let mut host = RegisterHostCall::new(NodeId(1), leaf(&mut registers, true));
    let mut request = write_request();
    assert_eq!(
        host.invoke(NodeId(0), HostCallId(0), &request),
        Err(RegisterRefusal::WrongBinding)
    );
    assert_eq!(
        host.cancel(NodeId(1), HostCallId(1)),
        Err(RegisterRefusal::WrongBinding)
    );
    for byte in 9..16 {
        request[byte] = 1;
        assert_eq!(
            host.invoke(NodeId(1), HostCallId(0), &request),
            Err(RegisterRefusal::InvalidRequest)
        );
        request[byte] = 0;
    }
    request[8] = 2;
    assert_eq!(
        host.invoke(NodeId(1), HostCallId(0), &request),
        Err(RegisterRefusal::InvalidRequest)
    );
    request[8] = 0; // Read cannot carry a hidden write value.
    assert_eq!(
        host.invoke(NodeId(1), HostCallId(0), &request),
        Err(RegisterRefusal::InvalidRequest)
    );
    assert_eq!(registers, [9]);
    host.cancel(NodeId(1), HostCallId(0)).unwrap();
    assert!(matches!(
        host.invoke(NodeId(1), HostCallId(0), &write_request()),
        Err(RegisterRefusal::Capability(
            conduit_core::BaseCapabilityRefusal::Revoked
        ))
    ));
    assert_eq!(registers, [9]);
}

#[test]
fn cancellation_rejects_a_late_register_completion() {
    let mut registers = [0];
    let mut host = RegisterHostCall::new(NodeId(1), leaf(&mut registers, true));
    let mut kernel = kernel(write_request(), 1, false);
    let request = (0..16)
        .find_map(|_| {
            kernel.step().unwrap();
            kernel.next_host_request()
        })
        .unwrap();
    let output = host
        .invoke(
            request.node,
            request.call,
            kernel.host_value(request.input.value).unwrap(),
        )
        .unwrap();
    let output = kernel.store_host_value(&output).unwrap();
    kernel.cancel().unwrap();
    host.cancel(request.node, request.call).unwrap();
    assert_eq!(
        kernel.complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 4).unwrap()),
                failure: None,
            }
        ),
        Err(SchedulerError::HostCallCompletionRejected)
    );
    assert_eq!(kernel.values().used_items(), 0);
}

#[test]
fn output_pressure_retains_one_completion_without_repeating_register_effects() {
    let mut registers = [0];
    let mut host = RegisterHostCall::new(NodeId(1), leaf(&mut registers, true));
    let mut kernel = kernel(write_request(), 128, true);
    let mut calls = 0;
    for _ in 0..128 {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            let output = host
                .invoke(
                    request.node,
                    request.call,
                    kernel.host_value(request.input.value).unwrap(),
                )
                .unwrap();
            let output = kernel.store_host_value(&output).unwrap();
            kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(BoundedValueRef::new(output, 4).unwrap()),
                        failure: None,
                    },
                )
                .unwrap();
            calls += 1;
        }
        assert!(kernel.values().used_items() <= 4);
        assert!(kernel.values().used_bytes() <= 64);
    }
    assert_eq!(calls, 2); // One delivered value and one pressure-held completion.
    assert_eq!(kernel.pending_host_call_count(), 1);
    assert!(matches!(
        kernel.drivers()[2],
        Back::Sink { received: 0, .. }
    ));
    kernel.cancel().unwrap();
    host.cancel(NodeId(1), HostCallId(0)).unwrap();
    assert_eq!(kernel.values().used_items(), 0);
}
