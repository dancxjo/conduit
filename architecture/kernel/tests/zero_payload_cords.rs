//! Zero-byte canonical values retain finite item pressure and atomic fanout.
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, RemoteIngressOutcome, SchedulerError,
    SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId,
    RemoteEndpointId, ValueRef, ValueStorage,
};

#[derive(Clone, Copy)]
struct Sink {
    consume: bool,
    seen: Option<ValueRef>,
    deliveries: u16,
}
impl StepBack<1> for Sink {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        if !self.consume {
            return StepOutcome::Await;
        }
        if let Some(value) = io.input(PortId(0)) {
            assert_eq!(value.byte_len, 0);
            assert_eq!(bytes.input(PortId(0)).unwrap(), &[]);
            self.seen = Some(value);
            self.deliveries += 1;
            io.consume(PortId(0)).unwrap();
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
}
type Scheduler = FixedScheduler<Sink, FixedValueStore<2, 1>, FixedSignLog<32>, 2, 2, 1, 2, 2, 1>;
const TARGETS: [(RemoteEndpointId, CordId); 2] = [
    (RemoteEndpointId(0), CordId(0)),
    (RemoteEndpointId(1), CordId(1)),
];
fn prepare(byte_capacity: u32, item_capacity: u16) -> Result<Scheduler, SchedulerError> {
    let mut routes = FixedRoutes::<2, 1>::new(1);
    routes.seal().unwrap();
    let cords = core::array::from_fn(|index| {
        CordSpec::remote_ingress(
            CordId(index as u16),
            RemoteEndpointId(index as u16),
            (NodeId(index as u16), PortId(0)),
            CordCapacity {
                slot_start: index as u16,
                item_capacity,
                byte_capacity,
                pressure_policy: Default::default(),
            },
        )
        .with_maximum_value_bytes(0)
    });
    Scheduler::new(
        core::array::from_fn(|index| NodeSpec {
            input_cords: [Some(CordId(index as u16))],
            maximum_step_fuel: 1,
        }),
        cords,
        routes,
        core::array::from_fn(|index| Sink {
            consume: index == 0,
            seen: None,
            deliveries: 0,
        }),
        FixedValueStore::new(2).unwrap(),
        FixedSignLog::new_with_remote_storage(
            (32 * core::mem::size_of::<KernelEvent>()) as u32,
            32,
            conduit_kernel::remote_sign_storage_bytes(32).unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn empty_payload_fanout_is_atomic_under_item_pressure() {
    let mut scheduler = prepare(1, 1).unwrap();
    assert_eq!(
        scheduler.admit_remote_input_fanout(&TARGETS, 0, &[]),
        Ok(RemoteIngressOutcome::Accepted { sequence: 0 })
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (1, 0));
    assert_eq!(scheduler.cord_usage(CordId(1)).unwrap(), (1, 0));
    assert_eq!(scheduler.values().used_items(), 1);
    assert_eq!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress { node: NodeId(0) })
    );
    let original = scheduler.drivers()[0].seen.unwrap();
    assert_eq!(scheduler.values().get(original).unwrap(), &[]);
    assert_eq!(scheduler.values().reference_count(original).unwrap(), 1);
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
    // No byte pressure exists, but the second branch's single item is occupied.
    assert_eq!(
        scheduler.admit_remote_input_fanout(&TARGETS, 1, &[]),
        Ok(RemoteIngressOutcome::Full { sequence: 1 })
    );
    assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
    assert_eq!(scheduler.cord_usage(CordId(1)).unwrap(), (1, 0));
    assert_eq!(scheduler.values().used_items(), 1);
    assert_eq!(scheduler.values().used_bytes(), 0);
    assert_eq!(scheduler.values().item_capacity(), 2);
    assert_eq!(scheduler.values().byte_capacity(), 2);
    assert_eq!(scheduler.values().reference_count(original).unwrap(), 1);
    assert_eq!(scheduler.drivers()[0].deliveries, 1);
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.values().used_items(), 0);
}

#[test]
fn semantic_zero_refuses_nonempty_before_storage_or_queue_mutation() {
    let mut scheduler = prepare(1, 1).unwrap();
    assert_eq!(
        scheduler.admit_remote_input_fanout(&TARGETS, 0, &[1]),
        Err(SchedulerError::SemanticValueBoundExceeded)
    );
    assert_eq!(scheduler.values().used_items(), 0);
    for (_, cord) in TARGETS {
        assert_eq!(scheduler.cord_usage(cord).unwrap(), (0, 0));
    }
    assert_eq!(
        scheduler.admit_remote_input_fanout(&TARGETS, 0, &[]),
        Ok(RemoteIngressOutcome::Accepted { sequence: 0 })
    );
}

#[test]
fn zero_semantic_bound_does_not_admit_zero_physical_capacity() {
    assert!(matches!(prepare(0, 1), Err(SchedulerError::InvalidPlan)));
    assert!(matches!(prepare(1, 0), Err(SchedulerError::InvalidPlan)));
}
