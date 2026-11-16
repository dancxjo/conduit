//! Stock follows checked write domains, not a reduced abnormal-value envelope.
use super::*;
use std::vec::Vec;

type StockKernel<const T: usize, S = FixedValueStore<2, 8>> =
    FixedScheduler<Back, S, FixedSignLog<64>, 1, 3, 2, 2, 1, 1, 0, 1, 0, 0, 1, T>;
fn tables(
    nodes: &mut [NodeSpec<2>; 1],
    cords: &mut [CordSpec; 3],
    routes: &mut FixedRoutes<1, 1>,
    calls: &mut FixedHostCallBindings<0>,
) -> Result<(usize, usize), KernelPreparationError> {
    write(nodes, (&mut cords[..2]).try_into().unwrap(), routes, calls)
}
fn prepared<const T: usize>(
    maximum: usize,
) -> Result<PreparedKernelStorage<Back, StockKernel<T>, 1>, KernelPreparationError> {
    StockKernel::prepare_boxed_with_linked_table_writer(
        FixedValueStore::new(16).unwrap(),
        signs(),
        maximum,
        &CONTRACTS,
        &CONTRACTS,
        |e| e,
        tables,
    )
}
fn bound<const T: usize>() -> Box<StockKernel<T>> {
    prepared(usize::MAX).unwrap().bind([Back {
        first_closed: false,
        changed_law: false,
        finalizes: false,
    }])
}
#[test]
fn compact_stock_keeps_full_ingress_replay_context_and_signs_without_allocating() {
    let mut full = bound::<3>();
    let mut small = bound::<2>();
    let terminal = CanonicalValue::new(&[17; 100]).unwrap();
    let targets = [
        (RemoteEndpointId(0), CordId(0)),
        (RemoteEndpointId(1), CordId(1)),
    ];
    let allocation = AllocationSample::begin();
    for kernel in [&mut *full as &mut dyn ClosureExercise, &mut *small] {
        kernel.close(&targets, terminal);
    }
    assert_eq!(
        full.context(0).unwrap().input_abnormal(PortId(0)),
        Some(terminal)
    );
    assert_eq!(
        small.context(0).unwrap().input_abnormal(PortId(0)),
        Some(terminal)
    );
    assert_eq!(full.step(), small.step());
    let cost = allocation.finish();
    assert_eq!(cost.allocations + cost.reallocations, 0);
    assert_eq!(
        full.signs.events().collect::<Vec<_>>(),
        small.signs.events().collect::<Vec<_>>()
    );
    assert_eq!(small.cord_terminals, [Some(terminal); 2]);
}
trait ClosureExercise {
    fn close(&mut self, targets: &[(RemoteEndpointId, CordId)], terminal: CanonicalValue);
}
impl<const T: usize> ClosureExercise for StockKernel<T> {
    fn close(&mut self, targets: &[(RemoteEndpointId, CordId)], terminal: CanonicalValue) {
        self.close_remote_input_fanout_abnormal(targets, terminal)
            .unwrap();
        let count = self.signs.events().count();
        self.close_remote_input_fanout_abnormal(targets, terminal)
            .unwrap();
        assert_eq!(self.signs.events().count(), count);
        assert_eq!(
            self.close_remote_input(targets[0].0, targets[0].1),
            Err(SchedulerError::RemoteDeliveryRejected)
        );
        assert_eq!(
            self.close_remote_input_abnormal(
                targets[0].0,
                targets[0].1,
                CanonicalValue::new(&[18; 100]).unwrap()
            ),
            Err(SchedulerError::RemoteDeliveryRejected)
        );
        assert_eq!(
            self.close_remote_input_abnormal(RemoteEndpointId(99), CordId(0), terminal),
            Err(SchedulerError::InvalidRemoteCordAccess)
        );
    }
}
#[test]
fn compact_stock_is_exact_budgeted_storage_and_wrong_counts_refuse() {
    let exact = size_of::<StockKernel<2>>();
    assert!(exact < size_of::<StockKernel<3>>());
    assert!(prepared::<2>(exact).is_ok());
    assert_eq!(
        prepared::<2>(exact - 1).err(),
        Some(KernelPreparationError::Capacity {
            required: exact,
            available: exact - 1
        })
    );
    for error in [
        prepared::<0>(usize::MAX).err(),
        prepared::<1>(usize::MAX).err(),
        prepared::<4>(usize::MAX).err(),
    ] {
        assert_eq!(
            error,
            Some(KernelPreparationError::Topology(
                SchedulerError::InvalidPlan
            ))
        );
    }
}
#[test]
fn compact_stock_refuses_actual_routed_tracks_and_drops_owned_values_once() {
    for stock in [false, true] {
        let drops = Rc::new(Cell::new(0));
        let run = |values| {
            StockKernel::<2, DroppedValues>::prepare_boxed_with_linked_table_writer(
                values,
                signs(),
                usize::MAX,
                &CONTRACTS,
                &CONTRACTS,
                |e| e,
                |nodes, cords, routes, calls| {
                    let active = tables(nodes, cords, routes, calls)?;
                    if stock {
                        cords[0] = cords[0].with_track(AssignedConnectionTrack::AbnormalTerminal);
                    }
                    Ok(active)
                },
            )
        };
        let result = run(DroppedValues {
            inner: FixedValueStore::new(16).unwrap(),
            drops: drops.clone(),
        });
        if stock {
            assert_eq!(
                result.err(),
                Some(KernelPreparationError::Topology(
                    SchedulerError::InvalidPlan
                ))
            );
        } else {
            drop(result.unwrap());
        }
        assert_eq!(drops.get(), 1);
    }
}
#[test]
fn known_compact_topology_refuses_before_allocating_and_ordinary_path_matches() {
    let mut nodes = [NodeSpec {
        input_cords: [None; 2],
        maximum_step_fuel: 0,
    }];
    let mut cords = [CordSpec::inactive(); 3];
    let mut routes = FixedRoutes::new(1);
    let mut calls = FixedHostCallBindings::new(1);
    tables(&mut nodes, &mut cords, &mut routes, &mut calls).unwrap();
    let values = FixedValueStore::new(16).unwrap();
    let log = signs();
    cords[0] = cords[0].with_track(AssignedConnectionTrack::AbnormalTerminal);
    let allocation = AllocationSample::begin();
    let result =
        StockKernel::<2>::prepare_boxed(1, 2, nodes, cords, routes, calls, values, log, usize::MAX);
    let cost = allocation.finish();
    assert_eq!(
        result.err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
    assert_eq!(cost.allocations + cost.reallocations, 0);
    let mut routes = FixedRoutes::new(1);
    let mut calls = FixedHostCallBindings::new(1);
    tables(&mut nodes, &mut cords, &mut routes, &mut calls).unwrap();
    type InlineStock = FixedScheduler<
        Back,
        FixedValueStore<2, 8>,
        FixedSignLog<64>,
        1,
        3,
        2,
        2,
        1,
        1,
        0,
        1,
        1,
        0,
        1,
        2,
    >;
    let mut ordinary = InlineStock::new_with_active_counts_and_host_calls(
        1,
        2,
        nodes,
        cords,
        routes,
        calls,
        [Back {
            first_closed: false,
            changed_law: false,
            finalizes: false,
        }],
        FixedValueStore::new(16).unwrap(),
        signs(),
    )
    .unwrap();
    let mut boxed = bound::<2>();
    for endpoint in [0, 1] {
        ordinary
            .close_remote_input(RemoteEndpointId(endpoint), CordId(endpoint))
            .unwrap();
        boxed
            .close_remote_input(RemoteEndpointId(endpoint), CordId(endpoint))
            .unwrap();
    }
    for _ in 0..8 {
        let status = ordinary.step();
        assert_eq!(status, boxed.step());
        if status == Ok(SchedulerStatus::Drained) {
            break;
        }
    }
    assert!(ordinary.completed.iter().all(|done| *done));
    assert_eq!(
        ordinary.signs.events().collect::<Vec<_>>(),
        boxed.signs.events().collect::<Vec<_>>()
    );
}
#[test]
fn cancellation_keeps_exact_cord_terminal_payload() {
    let terminal = CanonicalValue::new(&[29; 100]).unwrap();
    let mut small = bound::<2>();
    small
        .close_remote_input_abnormal(RemoteEndpointId(1), CordId(1), terminal)
        .unwrap();
    small.cancel().unwrap();
    assert_eq!(small.cord_terminal(1).unwrap(), Some(terminal));
    assert_eq!(small.step(), Ok(SchedulerStatus::Cancelled));
}
#[test]
fn zero_ingress_stock_keeps_unrouted_errors_and_default_routed_cancellation_inspection() {
    use super::phase_terminal_tests::{
        prepare_spontaneous, prepare_spontaneous_stock, Spontaneous,
    };
    assert_eq!(
        prepare_spontaneous_stock::<0, 1, 0>(true).err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
    let mut zero = prepare_spontaneous_stock::<0, 0, 0>(false)
        .unwrap()
        .bind([Spontaneous]);
    let mut full = prepare_spontaneous::<0, 0>(false)
        .unwrap()
        .bind([Spontaneous]);
    let allocation = AllocationSample::begin();
    assert_eq!(zero.step(), full.step());
    let cost = allocation.finish();
    assert_eq!(cost.allocations + cost.reallocations, 0);
    assert!(zero.cord_terminals.is_empty());
    let mut routed = prepare_spontaneous::<0, 1>(true)
        .unwrap()
        .bind([Spontaneous]);
    routed.step().unwrap();
    let expected = Some(CanonicalValue::new(&[9; 100]).unwrap());
    assert_eq!(
        routed
            .remote_egress_abnormal_terminal(RemoteEndpointId(1), CordId(1))
            .unwrap(),
        expected
    );
    routed.cancel().unwrap();
    assert_eq!(
        routed
            .remote_egress_abnormal_terminal(RemoteEndpointId(1), CordId(1))
            .unwrap(),
        expected
    );
}
