//! Phase byte specialization must not erase ordinary abnormal truth.
use super::*;
static FINALIZE: [[Option<AssignedTerminalTransduction>; 2]; 1] = [[
    Some(AssignedTerminalTransduction {
        input: PortId(0),
        output: PortId(0),
        normal_close: AssignedNormalCloseTransduction::PropagateWhenAllClose,
        abnormal: AssignedAbnormalTransduction::FinalizeThenPropagate(
            AssignedFiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: 1,
            },
        ),
        cancellation: AssignedCancellationTransduction::NotCancellable,
    }),
    law(1),
]];
#[test]
fn zero_phase_bytes_keep_normal_close_abnormal_and_cancellation_parity() {
    let make = || {
        prepare_profile::<0, 0>(true, usize::MAX, &CONTRACTS)
            .unwrap()
            .bind([Back {
                first_closed: false,
                changed_law: false,
                finalizes: false,
            }])
    };
    let mut full = bound::<0>(true);
    let mut small = make();
    let allocation = AllocationSample::begin();
    full.close_and_drain();
    small.close_and_drain();
    let cost = allocation.finish();
    assert_eq!(cost.allocations + cost.reallocations, 0);
    assert_eq!(
        full.signs.events().collect::<std::vec::Vec<_>>(),
        small.signs.events().collect::<std::vec::Vec<_>>()
    );
    let mut full = bound::<0>(true);
    let mut small = make();
    let terminal = CanonicalValue::new(&[7; 100]).unwrap();
    full.close_remote_input_abnormal(RemoteEndpointId(1), CordId(1), terminal)
        .unwrap();
    small
        .close_remote_input_abnormal(RemoteEndpointId(1), CordId(1), terminal)
        .unwrap();
    assert_eq!(full.step(), small.step());
    assert_eq!(small.cords[1].abnormal_terminal, Some(terminal));
    assert_eq!(
        full.signs.events().collect::<std::vec::Vec<_>>(),
        small.signs.events().collect::<std::vec::Vec<_>>()
    );
    let mut full = bound::<0>(true);
    let mut small = make();
    full.cancel().unwrap();
    small.cancel().unwrap();
    assert_eq!(full.step(), small.step());
    let exact = size_of::<Kernel<0, 0>>();
    assert_eq!(
        prepare_profile::<0, 0>(true, exact - 1, &CONTRACTS).err(),
        Some(KernelPreparationError::Capacity {
            required: exact,
            available: exact - 1
        })
    );
    assert!(exact < size_of::<Kernel<0, 100>>());
    std::eprintln!(
        "phase CPU full={} zero={} saved={}",
        size_of::<Kernel<0, 100>>(),
        exact,
        size_of::<Kernel<0, 100>>() - exact
    );
}
fn rejects_finalize_before_effects<const B: usize>() {
    type Store = DroppedValues;
    type Refused<const B: usize> =
        FixedScheduler<Back, Store, FixedSignLog<64>, 1, 2, 2, 2, 1, 1, 0, 1, 0, B>;
    let actual = Back {
        first_closed: false,
        changed_law: false,
        finalizes: true,
    };
    assert_eq!([actual.terminal_transductions()], FINALIZE);
    let drops = Rc::new(Cell::new(0));
    let values = DroppedValues {
        inner: FixedValueStore::new(16).unwrap(),
        drops: drops.clone(),
    };
    let log = signs();
    let calls = Cell::new(0);
    let allocation = AllocationSample::begin();
    let result = Refused::<B>::prepare_boxed_with_linked_table_writer(
        values,
        log,
        usize::MAX,
        &FINALIZE,
        &[actual.terminal_transductions()],
        |e| e,
        |nodes, cords, routes, bindings| {
            calls.set(calls.get() + 1);
            write(nodes, cords, routes, bindings)
        },
    );
    let cost = allocation.finish();
    assert_eq!(
        result.err(),
        Some(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(cost.allocations + cost.reallocations, 0);
    assert_eq!(drops.get(), 1);
}
#[test]
fn smaller_phase_profiles_refuse_even_matching_finite_finalizers_before_effects() {
    rejects_finalize_before_effects::<0>();
    rejects_finalize_before_effects::<99>();
    let prepared = Kernel::<0, 100>::prepare_boxed_with_linked_table_writer(
        FixedValueStore::new(16).unwrap(),
        signs(),
        usize::MAX,
        &FINALIZE,
        &FINALIZE,
        |e| e,
        write,
    )
    .unwrap();
    assert_eq!(prepared.storage_bytes(), size_of::<Kernel<0, 100>>());
    // A direct owned bind must apply the same stock check before replacing laws.
    let mut scheduler = prepare_profile::<1, 0>(false, usize::MAX, &CONTRACTS)
        .unwrap()
        .bind([Back {
            first_closed: false,
            changed_law: false,
            finalizes: true,
        }]);
    assert_eq!(
        scheduler.bind_terminal_transductions(FINALIZE),
        Err(SchedulerError::InvalidPlan)
    );
    assert_eq!(scheduler.terminal_transductions, CONTRACTS);
}
struct Spontaneous;
impl StepBack<1> for Spontaneous {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Abnormal {
            port: PortId(0),
            terminal: CanonicalValue::new(&[9; 100]).unwrap(),
        }
    }
}
type SpontaneousKernel<const B: usize> = FixedScheduler<
    Spontaneous,
    FixedValueStore<2, 100>,
    FixedSignLog<64>,
    1,
    2,
    1,
    2,
    1,
    2,
    0,
    1,
    0,
    B,
>;
fn spontaneous<const B: usize>() -> Box<SpontaneousKernel<B>> {
    static NONE: [[Option<AssignedTerminalTransduction>; 1]; 1] = [[None]];
    SpontaneousKernel::prepare_boxed_with_linked_table_writer(
        FixedValueStore::new(100).unwrap(),
        signs(),
        usize::MAX,
        &NONE,
        &NONE,
        |e| e,
        |nodes, cords, routes, calls| {
            nodes[0] = NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 4,
            };
            let mut targets = [RouteTarget {
                cord: CordId(0),
                sink: crate::CordEndpoint::Remote(RemoteEndpointId(0)),
            }; 2];
            for (i, cord) in cords.iter_mut().enumerate() {
                *cord = CordSpec::remote_egress(
                    CordId(i as u16),
                    (NodeId(0), PortId(0)),
                    RemoteEndpointId(i as u16),
                    CordCapacity {
                        slot_start: i as u16,
                        item_capacity: 1,
                        byte_capacity: 100,
                        pressure_policy: Default::default(),
                    },
                );
                targets[i] = RouteTarget {
                    cord: CordId(i as u16),
                    sink: crate::CordEndpoint::Remote(RemoteEndpointId(i as u16)),
                };
            }
            cords[0] = cords[0].with_track(AssignedConnectionTrack::AbnormalTerminal);
            routes.install(
                NodeId(0),
                PortId(0),
                crate::RouteRange { start: 0, len: 2 },
                &targets,
            )?;
            routes.seal()?;
            calls.seal()?;
            Ok((1, 2))
        },
    )
    .unwrap()
    .bind([Spontaneous])
}
#[test]
fn zero_phase_bytes_preserve_full_spontaneous_cord_and_unresolved_truth() {
    let mut full = spontaneous::<100>();
    let mut small = spontaneous::<0>();
    assert_eq!(full.step(), small.step());
    let terminal = CanonicalValue::new(&[9; 100]).unwrap();
    assert_eq!(small.cords[1].abnormal_terminal, Some(terminal));
    assert_eq!(small.unresolved_abnormal[0].unwrap().terminal, terminal);
    let output = small
        .remote_egress_offer(RemoteEndpointId(0), CordId(0))
        .unwrap()
        .unwrap();
    let full_output = full
        .remote_egress_offer(RemoteEndpointId(0), CordId(0))
        .unwrap()
        .unwrap();
    assert_eq!(small.host_value(output.value).unwrap(), terminal.as_slice());
    assert_eq!(
        full.host_value(full_output.value).unwrap(),
        terminal.as_slice()
    );
    assert_eq!(
        full.signs.events().collect::<std::vec::Vec<_>>(),
        small.signs.events().collect::<std::vec::Vec<_>>()
    );
}
