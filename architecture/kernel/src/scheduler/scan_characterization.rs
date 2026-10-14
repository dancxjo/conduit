//! Test-only observations of existing traversals; absent from product builds.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ScanCounts {
    pub next_ready_nodes: u64,
    pub ready_bytes: u64,
    pub pending_slots: u64,
    pub context_pending_slots: u64,
    pub input_ports: u64,
    pub cord_scans: u64,
    pub queue_peeks: u64,
}

extern crate std;
use super::*;
use crate::{FixedSignLog, FixedValueStore};
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
fn specimen(active: usize, sparse: bool, trial: usize, print: bool) {
    let prepare = Instant::now();
    let mut routes = FixedRoutes::<32, 1>::new(1);
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
        expected: i as u8 + 1,
        received: 0,
        staged: false,
    });
    let signs = FixedSignLog::<1024>::new_with_remote_storage(
        (1024 * core::mem::size_of::<crate::KernelEvent>()) as u32,
        1024,
        crate::remote_sign_storage_bytes(1024).unwrap(),
    )
    .unwrap();
    let mut scheduler =
        FixedScheduler::<_, _, _, 32, 32, 1, 32, 32, 1, 0, 1>::new_with_active_counts(
            active,
            active,
            specs,
            cords,
            routes,
            drivers,
            FixedValueStore::<32, 1>::new(32).unwrap(),
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
    for node in 0..active {
        if !sparse || node == active - 1 {
            assert_eq!(
                scheduler
                    .admit_remote_input(
                        RemoteEndpointId(node as u16),
                        CordId(node as u16),
                        0,
                        &[node as u8 + 1]
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
                        &[node as u8 + 1]
                    )
                    .unwrap(),
                RemoteIngressOutcome::Full { sequence: 1 }
            );
        }
    }
    scheduler.scan_counts.set(ScanCounts::default());
    let before_signs = scheduler.signs.len();
    let before_bytes = scheduler.signs.used_bytes();
    let start = Instant::now();
    let mut steps = 0;
    let mut order = [u16::MAX; 64];
    loop {
        match scheduler.step().unwrap() {
            SchedulerStatus::Progress { node } => {
                order[steps] = node.0;
                steps += 1;
                assert!(steps <= 64);
            }
            SchedulerStatus::Idle => break,
            other => panic!("unexpected measured scheduler status {other:?}"),
        }
    }
    let step_ns = start.elapsed().as_nanos();
    let counts = scheduler.scan_counts.get();
    let ready = if sparse { 1 } else { active };
    assert_eq!(steps, 2 * ready);
    assert_eq!(counts.next_ready_nodes, steps as u64);
    assert!(counts.next_ready_nodes < 3 * active as u64);
    if sparse && active >= 8 {
        assert!(counts.ready_bytes + counts.next_ready_nodes < 3 * active as u64);
    }
    assert_eq!(counts.pending_slots, counts.next_ready_nodes);
    assert_eq!(counts.context_pending_slots, steps as u64);
    assert_eq!(counts.input_ports, steps as u64);
    assert!(counts.queue_peeks >= steps as u64);
    assert!(counts.cord_scans >= active as u64);
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
        std::println!("SCHEDULER_SCAN_ROW {{\"active_nodes\":{active},\"active_cords\":{active},\"ready_inputs\":{ready},\"sparse\":{sparse},\"trial\":{trial},\"prepare_ns\":{prepare_ns},\"step_ns\":{step_ns},\"steps\":{steps},\"ready_nodes_scanned\":{},\"ready_bytes_scanned\":{},\"pending_slots_scanned\":{},\"signs\":{},\"sign_bytes\":{},\"context_pending_slots\":{},\"input_ports_checked\":{},\"cords_scanned\":{},\"queue_peeks\":{},\"queue_pressure_refused\":true}}",counts.next_ready_nodes,counts.ready_bytes,counts.pending_slots,scheduler.signs.len()-before_signs,scheduler.signs.used_bytes()-before_bytes,counts.context_pending_slots,counts.input_ports,counts.cord_scans,counts.queue_peeks);
    }
}
#[test]
fn loaded_and_sparse_scan_counts_preserve_fairness_and_pressure() {
    for active in [1, 8, 32] {
        for sparse in [false, true] {
            specimen(active, sparse, 0, false);
        }
    }
}
#[test]
#[ignore = "manual optimized instrumented kernel scan baseline; no timing threshold"]
fn characterize_loaded_and_sparse_fixed_scheduler_scans() {
    for trial in 0..8 {
        for active in [1, 8, 32] {
            for sparse in [false, true] {
                specimen(active, sparse, trial, true);
            }
        }
    }
}
