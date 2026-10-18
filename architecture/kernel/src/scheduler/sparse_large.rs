//! Real sparse queue/wake execution at the parent issue's 10,000-node scale.
use super::*;

#[test]
fn two_ready_among_ten_thousand_preserve_fairness_pressure_and_ownership() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

fn run() {
    const N: usize = 10_000;
    let prepare = Instant::now();
    let mut routes = FixedRoutes::<N, 0>::new(1);
    routes.seal().unwrap();
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
    let drivers = core::array::from_fn(|node| Sink {
        expected: (node % 251) as u8 + 1,
        received: 0,
        staged: false,
    });
    // Startup fuel observations may retire with an explicit bounded gap;
    // required lifecycle evidence stays retained.
    let signs = FixedSignLog::<64>::new_with_remote_storage(
        64 * core::mem::size_of::<crate::KernelEvent>() as u32,
        8,
        crate::remote_sign_storage_bytes(8).unwrap(),
    )
    .unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, N, N, 1, N, N, 0, 0, 1>::new(
        nodes,
        cords,
        routes,
        drivers,
        ObservedValues::<4>::new(),
        signs,
    )
    .unwrap();
    let prepare_ns = prepare.elapsed().as_nanos();
    for node in 0..N {
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress {
                node: NodeId(node as u16)
            })
        );
    }
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Idle));
    assert!(scheduler.signs.retention_gap.is_some());
    let allocation = AllocationSample::begin();
    for node in [63, N - 1] {
        let byte = (node % 251) as u8 + 1;
        assert_eq!(
            scheduler.admit_remote_input(
                RemoteEndpointId(node as u16),
                CordId(node as u16),
                0,
                &[byte]
            ),
            Ok(RemoteIngressOutcome::Accepted { sequence: 0 })
        );
        assert_eq!(
            scheduler.admit_remote_input(
                RemoteEndpointId(node as u16),
                CordId(node as u16),
                1,
                &[byte]
            ),
            Ok(RemoteIngressOutcome::Full { sequence: 1 })
        );
    }
    scheduler.scan_counts.set(Default::default());
    let advance = Instant::now();
    for node in [63, N - 1, 63, N - 1] {
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress {
                node: NodeId(node as u16)
            })
        );
    }
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Idle));
    let advance_ns = advance.elapsed().as_nanos();
    let allocations = allocation.finish();
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
    let counts = scheduler.scan_counts.get();
    if cfg!(feature = "linear-ready-reference-test") {
        assert!(counts.next_ready_nodes >= N as u64);
    } else {
        assert_eq!(counts.next_ready_nodes, 4);
        assert!(counts.ready_bytes <= 1_000, "{counts:?}");
    }
    assert_eq!(
        counts.pending_slots + counts.context_pending_slots + counts.cord_scans,
        0
    );
    for node in 0..N {
        assert_eq!(
            scheduler.drivers[node].received,
            u32::from(node == 63 || node == N - 1)
        );
        assert_eq!(scheduler.cords[node].len, 0);
        assert!(!scheduler.ready.contains(node));
    }
    assert_eq!(scheduler.values.used_items(), 0);
    assert_eq!(scheduler.values.used_bytes(), 0);
    let access = scheduler.values.access();
    assert_eq!((access.successful_stores, access.stored_bytes), (2, 2));
    assert_eq!(scheduler.signs.remote_entries.iter().flatten().count(), 2);
    assert_eq!(
        scheduler
            .signs
            .events()
            .filter(|event| event.kind == KernelEventKind::ValueConsumed)
            .count(),
        2
    );
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
    let gap = std::format!("{:?}", scheduler.signs.retention_gap);
    std::println!("SPARSE_LARGE_REPLAY {{\"events\":{events:?},\"remote\":{remote:?},\"gap\":{gap:?},\"received_nodes\":[63,9999],\"value_items\":0,\"value_bytes\":0,\"terminal\":\"Idle\"}}");
    std::println!("SPARSE_LARGE_ROW {{\"nodes\":{N},\"ready\":2,\"prepare_ns\":{prepare_ns},\"advance_ns\":{advance_ns},\"ready_bytes\":{},\"candidate_nodes_scanned\":{},\"allocations\":{},\"reallocations\":{},\"logical_steps\":4,\"kernel_bytes\":{},\"legacy\":{}}}", counts.ready_bytes, counts.next_ready_nodes, allocations.allocations, allocations.reallocations, core::mem::size_of_val(&scheduler), cfg!(feature = "linear-ready-reference-test"));
}
