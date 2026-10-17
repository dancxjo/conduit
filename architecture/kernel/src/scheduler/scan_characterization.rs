//! Test-only observations of existing traversals; absent from product builds.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ScanCounts {
    pub next_ready_nodes: u64,
    pub ready_bytes: u64,
    pub pending_slots: u64,
    pub context_pending_slots: u64,
    pub completion_pending_slots: u64,
    pub preflight_completion_slots: u64,
    pub consumed_completion_slots: u64,
    pub retained_pending_slots: u64,
    pub allocation_pending_slots: u64,
    pub dispatch_pending_slots: u64,
    pub cancellation_pending_slots: u64,
    pub value_owner_pending_slots: u64,
    pub value_owner_queue_slots: u64,
    pub input_ports: u64,
    pub cord_scans: u64,
    pub queue_peeks: u64,
}

extern crate std;
use super::*;
use crate::FixedSignLog;
#[path = "overhead_observation.rs"]
mod overhead_observation;
use overhead_observation::{AllocationSample, ObservedValues};
use std::time::Instant;

#[derive(Clone, Copy)]
struct Sink {
    expected: u8,
    received: u32,
    staged: bool,
}
impl StepBack<1> for Sink {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            assert_eq!(bytes.input(PortId(0)), Some([self.expected].as_slice()));
            io.consume(PortId(0)).unwrap();
            self.staged = true;
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.received += 1;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
    }
}
fn specimen<const N: usize>(active: usize, sparse: bool, trial: usize, print: bool) {
    let prepare = Instant::now();
    let mut routes = FixedRoutes::<N, 1>::new(1);
    routes.seal().unwrap();
    let specs = core::array::from_fn(|i| NodeSpec {
        input_cords: [if i < active {
            Some(CordId(i as u16))
        } else {
            None
        }],
        maximum_step_fuel: 8,
    });
    let cords = core::array::from_fn(|i| {
        if i < active {
            CordSpec::remote_ingress(
                CordId(i as u16),
                RemoteEndpointId(i as u16),
                (NodeId(i as u16), PortId(0)),
                CordCapacity {
                    slot_start: i as u16,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            )
        } else {
            CordSpec::inactive()
        }
    });
    let drivers = core::array::from_fn(|i| Sink {
        expected: (i % 251) as u8 + 1,
        received: 0,
        staged: false,
    });
    let signs = FixedSignLog::<4096>::new_with_remote_storage(
        (4096 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        4096,
        crate::remote_sign_storage_bytes(4096).unwrap(),
    )
    .unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, N, N, 1, N, N, 1, 0, 1>::new_with_active_counts(
        active,
        active,
        specs,
        cords,
        routes,
        drivers,
        ObservedValues::<N>::new(),
        signs,
    )
    .unwrap();
    let prepare_ns = prepare.elapsed().as_nanos();
    // Warm startup through real Await steps; no ready flag is changed directly.
    for _ in 0..active {
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
    }
    assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Idle);
    let ingress_sample = AllocationSample::begin();
    for node in 0..active {
        if !sparse || node == active - 1 {
            assert_eq!(
                scheduler
                    .admit_remote_input(
                        RemoteEndpointId(node as u16),
                        CordId(node as u16),
                        0,
                        &[(node % 251) as u8 + 1]
                    )
                    .unwrap(),
                RemoteIngressOutcome::Accepted { sequence: 0 }
            );
            assert_eq!(
                scheduler
                    .admit_remote_input(
                        RemoteEndpointId(node as u16),
                        CordId(node as u16),
                        1,
                        &[(node % 251) as u8 + 1]
                    )
                    .unwrap(),
                RemoteIngressOutcome::Full { sequence: 1 }
            );
        }
    }
    let ingress_allocations = ingress_sample.finish();
    let ingress_values = scheduler.values.access();
    scheduler.scan_counts.set(ScanCounts::default());
    let before_signs = scheduler.signs.len();
    let before_bytes = scheduler.signs.used_bytes();
    let step_sample = AllocationSample::begin();
    let start = Instant::now();
    let mut steps = 0;
    let mut order = [u16::MAX; N];
    loop {
        match scheduler.step().unwrap() {
            SchedulerStatus::Progress { node } => {
                if steps < active {
                    order[steps] = node.0;
                }
                steps += 1;
                assert!(steps <= 2 * active);
            }
            SchedulerStatus::Idle => break,
            other => panic!("unexpected measured scheduler status {other:?}"),
        }
    }
    let step_ns = start.elapsed().as_nanos();
    let step_allocations = step_sample.finish();
    let step_values = scheduler.values.access();
    assert_eq!(
        ingress_allocations.allocations + ingress_allocations.reallocations,
        0
    );
    assert_eq!(
        step_allocations.allocations + step_allocations.reallocations,
        0
    );
    assert_eq!(step_values.stored_bytes, ingress_values.stored_bytes);
    let counts = scheduler.scan_counts.get();
    let ready = if sparse { 1 } else { active };
    assert_eq!(steps, 2 * ready);
    if cfg!(feature = "linear-ready-reference-test") {
        assert_eq!(counts.next_ready_nodes, 3 * active as u64);
        assert_eq!(counts.ready_bytes, 0);
    } else {
        assert_eq!(counts.next_ready_nodes, steps as u64);
        assert!(counts.next_ready_nodes < 3 * active as u64);
        if sparse && active >= 8 {
            assert!(counts.ready_bytes + counts.next_ready_nodes < 3 * active as u64);
        }
    }
    assert_eq!(counts.pending_slots, 0);
    assert_eq!(counts.context_pending_slots, 0);
    assert_eq!(counts.input_ports, steps as u64);
    assert!(counts.queue_peeks >= steps as u64);
    assert_eq!(
        counts.cord_scans, 0,
        "payload-only graphs need no quiescence probes"
    );
    for node in 0..active {
        assert_eq!(
            scheduler.drivers[node].received,
            u32::from(!sparse || node == active - 1)
        );
    }
    if !sparse {
        for (node, selected) in order[..active].iter().enumerate() {
            assert_eq!(*selected, node as u16);
        }
    }
    assert_eq!(scheduler.values.used_items(), 0);
    assert_eq!(scheduler.values.used_bytes(), 0);
    if print {
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
        let received = (0..active)
            .map(|n| scheduler.drivers[n].received)
            .collect::<std::vec::Vec<_>>();
        let readiness = (0..active)
            .map(|n| scheduler.ready.contains(n))
            .collect::<std::vec::Vec<_>>();
        std::println!("SCHEDULER_REPLAY_ROW {{\"capacity\":{N},\"active\":{active},\"sparse\":{sparse},\"trial\":{trial},\"legacy\":{},\"kernel_bytes\":{},\"events\":{events:?},\"remote\":{remote:?},\"received\":{received:?},\"readiness\":{readiness:?},\"value_items\":{},\"value_bytes\":{},\"terminal\":\"Idle\"}}", cfg!(feature = "linear-ready-reference-test"), core::mem::size_of_val(&scheduler), scheduler.values.used_items(), scheduler.values.used_bytes());
        std::println!("SCHEDULER_OVERHEAD_ROW {{\"active_nodes\":{active},\"sparse\":{sparse},\"trial\":{trial},\"ingress_allocations\":{},\"ingress_reallocations\":{},\"step_allocations\":{},\"step_reallocations\":{},\"ingress_store_calls\":{},\"ingress_value_store_copy_bytes\":{},\"step_value_store_copy_bytes\":{},\"step_borrowed_value_reads\":{},\"copy_scope\":\"successful fixed-value-store writes only; compiler moves, Sign and Host transport copies unmeasured\"}}", ingress_allocations.allocations, ingress_allocations.reallocations, step_allocations.allocations, step_allocations.reallocations, ingress_values.successful_stores, ingress_values.stored_bytes, step_values.stored_bytes - ingress_values.stored_bytes, step_values.borrowed_reads - ingress_values.borrowed_reads);
        std::println!("SCHEDULER_SCAN_ROW {{\"active_nodes\":{active},\"active_cords\":{active},\"ready_inputs\":{ready},\"sparse\":{sparse},\"trial\":{trial},\"prepare_ns\":{prepare_ns},\"step_ns\":{step_ns},\"steps\":{steps},\"ready_nodes_scanned\":{},\"ready_bytes_scanned\":{},\"pending_slots_scanned\":{},\"signs\":{},\"sign_bytes\":{},\"context_pending_slots\":{},\"input_ports_checked\":{},\"cords_scanned\":{},\"queue_peeks\":{},\"queue_pressure_refused\":true}}",counts.next_ready_nodes,counts.ready_bytes,counts.pending_slots,scheduler.signs.len()-before_signs,scheduler.signs.used_bytes()-before_bytes,counts.context_pending_slots,counts.input_ports,counts.cord_scans,counts.queue_peeks);
    }
}
#[test]
fn loaded_and_sparse_scan_counts_preserve_fairness_and_pressure() {
    for active in [1, 8, 32] {
        for sparse in [false, true] {
            specimen::<32>(active, sparse, 0, false);
        }
    }
}
#[test]
#[ignore = "manual optimized instrumented kernel scan baseline; no timing threshold"]
fn characterize_loaded_and_sparse_fixed_scheduler_scans() {
    for trial in 0..8 {
        for active in [1, 8, 32] {
            for sparse in [false, true] {
                match active {
                    1 => specimen::<1>(active, sparse, trial, true),
                    8 => specimen::<8>(active, sparse, trial, true),
                    32 => specimen::<32>(active, sparse, trial, true),
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[test]
fn large_sparse_and_loaded_pressure_preserve_outputs_and_readiness() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            for sparse in [false, true] {
                specimen::<256>(256, sparse, 0, false);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "manual deterministic replay and timing; diagnostic fixed stack, no SMP claim"]
fn capture_small_and_large_selector_replay() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            for trial in 0..8 {
                for sparse in [false, true] {
                    specimen::<1>(1, sparse, trial, true);
                    specimen::<8>(8, sparse, trial, true);
                    specimen::<32>(32, sparse, trial, true);
                    specimen::<256>(256, sparse, trial, true);
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[path = "sparse_large.rs"]
mod sparse_large;

#[path = "host_many.rs"]
mod host_many;
