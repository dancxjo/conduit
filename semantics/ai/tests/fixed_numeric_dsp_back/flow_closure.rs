//! Closure travels through real lowered Cords and the existing FixedScheduler.
use super::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, SchedulerError, SchedulerStatus},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, ValueStorage,
};
use std::{cell::RefCell, rc::Rc};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
enum Driver {
    Source {
        values: Vec<ValueRef>,
        next: usize,
        staged: bool,
    },
    Dsp(FixedDspBack<160, 1>),
    Sink {
        seen: Rc<RefCell<Vec<Vec<u8>>>>,
        staged: Option<Vec<u8>>,
    },
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, input: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source {
                values,
                next,
                staged,
            } => {
                if *next == values.len() {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), values[*next]).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Dsp(back) => back.step(io, input),
            Self::Sink { staged, .. } => {
                if io.input_closed(PortId(0)) {
                    return StepOutcome::Complete;
                }
                let Some(bytes) = input.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                *staged = Some(bytes.to_vec());
                io.consume(PortId(0)).unwrap();
                StepOutcome::Progress
            }
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { next, staged, .. } if *staged => {
                *next += 1;
                *staged = false;
            }
            Self::Dsp(back) => <FixedDspBack<160, 1> as StepBack<PORTS>>::step_committed(back),
            Self::Sink { seen, staged } => {
                if let Some(bytes) = staged.take() {
                    seen.borrow_mut().push(bytes);
                }
            }
            _ => {}
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if let Self::Dsp(back) = self {
            <FixedDspBack<160, 1> as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Dsp(back) = self {
            <FixedDspBack<160, 1> as StepBack<PORTS>>::cancel(back);
        }
    }
}
fn run(left: usize, right: usize, expected_outputs: usize, mismatch: bool) {
    let plan = plan(FixedDspOperation::Dot160, true);
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut values = HostedValueStore::new(16, 16384, 16 * 16384).unwrap();
    let mut codec =
        FixedF32VectorCodec::<160>::prepare(&fixed_numeric_type("NumericF32Vector160").unwrap())
            .unwrap();
    let mut sources = Vec::new();
    for count in [left, right] {
        let mut refs = Vec::new();
        for _ in 0..count {
            refs.push(values.store(codec.encode(&[0.5; 160]).unwrap()).unwrap());
        }
        sources.push(refs);
    }
    let seen = Rc::new(RefCell::new(Vec::new()));
    let drivers: Vec<Driver> = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(i, gear)| match gear.kind_id.as_str() {
            "dsp-fixture/left" => Driver::Source {
                values: sources[0].clone(),
                next: 0,
                staged: false,
            },
            "dsp-fixture/right" => Driver::Source {
                values: sources[1].clone(),
                next: 0,
                staged: false,
            },
            "dsp-fixture/sink" => Driver::Sink {
                seen: seen.clone(),
                staged: None,
            },
            _ => Driver::Dsp(
                FixedDspBack::<160, 1>::prepare_planned::<PORTS>(
                    gear,
                    lowered.node_specs[i].maximum_step_fuel,
                    FixedDspOperation::Dot160,
                    true,
                )
                .unwrap(),
            ),
        })
        .collect();
    let mut routes = FixedRoutes::<4, 3>::new(1);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 4, 3, PORTS, 3, 4, 3>::new(
        lowered.node_specs.try_into().unwrap(),
        lowered
            .cords
            .into_iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes,
        drivers
            .try_into()
            .unwrap_or_else(|_| panic!("driver count")),
        values,
        HostedSignLog::new(1024, 1024 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    let mut terminal = false;
    for _ in 0..96 {
        match scheduler.step() {
            Ok(SchedulerStatus::Drained) => {
                assert!(!mismatch);
                terminal = true;
                break;
            }
            Err(SchedulerError::BackFailed(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidInput,
                detail: 2802,
            })) => {
                assert!(mismatch);
                terminal = true;
                break;
            }
            Err(error) => panic!("unexpected kernel failure: {error:?}"),
            _ => {}
        }
    }
    assert!(terminal, "finite closure must settle, not await forever");
    if mismatch {
        // A refusal can settle the Play before an already-published value is
        // delivered to this sink; committed DSP output count is checked below.
        assert!(seen.borrow().len() <= expected_outputs);
    } else {
        assert_eq!(seen.borrow().len(), expected_outputs);
    }
    let decoder =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap();
    for bytes in seen.borrow().iter() {
        let mut output = [0.];
        decoder.decode(bytes, &mut output).unwrap();
        assert_eq!(output, [40.]);
    }
    for driver in scheduler.drivers() {
        if let Driver::Dsp(back) = driver {
            assert_eq!(back.committed_frames(), expected_outputs as u64);
        }
    }
}
#[test]
fn empty_dot_closes_without_fabricating_a_value() {
    run(0, 0, 0, false);
}
#[test]
fn queued_pair_drains_before_both_eof_and_closes() {
    run(1, 1, 1, false);
}
#[test]
fn asymmetric_eof_with_pending_other_side_refuses_both_orders() {
    run(0, 1, 0, true);
    run(1, 0, 0, true);
}
#[test]
fn unmatched_tail_after_a_committed_pair_refuses_both_orders() {
    run(1, 2, 1, true);
    run(2, 1, 1, true);
}
