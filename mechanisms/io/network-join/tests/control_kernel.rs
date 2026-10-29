//! Numeric kernel protocol fixture, not an initialized Host, Body or radio.
use conduit_kernel::{
    self as k,
    scheduler::{CordCapacity, CordSpec, NodeSpec, SchedulerError, SchedulerStatus},
    ValueStorage,
};
use conduit_network_join_call::{NetworkControlBack, NetworkControlKernel};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn bounded_source_join_and_sink_follow_real_kernel_requests_and_failures() {
    for failed_node in [None, Some(0), Some(1), Some(2)] {
        execute(failed_node, 0);
    }
}
#[test]
fn missing_or_cancelled_credentials_do_not_reach_the_join_call() {
    for mode in [1, 2] {
        execute(None, mode);
    }
}
#[test]
fn both_full_167_byte_results_fit_the_production_three_slot_kernel() {
    execute(None, 3);
    println!(
        "production kernel bytes={} storage bytes={} saving={}",
        core::mem::size_of::<conduit_network_join_call::PreparedNetworkKernel>(),
        conduit_network_join_call::PreparedNetworkKernelStorage::allocation_bytes(),
        core::mem::size_of::<k::FixedValueStore<8, 167>>()
            - core::mem::size_of::<k::FixedValueStore<3, 167>>()
    );
}
#[test]
fn cancellation_at_each_pending_call_retires_without_allocating() {
    for node in 0..3 {
        execute(Some(node), 4);
    }
}
fn execute(failed_node: Option<u16>, source_mode: u8) {
    let mut values = k::FixedValueStore::<3, 167>::new(3 * 167).unwrap();
    let empty = values.store(&[]).unwrap();
    let mut routes = k::FixedRoutes::<3, 2>::new(1);
    for id in 0..2 {
        routes
            .install(
                k::NodeId(id),
                k::PortId(0),
                k::RouteRange { start: id, len: 1 },
                &[k::RouteTarget {
                    cord: k::CordId(id),
                    sink: k::CordEndpoint::local(k::NodeId(id + 1), k::PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = k::FixedHostCallBindings::<3>::new(1);
    for id in 0..3 {
        bindings
            .install(
                k::NodeId(id),
                k::HostCallBinding {
                    call: k::HostCallId(0),
                    maximum_input_bytes: if id == 0 { 1 } else { 167 },
                    maximum_output_bytes: if id == 2 { 1 } else { 167 },
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    let nodes = core::array::from_fn(|i| NodeSpec {
        input_cords: [if i == 0 {
            None
        } else {
            Some(k::CordId(i as u16 - 1))
        }],
        maximum_step_fuel: 4,
    });
    let cords = core::array::from_fn(|i| {
        CordSpec::local(
            k::CordId(i as u16),
            (k::NodeId(i as u16), k::PortId(0)),
            (k::NodeId(i as u16 + 1), k::PortId(0)),
            CordCapacity {
                slot_start: i as u16,
                item_capacity: 1,
                byte_capacity: 167,
                pressure_policy: Default::default(),
            },
        )
    });
    let mut kernel = NetworkControlKernel::new_with_host_calls(
        nodes,
        cords,
        routes,
        bindings,
        [
            NetworkControlBack::credentials(empty).unwrap(),
            NetworkControlBack::join(),
            NetworkControlBack::attachment_sign(),
        ],
        values,
        k::HostedSignLog::try_new(128, (128 * core::mem::size_of::<k::KernelEvent>()) as u32)
            .unwrap(),
    )
    .unwrap();
    let mut join = [0; 167];
    let mut join_len = conduit_net::encode_network_join_request(
        conduit_net::NetworkJoinRequest {
            ssid: b"fixture-net",
            credential: b"fixture-password",
        },
        &mut join,
    )
    .unwrap();
    let mut attachment = [0; 167];
    let mut attachment_len = conduit_net::encode_network_attachment(
        conduit_net::NetworkAttachmentInfo {
            attachment_id: "attachment/result",
            host_id: "host/current",
            boot_id: "boot/current",
            resource_pool_id: "radio/current",
            generation: 9,
        },
        &mut attachment,
    )
    .unwrap();
    if source_mode == 3 {
        join.fill(0x5a);
        attachment.fill(0xa5);
        join_len = join.len();
        attachment_len = attachment.len();
    }
    let failure = k::Failure {
        code: k::FailureCode::HostCallDenied,
        detail: 42,
    };
    let mut calls = [0; 3];
    let mut emitted = [None; 2];
    let mut finished = false;
    let ((), allocations) = allocation_probe::observe(|| {
        for _ in 0..100 {
            if let Some(request) = kernel.next_host_request() {
                let node = usize::from(request.node.0);
                calls[node] += 1;
                assert_eq!(request.request, k::RequestId(0));
                if node != 0 {
                    assert_eq!(Some(request.input.value), emitted[node - 1]);
                }
                let input = kernel.host_value(request.input.value).unwrap();
                match node {
                    0 => assert!(input.is_empty()),
                    1 => assert_eq!(input, &join[..join_len]),
                    2 => assert_eq!(input, &attachment[..attachment_len]),
                    _ => panic!("foreign node"),
                }
                if source_mode == 4 && failed_node == Some(request.node.0) {
                    kernel.cancel().unwrap();
                    assert!(kernel.can_retire());
                    finished = true;
                    break;
                }
                let output = if failed_node == Some(request.node.0)
                    || node == 2
                    || (node == 0 && matches!(source_mode, 1 | 2))
                {
                    None
                } else {
                    let bytes = if node == 0 {
                        &join[..join_len]
                    } else {
                        &attachment[..attachment_len]
                    };
                    Some(
                        k::BoundedValueRef::new(kernel.store_host_value(bytes).unwrap(), 167)
                            .unwrap(),
                    )
                };
                if node < 2 {
                    emitted[node] = output.map(|value| value.value);
                }
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: if node == 0 && source_mode == 2 {
                                k::HostCallDisposition::Cancelled
                            } else if failed_node == Some(request.node.0) {
                                k::HostCallDisposition::Denied
                            } else {
                                k::HostCallDisposition::Completed
                            },
                            output,
                            failure: (failed_node == Some(request.node.0)).then_some(failure),
                        },
                    )
                    .unwrap();
            }
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert!(failed_node.is_none() && matches!(source_mode, 0 | 3));
                    finished = true;
                    break;
                }
                Err(SchedulerError::BackFailed(error)) => {
                    if matches!(source_mode, 0 | 3) {
                        assert!(failed_node.is_some());
                        assert_eq!(error, failure);
                    } else {
                        assert_eq!(
                            error.code,
                            if source_mode == 1 {
                                k::FailureCode::InvalidLifecycle
                            } else {
                                k::FailureCode::Cancelled
                            }
                        );
                    }
                    finished = true;
                    break;
                }
                Ok(_) => {}
                Err(error) => panic!("{error:?}"),
            }
        }
    });
    assert!(finished);
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    if source_mode == 4 {
        return;
    }
    let failed_node = if matches!(source_mode, 0 | 3) {
        failed_node
    } else {
        Some(0)
    };
    for (node, count) in calls.iter().enumerate() {
        assert_eq!(
            *count,
            usize::from(failed_node.is_none_or(|last| node <= usize::from(last)))
        );
    }
}
