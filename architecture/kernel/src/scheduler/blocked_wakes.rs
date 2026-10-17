//! Real input wakes while ordinary Backs are blocked on outstanding Host calls.
use super::*;
use crate::{HostCallDisposition, HostCallId, HostCallOutcome, RequestId};
const N: usize = 256;

#[derive(Clone, Copy, Default)]
struct BlockedBack {
    requests: u32,
    completions: u32,
    invocations: u32,
    cancellations: u32,
}
impl StepBack<1> for BlockedBack {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        self.invocations += 1;
        if let Some((request, _)) = io.host_completion() {
            assert_eq!(request, RequestId(0));
            io.consume_host_completion().unwrap();
            self.completions += 1;
            return StepOutcome::Progress;
        }
        if let Some(input) = io.input(PortId(0)) {
            assert_eq!(
                bytes.input(PortId(0)),
                Some([self.requests as u8].as_slice())
            );
            io.consume(PortId(0)).unwrap();
            io.request_host_call(
                RequestId(self.requests),
                HostCallId(0),
                BoundedValueRef::new(input, 1).unwrap(),
            )
            .unwrap();
            self.requests += 1;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.cancellations += 1;
    }
}

#[test]
fn input_wakes_of_host_blocked_nodes_are_characterized_without_running_backs() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

fn run() {
    let mut routes = FixedRoutes::<N, 0>::new(1);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<N>::new(1);
    for node in 0..N {
        bindings
            .install(
                NodeId(node as u16),
                HostCallBinding {
                    call: HostCallId(0),
                    maximum_input_bytes: 1,
                    maximum_output_bytes: 0,
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    let nodes = core::array::from_fn(|node| NodeSpec {
        input_cords: [Some(CordId(node as u16))],
        maximum_step_fuel: 8,
    });
    let cords = core::array::from_fn(|node| {
        CordSpec::remote_ingress(
            CordId(node as u16),
            RemoteEndpointId(node as u16),
            (NodeId(node as u16), PortId(0)),
            CordCapacity {
                slot_start: node as u16,
                item_capacity: 1,
                byte_capacity: 1,
                pressure_policy: Default::default(),
            },
        )
    });
    let mut scheduler = FixedScheduler::<_, _, _, N, N, 1, N, N, 0, N, N>::new_with_host_calls(
        nodes,
        cords,
        routes,
        bindings,
        [BlockedBack::default(); N],
        ObservedValues::<512>::new(),
        FixedSignLog::<4096>::new_with_remote_storage(
            4096 * core::mem::size_of::<crate::KernelEvent>() as u32,
            1024,
            crate::remote_sign_storage_bytes(1024).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    for _ in 0..N {
        scheduler.step().unwrap();
    }
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Idle));
    let allocation = AllocationSample::begin();
    for node in 0..N {
        assert_eq!(
            scheduler.admit_remote_input(
                RemoteEndpointId(node as u16),
                CordId(node as u16),
                0,
                &[0]
            ),
            Ok(RemoteIngressOutcome::Accepted { sequence: 0 })
        );
    }
    for node in 0..N {
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress {
                node: NodeId(node as u16)
            })
        );
        let request = scheduler.next_host_request().unwrap();
        assert_eq!(
            (request.node, request.request),
            (NodeId(node as u16), RequestId(0))
        );
    }
    assert_eq!(scheduler.pending_host_call_count(), N);
    let wake = Instant::now();
    for node in 0..N {
        assert_eq!(
            scheduler.admit_remote_input(
                RemoteEndpointId(node as u16),
                CordId(node as u16),
                1,
                &[1]
            ),
            Ok(RemoteIngressOutcome::Accepted { sequence: 1 })
        );
        assert_eq!(
            scheduler.admit_remote_input(
                RemoteEndpointId(node as u16),
                CordId(node as u16),
                2,
                &[9]
            ),
            Ok(RemoteIngressOutcome::Full { sequence: 2 })
        );
    }
    let wake_ns = wake.elapsed().as_nanos();
    assert!(!scheduler.has_ready_work());
    assert_eq!(
        (0..N)
            .filter(|node| scheduler.ready.contains(*node))
            .count(),
        N
    );
    scheduler.scan_counts.set(Default::default());
    let idle = Instant::now();
    for _ in 0..8 {
        assert_eq!(scheduler.step(), Ok(SchedulerStatus::Idle));
    }
    let idle_ns = idle.elapsed().as_nanos();
    let counts = scheduler.scan_counts.get();
    assert_eq!(counts.next_ready_nodes, 8 * N as u64);
    assert_eq!(counts.pending_slots, 8 * N as u64);
    assert!(scheduler
        .drivers
        .iter()
        .all(|back| back.invocations == 2 && back.requests == 1));
    scheduler
        .complete_host_call(
            NodeId((N - 1) as u16),
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    assert_eq!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress {
            node: NodeId((N - 1) as u16)
        })
    );
    assert_eq!(scheduler.drivers[N - 1].completions, 1);
    // Completion wakes the exact node; the queued next input must still survive.
    assert_eq!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress {
            node: NodeId((N - 1) as u16)
        })
    );
    let next = scheduler.next_host_request().unwrap();
    assert_eq!(
        (next.node, next.request),
        (NodeId((N - 1) as u16), RequestId(1))
    );
    assert_eq!(scheduler.pending_host_call_count(), N);
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Cancelled));
    assert!(scheduler.drivers.iter().all(|back| back.cancellations == 1));
    assert!(!scheduler.has_ready_work());
    assert_eq!(scheduler.values.used_items(), 0);
    assert_eq!(scheduler.values.used_bytes(), 0);
    let allocations = allocation.finish();
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
    let events = std::format!(
        "{:?}",
        scheduler.signs.events().collect::<std::vec::Vec<_>>()
    );
    let remote = std::format!(
        "{:?}",
        scheduler
            .signs
            .remote_entries
            .iter()
            .flatten()
            .collect::<std::vec::Vec<_>>()
    );
    let drivers = std::format!(
        "{:?}",
        scheduler
            .drivers
            .iter()
            .map(|back| (
                back.requests,
                back.completions,
                back.invocations,
                back.cancellations
            ))
            .collect::<std::vec::Vec<_>>()
    );
    std::println!("BLOCKED_WAKE_REPLAY {{\"events\":{events:?},\"remote\":{remote:?},\"drivers\":{drivers:?},\"terminal\":\"Cancelled\",\"values\":0,\"pending\":0}}");
    std::println!("BLOCKED_WAKE_ROW {{\"nodes\":{N},\"idle_polls\":8,\"wake_ns\":{wake_ns},\"idle_ns\":{idle_ns},\"candidate_nodes\":{},\"pending_probes\":{},\"ready_bytes\":{},\"allocations\":{},\"reallocations\":{}}}", counts.next_ready_nodes, counts.pending_slots, counts.ready_bytes, allocations.allocations, allocations.reallocations);
}
