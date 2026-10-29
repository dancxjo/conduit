//! Actual kernel/storage conformance; the one-octet fixture codec is not an
//! environmental Type, native budget, or physical-device acceptance claim.
#![cfg(feature = "alloc")]
use conduit_kernel::{scheduler::*, *};
#[allow(
    dead_code,
    reason = "shared allocation probe supports other owning suites"
)]
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn layout(tag: u8) -> CompactCanonicalLayout {
    // SAFETY: fixture endpoints agree that one octet represents exactly 100
    // canonical bytes. Product eligibility is independently proved elsewhere.
    unsafe { CompactCanonicalLayout::from_trusted_make([tag; 32], 1, 100).unwrap() }
}
enum Driver {
    Source {
        next: u8,
        staged: bool,
        bytes: [u8; 1],
    },
    Sink {
        layout: Option<CompactCanonicalLayout>,
        current: [u8; 2],
        count: usize,
        pending: Option<u8>,
    },
}
impl StepBack<1> for Driver {
    fn compact_canonical_output(&self) -> Option<(PortId, CompactCanonicalLayout)> {
        matches!(self, Self::Source { .. }).then_some((PortId(0), layout(1)))
    }
    fn compact_canonical_input(&self, _: PortId) -> Option<CompactCanonicalLayout> {
        match self {
            Self::Sink { layout, .. } => *layout,
            _ => None,
        }
    }
    fn prepared_compact_canonical_output(&self, _: PortId) -> Option<&[u8]> {
        match self {
            Self::Source {
                bytes,
                staged: true,
                ..
            } => Some(bytes),
            _ => None,
        }
    }
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source {
                next,
                staged,
                bytes,
            } => {
                *staged = false;
                if *next == 2 {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                bytes[0] = *next + 1;
                io.send_compact_canonical(PortId(0), 100).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Sink { pending, .. } => {
                *pending = None;
                if let Some(reference) = io.input(PortId(0)) {
                    assert_eq!(reference.byte_len, 100);
                    assert!(input.input(PortId(0)).is_none());
                    let bytes = input.compact_canonical(PortId(0)).unwrap();
                    assert_eq!(bytes.len(), 1);
                    *pending = Some(bytes[0]);
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source {
                next, staged: true, ..
            } => *next += 1,
            Self::Sink {
                current,
                count,
                pending: Some(value),
                ..
            } => {
                current[*count] = *value;
                *count += 1;
            }
            _ => {}
        }
    }
}
type Play<E> = FixedScheduler<Driver, HostedValueStore, E, 2, 1, 1, 1, 2, 1>;
fn prepare<E: SignSink>(
    sink: Option<CompactCanonicalLayout>,
    compact_items: u16,
    signs: E,
) -> Play<E> {
    let mut routes = FixedRoutes::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    Play::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 8,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 8,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 100,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [
            Driver::Source {
                next: 0,
                staged: false,
                bytes: [0],
            },
            Driver::Sink {
                layout: sink,
                current: [0; 2],
                count: 0,
                pending: None,
            },
        ],
        HostedValueStore::new_with_compact_canonical_slots(
            &[4; 7],
            compact_items,
            layout(1),
            28 + u32::from(compact_items) * 100,
        )
        .unwrap(),
        signs,
    )
    .unwrap()
}
fn signs() -> FixedSignLog<64> {
    FixedSignLog::new(64 * core::mem::size_of::<KernelEvent>() as u32).unwrap()
}
unsafe fn bind<E: SignSink>(
    play: &mut Play<E>,
    selected: CompactCanonicalLayout,
) -> Result<(), SchedulerError> {
    // SAFETY: this fixture constructed both actual endpoints and this topology.
    unsafe {
        play.bind_compact_canonical_edge_from_trusted_make(
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            selected,
        )
    }
}
#[test]
fn admitted_binding_play_and_cancellation_allocate_nothing() {
    for cancel in [false, true] {
        let mut play = prepare(Some(layout(1)), 2, signs());
        let backing = play.values().reserved_storage_bytes();
        let (_, allocation) = allocation_probe::observe(|| {
            unsafe { bind(&mut play, layout(1)) }.unwrap();
            assert_eq!(
                unsafe { bind(&mut play, layout(1)) },
                Err(SchedulerError::InvalidPlan)
            );
            if cancel {
                play.step().unwrap();
                assert_eq!(play.values().used_bytes(), 100);
                play.cancel().unwrap();
                assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
            } else {
                let mut drained = false;
                for _ in 0..32 {
                    if play.step().unwrap() == SchedulerStatus::Drained {
                        drained = true;
                        break;
                    }
                }
                assert!(drained);
            }
        });
        assert_eq!(
            (
                allocation.allocations,
                allocation.reallocations,
                allocation.deallocations
            ),
            (0, 0, 0)
        );
        assert_eq!(
            (play.values().used_items(), play.values().used_bytes()),
            (0, 0)
        );
        assert_eq!(play.values().reserved_storage_bytes(), backing);
        if !cancel {
            match &play.drivers()[1] {
                Driver::Sink { current, count, .. } => assert_eq!((*current, *count), ([1, 2], 2)),
                _ => unreachable!(),
            }
        }
    }
}
#[test]
fn missing_mismatched_and_underbacked_consumers_refuse_before_play() {
    for (sink, items, selected) in [
        (None, 2, layout(1)),
        (Some(layout(2)), 2, layout(1)),
        (Some(layout(1)), 1, layout(1)),
        (Some(layout(1)), 2, layout(2)),
    ] {
        let mut play = prepare(sink, items, signs());
        let (_, allocation) = allocation_probe::observe(|| {
            assert_eq!(
                unsafe { bind(&mut play, selected) },
                Err(SchedulerError::InvalidPlan)
            )
        });
        assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
        assert_eq!(play.decisions(), 0);
        assert_eq!(play.values().used_items(), 0);
    }
}
#[test]
fn real_constructor_refusal_at_every_allocation_drops_all_fresh_backing() {
    let (baseline, allocation) = allocation_probe::observe(|| {
        HostedValueStore::new_with_compact_canonical_slots(&[4; 7], 2, layout(1), 228).unwrap()
    });
    assert_eq!(
        allocation.retained_bytes(),
        baseline.reserved_storage_bytes()
    );
    drop(baseline);
    for index in 0..allocation.allocations {
        let (result, failed) = allocation_probe::observe(|| {
            let _failure = allocation_probe::refuse_from_nth_allocation(index);
            HostedValueStore::new_with_compact_canonical_slots(&[4; 7], 2, layout(1), 228)
        });
        assert!(
            matches!(result, Err(StorageError::ByteCapacityExceeded)),
            "index={index}"
        );
        assert_eq!(failed.retained_bytes(), 0, "index={index}");
    }
}
