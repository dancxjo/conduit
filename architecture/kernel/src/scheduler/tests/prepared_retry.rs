//! An independently delayed consumer must not split a paired publication.
use super::*;

#[derive(Clone, Copy)]
enum Back {
    Producer {
        committed: u8,
    },
    Consumer {
        delay: u8,
        seen: [u8; 2],
        count: usize,
        staged: Option<u8>,
    },
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, input: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Producer { committed } => {
                if *committed == 2 {
                    return StepOutcome::Complete;
                }
                if !io.output_ready_for(PortId(0), 1) || !io.output_ready_for(PortId(1), 1) {
                    return StepOutcome::Await;
                }
                // Deliberately retain the first staged output if the second
                // exhausts fuel. The scheduler must reject the whole frame.
                if io.send_prepared(PortId(0), 1).is_err()
                    || io.send_prepared(PortId(1), 1).is_err()
                {
                    return StepOutcome::Progress;
                }
                StepOutcome::Progress
            }
            Self::Consumer { delay, staged, .. } => {
                if *delay != 0 {
                    *delay -= 1;
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
                if let Some(bytes) = input.input(PortId(0)) {
                    *staged = Some(bytes[0]);
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
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match (self, port.0) {
            (Self::Producer { committed: 0 }, 0) => Some(&[10]),
            (Self::Producer { committed: 0 }, 1) => Some(&[20]),
            (Self::Producer { committed: 1 }, 0) => Some(&[11]),
            (Self::Producer { committed: 1 }, 1) => Some(&[21]),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Producer { committed } => *committed += 1,
            Self::Consumer {
                seen,
                count,
                staged,
                ..
            } => {
                if let Some(value) = staged.take() {
                    seen[*count] = value;
                    *count += 1;
                }
            }
        }
    }
}
type Play = FixedScheduler<Back, FixedValueStore<4, 4>, FixedSignLog<64>, 3, 2, PORTS, 2, 2, 2>;
fn prepare(fuel: u16) -> Play {
    let mut routes = FixedRoutes::new(PORTS as u16);
    for index in 0..2 {
        routes
            .install(
                NodeId(0),
                PortId(index),
                RouteRange {
                    start: index,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index),
                    sink: crate::CordEndpoint::local(NodeId(index + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut producer = node([None; PORTS]);
    producer.maximum_step_fuel = fuel;
    FixedScheduler::new(
        [
            producer,
            node([Some(CordId(0)), None]),
            node([Some(CordId(1)), None]),
        ],
        core::array::from_fn(|index| {
            CordSpec::local(
                CordId(index as u16),
                (NodeId(0), PortId(index as u16)),
                (NodeId(index as u16 + 1), PortId(0)),
                CordCapacity {
                    slot_start: index as u16,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        [
            Back::Producer { committed: 0 },
            Back::Consumer {
                delay: 0,
                seen: [0; 2],
                count: 0,
                staged: None,
            },
            Back::Consumer {
                delay: 8,
                seen: [0; 2],
                count: 0,
                staged: None,
            },
        ],
        FixedValueStore::new(4).unwrap(),
        FixedSignLog::new((64 * core::mem::size_of::<crate::KernelEvent>()) as u32).unwrap(),
    )
    .unwrap()
}
#[test]
fn one_full_port_holds_both_outputs_then_retries_once_without_duplicate_values() {
    let mut play = prepare(3);
    let mut asymmetric = false;
    for _ in 0..100 {
        play.step().unwrap();
        if play.cord_usage(CordId(0)).unwrap() == (0, 0)
            && play.cord_usage(CordId(1)).unwrap() == (1, 1)
            && matches!(play.drivers[2], Back::Consumer { count: 0, .. })
        {
            asymmetric = true;
            let Back::Producer { committed } = play.drivers[0] else {
                unreachable!()
            };
            assert_eq!(committed, 1);
            assert_eq!(play.values.used_items(), 1);
        }
        if matches!(play.drivers[1], Back::Consumer { count: 2, .. })
            && matches!(play.drivers[2], Back::Consumer { count: 2, .. })
        {
            break;
        }
    }
    assert!(asymmetric);
    assert!(matches!(
        play.drivers[1],
        Back::Consumer {
            seen: [10, 11],
            count: 2,
            ..
        }
    ));
    assert!(matches!(
        play.drivers[2],
        Back::Consumer {
            seen: [20, 21],
            count: 2,
            ..
        }
    ));
    assert_eq!(play.values.used_items(), 0);
}
#[test]
fn fuel_refusal_after_first_output_never_materializes_or_commits_either() {
    let mut play = prepare(1);
    assert!(play.step().is_err());
    assert!(matches!(play.drivers[0], Back::Producer { committed: 0 }));
    assert_eq!(play.values.used_items(), 0);
    for index in 0..2 {
        assert_eq!(play.cord_usage(CordId(index)).unwrap(), (0, 0));
    }
    assert!(!play
        .signs
        .events()
        .any(|e| e.kind == KernelEventKind::ValueRouted));
}
#[test]
fn cancellation_at_each_pressure_frontier_releases_both_ports_without_late_retry() {
    for frontier in 0..20 {
        let mut play = prepare(3);
        for _ in 0..frontier {
            play.step().unwrap();
        }
        play.cancel().unwrap();
        let before = play.drivers[0];
        assert_eq!(play.values.used_items(), 0);
        for index in 0..2 {
            assert_eq!(play.cord_usage(CordId(index)).unwrap(), (0, 0));
        }
        for _ in 0..3 {
            assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
        }
        let (Back::Producer { committed: before }, Back::Producer { committed: after }) =
            (before, play.drivers[0])
        else {
            unreachable!()
        };
        assert_eq!(before, after);
    }
}
