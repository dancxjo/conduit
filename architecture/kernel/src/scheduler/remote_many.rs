//! Large production fan-out admission, refusal, closure, wake and ownership.
use super::*;
#[derive(Clone, Copy)]
struct Waiting;
impl StepBack<1> for Waiting {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Await
    }
}
#[test]
fn ten_thousand_remote_targets_preserve_atomic_admission_and_terminal_lineage() {
    std::thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
fn run() {
    const N: usize = 10_000;
    let mut routes = FixedRoutes::<N, 0>::new(1);
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, N, N, 1, N, N, 0>::new(
        core::array::from_fn(|i| NodeSpec {
            input_cords: [Some(CordId(i as u16))],
            maximum_step_fuel: 8,
        }),
        core::array::from_fn(|i| {
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
        }),
        routes,
        [Waiting; N],
        ObservedValues::<2>::new(),
        FixedSignLog::<32768>::new_with_remote_storage(
            32768 * core::mem::size_of::<crate::KernelEvent>() as u32,
            20_000,
            crate::remote_sign_storage_bytes(20_000).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    for i in 0..N {
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress {
                node: NodeId(i as u16)
            })
        );
    }
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Idle));
    assert!(!scheduler.has_ready_work());
    let mut targets: [(RemoteEndpointId, CordId); N] =
        core::array::from_fn(|i| (RemoteEndpointId(i as u16), CordId(i as u16)));
    let allocation = AllocationSample::begin();
    assert_eq!(
        scheduler.admit_remote_input_fanout(&targets, 0, &[7]),
        Ok(RemoteIngressOutcome::Accepted { sequence: 0 })
    );
    let value = scheduler.peek(0).unwrap().unwrap();
    assert_eq!(scheduler.values.reference_count(value), Ok(N as u16));
    assert_eq!(scheduler.values.used_items(), 1);
    let signs = scheduler.signs.events().count();
    assert_eq!(
        scheduler.admit_remote_input_fanout(&targets, 1, &[8]),
        Ok(RemoteIngressOutcome::Full { sequence: 1 })
    );
    // Late invalid identity and duplicate must outrank earlier retryable pressure.
    targets[N - 1].0 = RemoteEndpointId(0);
    assert_eq!(
        scheduler.admit_remote_input_fanout(&targets, 1, &[8]),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
    assert_eq!(
        scheduler.close_remote_input_fanout(&targets),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
    targets[N - 1] = targets[0];
    assert_eq!(
        scheduler.admit_remote_input_fanout(&targets, 1, &[8]),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
    assert_eq!(
        scheduler.close_remote_input_fanout(&targets),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
    targets[N - 1] = (RemoteEndpointId((N - 1) as u16), CordId((N - 1) as u16));
    assert_eq!(scheduler.signs.events().count(), signs);
    for i in 0..N {
        assert_eq!(scheduler.peek(i).unwrap(), Some(value));
        assert_eq!(scheduler.cords[i].len, 1);
        assert_eq!(scheduler.cords[i].next_remote_sequence, 1);
        assert!(scheduler.ready.contains(i));
        assert!(!scheduler.cords[i].producer_closed);
    }
    scheduler.close_remote_input_fanout(&targets).unwrap();
    let closed_signs = scheduler.signs.events().count();
    scheduler.close_remote_input_fanout(&targets).unwrap();
    assert_eq!(scheduler.signs.events().count(), closed_signs);
    assert_eq!(
        scheduler
            .close_remote_input_fanout_abnormal(&targets, CanonicalValue::new(b"lost").unwrap()),
        Err(SchedulerError::RemoteDeliveryRejected)
    );
    for i in 0..N {
        assert!(scheduler.cords[i].producer_closed);
        assert_eq!(scheduler.peek(i).unwrap(), Some(value));
    }
    assert_eq!(scheduler.values.reference_count(value), Ok(N as u16));
    assert_eq!(scheduler.signs.remote_len(), 20_000);
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
        assert_eq!(event.node, NodeId(node as u16));
        assert_eq!(
            event.kind,
            if index < N {
                KernelEventKind::RemoteInputAdmitted
            } else {
                KernelEventKind::RemoteInputClosed
            }
        );
    }
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.values.used_items(), 0);
    assert!(!scheduler.has_ready_work());
    assert!(scheduler.signs.retention_gap.is_none());
    for (sequence, event) in scheduler.signs.events().enumerate() {
        assert_eq!(event.sequence, sequence as u32);
    }
    let access = scheduler.values.access();
    assert_eq!((access.successful_stores, access.stored_bytes), (1, 1));
    let allocations = allocation.finish();
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
    std::println!("REMOTE_MANY_ROW {{\"targets\":{N},\"remote_signs\":20000,\"stored_bytes\":1,\"final_values\":0,\"allocations\":0,\"ordinary_signs\":{}}}", scheduler.signs.events().count());
}
