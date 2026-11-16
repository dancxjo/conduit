//! Zero routed-abnormal stock preserves full semantic terminal truth.
use super::phase_terminal_tests::{prepare_spontaneous, Spontaneous};
use super::*;

fn small() -> Box<Kernel<0, 0, 0>> {
    prepare_unresolved_profile::<0, 0, 0>(true, usize::MAX, &CONTRACTS)
        .unwrap()
        .bind([Back {
            first_closed: false,
            changed_law: false,
            finalizes: false,
        }])
}
#[test]
fn zero_unresolved_stock_preserves_normal_abnormal_and_cancel_execution() {
    let mut full = bound::<0>(true);
    let mut zero = small();
    let allocation = AllocationSample::begin();
    full.close_and_drain();
    zero.close_and_drain();
    let cost = allocation.finish();
    assert_eq!(cost.allocations + cost.reallocations, 0);
    assert_eq!(
        full.signs.events().collect::<std::vec::Vec<_>>(),
        zero.signs.events().collect::<std::vec::Vec<_>>()
    );
    let mut full = bound::<0>(true);
    let mut zero = small();
    let terminal = CanonicalValue::new(&[7; 100]).unwrap();
    full.close_remote_input_abnormal(RemoteEndpointId(1), CordId(1), terminal)
        .unwrap();
    zero.close_remote_input_abnormal(RemoteEndpointId(1), CordId(1), terminal)
        .unwrap();
    assert_eq!(full.step(), zero.step());
    assert_eq!(zero.cords[1].abnormal_terminal, Some(terminal));
    let mut full = bound::<0>(true);
    let mut zero = small();
    full.cancel().unwrap();
    zero.cancel().unwrap();
    assert_eq!(full.step(), zero.step());
}
#[test]
fn zero_unresolved_stock_preserves_full_unrouted_spontaneous_error() {
    let mut full = prepare_spontaneous::<100, 1>(false)
        .unwrap()
        .bind([Spontaneous]);
    let mut zero = prepare_spontaneous::<0, 0>(false)
        .unwrap()
        .bind([Spontaneous]);
    let expected = Err(SchedulerError::SemanticAbnormal {
        node: NodeId(0),
        port: PortId(0),
        terminal: CanonicalValue::new(&[9; 100]).unwrap(),
    });
    let allocation = AllocationSample::begin();
    assert_eq!(full.step(), expected);
    assert_eq!(zero.step(), expected);
    let cost = allocation.finish();
    assert_eq!(cost.allocations + cost.reallocations, 0);
}
#[test]
fn zero_unresolved_stock_refuses_routed_abnormal_topology_before_binding() {
    assert_eq!(
        prepare_spontaneous::<0, 0>(true).err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
    assert!(prepare_spontaneous::<0, 1>(true).is_ok());
    assert_eq!(
        prepare_unresolved_profile::<0, 0, 2>(true, usize::MAX, &CONTRACTS).err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
}
#[test]
fn zero_unresolved_stock_is_real_budgeted_storage() {
    let exact = size_of::<Kernel<0, 0, 0>>();
    assert!(exact < size_of::<Kernel<0, 0, 1>>());
    assert_eq!(
        prepare_unresolved_profile::<0, 0, 0>(true, exact - 1, &CONTRACTS).err(),
        Some(KernelPreparationError::Capacity {
            required: exact,
            available: exact - 1
        })
    );
    assert!(prepare_unresolved_profile::<0, 0, 0>(true, exact, &CONTRACTS).is_ok());
}

#[test]
fn zero_unresolved_table_writer_refusal_drops_owned_stores_once() {
    type Refused =
        FixedScheduler<Back, DroppedValues, FixedSignLog<64>, 1, 2, 2, 2, 1, 1, 0, 1, 0, 0, 0>;
    let drops = Rc::new(Cell::new(0));
    let calls = Cell::new(0);
    let values = DroppedValues {
        inner: FixedValueStore::new(16).unwrap(),
        drops: drops.clone(),
    };
    let result = Refused::prepare_boxed_with_linked_table_writer(
        values,
        signs(),
        usize::MAX,
        &CONTRACTS,
        &CONTRACTS,
        |e| e,
        |nodes, cords, routes, bindings| {
            calls.set(calls.get() + 1);
            let active = write(nodes, cords, routes, bindings)?;
            cords[0] = cords[0].with_track(AssignedConnectionTrack::AbnormalTerminal);
            Ok(active)
        },
    );
    assert_eq!(
        result.err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
    assert_eq!(calls.get(), 1); // Topology is evidence produced by this pure writer.
    assert_eq!(drops.get(), 1);
}
#[test]
fn zero_unresolved_known_topology_refuses_before_allocating() {
    let mut nodes = [NodeSpec {
        input_cords: [None; 2],
        maximum_step_fuel: 0,
    }];
    let mut cords = [CordSpec::remote_ingress(
        CordId(0),
        RemoteEndpointId(0),
        (NodeId(0), PortId(0)),
        CordCapacity {
            slot_start: 0,
            item_capacity: 1,
            byte_capacity: 4,
            pressure_policy: Default::default(),
        },
    ); 2];
    let mut routes = FixedRoutes::new(1);
    let mut calls = FixedHostCallBindings::new(1);
    write(&mut nodes, &mut cords, &mut routes, &mut calls).unwrap();
    cords[0] = cords[0].with_track(AssignedConnectionTrack::AbnormalTerminal);
    let values = FixedValueStore::new(16).unwrap();
    let log = signs();
    let allocation = AllocationSample::begin();
    let result = Kernel::<0, 0, 0>::prepare_boxed(
        1,
        2,
        nodes,
        cords,
        routes,
        calls,
        values,
        log,
        usize::MAX,
    );
    let cost = allocation.finish();
    assert_eq!(
        result.err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
    assert_eq!(cost.allocations + cost.reallocations, 0);
}
