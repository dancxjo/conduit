//! Actual bounded scheduler with prepared-byte Source and capacity-one Cords.
use conduit_bosch_env::typed::{
    ObservationBack, ObservationContract, INPUT_MAXIMUM_BYTES, RESULT_MAXIMUM_BYTES,
};
use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepBack, StepInputBytes, StepIo,
        StepOutcome,
    },
    CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId,
    RouteRange, RouteTarget,
};
use std::cell::Cell;
const BYTES: usize = if INPUT_MAXIMUM_BYTES > RESULT_MAXIMUM_BYTES {
    INPUT_MAXIMUM_BYTES
} else {
    RESULT_MAXIMUM_BYTES
};
pub type Play<'a> =
    FixedScheduler<Back<'a>, FixedValueStore<4, BYTES>, FixedSignLog<128>, 3, 2, 1, 2, 3, 2>;
pub enum Back<'a> {
    Source {
        values: Vec<Vec<u8>>,
        next: usize,
        staged: bool,
    },
    Native {
        back: Box<ObservationBack>,
        commits: usize,
        staged: bool,
    },
    Sink {
        expected: Vec<Vec<u8>>,
        seen: usize,
        staged: bool,
        open: &'a Cell<bool>,
    },
}
impl Back<'_> {
    pub fn commits(&self) -> usize {
        match self {
            Self::Native { commits, .. } => *commits,
            _ => panic!("native"),
        }
    }
    pub fn seen(&self) -> usize {
        match self {
            Self::Sink { seen, .. } => *seen,
            _ => panic!("sink"),
        }
    }
}
impl StepBack<1> for Back<'_> {
    fn step(&mut self, io: &mut StepIo<1>, inputs: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source {
                values,
                next,
                staged,
            } => {
                *staged = false;
                let Some(value) = values.get(*next) else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_prepared(PortId(0), value.len() as u32).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Native { back, staged, .. } => {
                let result = back.step(io, inputs);
                *staged = matches!(result, StepOutcome::Progress);
                result
            }
            Self::Sink {
                expected,
                seen,
                staged,
                open,
            } => {
                *staged = false;
                if !open.get() {
                    // Fixture gate has no external wake. Yield cooperatively
                    // while preserving the production scheduler readiness law.
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
                if io.input(PortId(0)).is_some() {
                    assert_eq!(
                        inputs.input(PortId(0)).unwrap(),
                        expected.get(*seen).expect("no duplicate output")
                    );
                    io.consume(PortId(0)).unwrap();
                    *staged = true;
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
        match self {
            Self::Source {
                values,
                next,
                staged: true,
            } if port == PortId(0) => values.get(*next).map(Vec::as_slice),
            Self::Native { back, .. } => {
                <ObservationBack as StepBack<1>>::prepared_output(back, port)
            }
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { next, staged, .. } if *staged => {
                *next += 1;
                *staged = false;
            }
            Self::Native {
                back,
                commits,
                staged,
            } => {
                if *staged {
                    *commits += 1;
                }
                *staged = false;
                <ObservationBack as StepBack<1>>::step_committed(back);
            }
            Self::Sink { seen, staged, .. } if *staged => {
                *seen += 1;
                *staged = false;
            }
            _ => {}
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source { staged, .. } | Self::Sink { staged, .. } => *staged = false,
            Self::Native { back, staged, .. } => {
                *staged = false;
                <ObservationBack as StepBack<1>>::cancel(back);
            }
        }
    }
}
pub fn prepare<'a>(
    contract: &ObservationContract,
    input: Vec<Vec<u8>>,
    expected: Vec<Vec<u8>>,
    open: &'a Cell<bool>,
) -> Play<'a> {
    let mut routes = FixedRoutes::<3, 2>::new(1);
    for (cord, source, sink) in [(0, 0, 1), (1, 1, 2)] {
        routes
            .install(
                NodeId(source),
                PortId(0),
                RouteRange {
                    start: cord,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord),
                    sink: CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let cords = core::array::from_fn(|index| {
        CordSpec::local(
            CordId(index as u16),
            (NodeId(index as u16), PortId(0)),
            (NodeId(index as u16 + 1), PortId(0)),
            CordCapacity {
                slot_start: index as u16,
                item_capacity: 1,
                byte_capacity: BYTES as u32,
                pressure_policy: Default::default(),
            },
        )
    });
    Play::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 4,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 4,
            },
            NodeSpec {
                input_cords: [Some(CordId(1))],
                maximum_step_fuel: 4,
            },
        ],
        cords,
        routes,
        [
            Back::Source {
                values: input,
                next: 0,
                staged: false,
            },
            Back::Native {
                back: Box::new(ObservationBack::prepare(contract).unwrap()),
                commits: 0,
                staged: false,
            },
            Back::Sink {
                expected,
                seen: 0,
                staged: false,
                open,
            },
        ],
        FixedValueStore::new((4 * BYTES) as u32).unwrap(),
        FixedSignLog::new((128 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
    )
    .unwrap()
}
