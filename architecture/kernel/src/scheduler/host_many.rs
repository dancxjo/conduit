//! Many live Host calls, cancellation batches, reverse completion and rearming.
use super::*;
use crate::{HostCallDisposition, HostCallId, HostCallOutcome, RequestId};
const N: usize = 2048;

#[derive(Clone, Copy)]
struct ManyBack {
    input: ValueRef,
    phase: u8,
    output: bool,
    cancelled: bool,
}
impl StepBack<1> for ManyBack {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        if self.phase == 0 {
            io.request_host_call(
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(self.input, 0).unwrap(),
            )
            .unwrap();
            self.phase = 1;
            return StepOutcome::Progress;
        }
        let round = u32::from((self.phase - 1) / 2);
        if self.phase == 1 || self.phase == 3 {
            let Some(_) = io.input(PortId(0)) else {
                return StepOutcome::Await;
            };
            assert_eq!(bytes.input(PortId(0)), Some([1].as_slice()));
            io.consume(PortId(0)).unwrap();
            io.cancel_host_call(RequestId(round)).unwrap();
            self.phase += 1;
            return StepOutcome::Progress;
        }
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        assert_eq!(request, RequestId(round));
        assert_eq!(
            outcome.disposition,
            if self.output {
                HostCallDisposition::Completed
            } else {
                HostCallDisposition::Cancelled
            }
        );
        assert_eq!(bytes.host_output(), self.output.then_some([7].as_slice()));
        io.consume_host_completion().unwrap();
        self.phase += 1;
        if round == 0 {
            io.request_host_call(
                RequestId(1),
                HostCallId(0),
                BoundedValueRef::new(self.input, 0).unwrap(),
            )
            .unwrap();
            StepOutcome::Progress
        } else {
            StepOutcome::Complete
        }
    }
    fn accepts_input_while_host_call_pending(&self) -> bool {
        true
    }
    fn cancel(&mut self) {
        self.cancelled = true;
    }
}

#[test]
fn thousands_of_host_cancellations_complete_rearm_and_cancel_without_prefix_rescans() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            run(false);
            run(true);
        })
        .unwrap()
        .join()
        .unwrap();
}

fn run(cancel_all: bool) {
    let mut values = ObservedValues::<2050>::new();
    let input = values.store(&[]).unwrap();
    let mut routes = FixedRoutes::<N, 0>::new(1);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<N>::new(1);
    for node in 0..N {
        bindings
            .install(
                NodeId(node as u16),
                HostCallBinding {
                    call: HostCallId(0),
                    maximum_input_bytes: 0,
                    maximum_output_bytes: 1,
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
    let signs = FixedSignLog::<32768>::new_with_remote_storage(
        32768 * core::mem::size_of::<crate::KernelEvent>() as u32,
        4096,
        crate::remote_sign_storage_bytes(4096).unwrap(),
    )
    .unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, N, N, 1, N, N, 0, N, 4096>::new_with_host_calls(
        nodes,
        cords,
        routes,
        bindings,
        core::array::from_fn(|node| ManyBack {
            input,
            phase: 0,
            output: node % 2 == 0,
            cancelled: false,
        }),
        values,
        signs,
    )
    .unwrap();
    let allocation = AllocationSample::begin();
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
    assert_eq!(
        scheduler.scan_counts.get().allocation_pending_slots,
        N as u64
    );
    assert_eq!(scheduler.pending_host_call_count(), N);
    assert!(!scheduler.has_ready_work());
    let mut cancellation_probes = [0; 2];
    for round in 0..2 {
        if round == 1 {
            // Request zero was real but has been consumed and rearmed as one.
            let stale = HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            };
            assert_eq!(
                scheduler.complete_host_call(NodeId(0), RequestId(0), stale),
                Err(SchedulerError::HostCallCompletionRejected)
            );
            assert!(!scheduler.ready.contains(0));
            assert_eq!(scheduler.pending_host_call_count(), N);
        }
        for node in 0..N {
            assert_eq!(
                scheduler.admit_remote_input(
                    RemoteEndpointId(node as u16),
                    CordId(node as u16),
                    round,
                    &[1]
                ),
                Ok(RemoteIngressOutcome::Accepted { sequence: round })
            );
        }
        for node in 0..N {
            assert_eq!(
                scheduler.step(),
                Ok(SchedulerStatus::Progress {
                    node: NodeId(node as u16)
                })
            );
        }
        assert_eq!(scheduler.undispatched_cancellation_count, N as u32);
        scheduler.scan_counts.set(Default::default());
        for node in 0..N {
            let cancellation = scheduler.next_host_cancellation().unwrap();
            assert_eq!(
                (cancellation.node, cancellation.request),
                (NodeId(node as u16), RequestId(round as u32))
            );
        }
        assert!(scheduler.next_host_cancellation().is_none());
        cancellation_probes[round as usize] =
            scheduler.scan_counts.get().cancellation_pending_slots;
        assert_eq!(cancellation_probes[round as usize], N as u64);
        let output = scheduler.store_host_value(&[7]).unwrap();
        for _ in 1..N / 2 {
            scheduler.values.retain(output).unwrap();
        }
        for node in (0..N).rev() {
            let outcome = HostCallOutcome {
                disposition: if node % 2 == 0 {
                    HostCallDisposition::Completed
                } else {
                    HostCallDisposition::Cancelled
                },
                output: (node % 2 == 0).then_some(BoundedValueRef::new(output, 1).unwrap()),
                failure: None,
            };
            scheduler
                .complete_host_call(NodeId(node as u16), RequestId(round as u32), outcome)
                .unwrap();
            assert!(scheduler.ready.contains(node));
            if node > 0 {
                assert!(!scheduler.ready.contains(node - 1));
            }
        }
        let stale = HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: None,
            failure: None,
        };
        assert_eq!(
            scheduler.complete_host_call(NodeId(0), RequestId(round as u32 + 99), stale),
            Err(SchedulerError::HostCallCompletionRejected)
        );
        assert_eq!(
            scheduler.complete_host_call(NodeId(0), RequestId(round as u32), stale),
            Err(SchedulerError::HostCallCompletionRejected)
        );
        assert_eq!(
            scheduler.scan_counts.get().completion_pending_slots,
            N as u64 + 2
        );
        assert!(scheduler.has_retained_host_call_for_nodes(&[NodeId(0), NodeId((N - 1) as u16)]));
        if cancel_all {
            scheduler.cancel().unwrap();
            break;
        }
        for node in 0..N {
            let expected = if round == 1 && node == N - 1 {
                SchedulerStatus::Drained
            } else {
                SchedulerStatus::Progress {
                    node: NodeId(node as u16),
                }
            };
            assert_eq!(scheduler.step(), Ok(expected));
            if round == 0 {
                let request = scheduler.next_host_request().unwrap();
                assert_eq!(
                    (request.node, request.request),
                    (NodeId(node as u16), RequestId(1))
                );
            }
        }
        assert_eq!(
            scheduler.scan_counts.get().preflight_completion_slots,
            N as u64
        );
        assert_eq!(
            scheduler.scan_counts.get().consumed_completion_slots,
            N as u64
        );
        assert_eq!(
            scheduler.pending_host_call_count(),
            if round == 0 { N } else { 0 }
        );
    }
    if cancel_all {
        assert_eq!(scheduler.step(), Ok(SchedulerStatus::Cancelled));
        assert!(scheduler.drivers.iter().all(|back| back.cancelled));
        assert!(scheduler.next_host_request().is_none());
        assert!(scheduler.next_host_cancellation().is_none());
    } else {
        assert_eq!(scheduler.step(), Ok(SchedulerStatus::Drained));
        assert!(scheduler.drivers.iter().all(|back| back.phase == 5));
        scheduler.discard_host_value(input).unwrap();
    }
    assert_eq!(scheduler.pending_host_call_count(), 0);
    assert!(scheduler.pending_host_slots.iter().all(Option::is_none));
    assert_eq!(scheduler.values.used_items(), 0);
    assert_eq!(scheduler.values.used_bytes(), 0);
    assert!(!scheduler.has_ready_work());
    assert!(scheduler.signs.retention_gap.is_none());
    let rounds = if cancel_all { 1 } else { 2 };
    let (mut requested, mut cancelled, mut completed) = (0, 0, 0);
    for (sequence, event) in scheduler.signs.events().enumerate() {
        assert_eq!(event.sequence as usize, sequence);
        let index = match event.kind {
            KernelEventKind::HostCallRequested => {
                let index = requested;
                requested += 1;
                Some((index % N, index / N))
            }
            KernelEventKind::HostCallCancellationRequested => {
                let index = cancelled;
                cancelled += 1;
                Some((index % N, index / N))
            }
            KernelEventKind::HostCallCompleted => {
                let index = completed;
                completed += 1;
                Some((N - 1 - index % N, index / N))
            }
            _ => None,
        };
        if let Some((node, request)) = index {
            assert_eq!(
                (event.node, event.request),
                (NodeId(node as u16), Some(RequestId(request as u32)))
            );
        }
    }
    assert_eq!(
        (requested, cancelled, completed),
        (rounds * N, rounds * N, rounds * N)
    );
    for (index, remote) in scheduler.signs.remote_entries.iter().flatten().enumerate() {
        let node = index % N;
        assert_eq!(remote.identity.endpoint, RemoteEndpointId(node as u16));
        assert_eq!(remote.identity.cord, CordId(node as u16));
        assert_eq!(remote.identity.sequence, (index / N) as u64);
        assert_eq!(
            remote.identity.direction,
            crate::RemoteCordDirection::Ingress
        );
        let event = scheduler.signs.entries[remote.event_sequence as usize].unwrap();
        assert_eq!(
            (event.node, event.kind),
            (NodeId(node as u16), KernelEventKind::RemoteInputAdmitted)
        );
    }
    assert_eq!(
        scheduler.signs.remote_entries.iter().flatten().count(),
        rounds * N
    );
    assert_eq!(
        scheduler.signs.events().count(),
        if cancel_all { 14_338 } else { 32_768 }
    );
    let allocations = allocation.finish();
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
    std::println!("MANY_HOST_ROW {{\"nodes\":{N},\"cancel_all\":{cancel_all},\"cancellation_probes\":{cancellation_probes:?},\"allocations\":{},\"signs\":{},\"value_items\":0}}", allocations.allocations, scheduler.signs.events().count());
}
