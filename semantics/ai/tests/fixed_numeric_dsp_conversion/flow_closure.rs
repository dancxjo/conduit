//! Conversion closure and lifecycle through actual lowered kernel Cords.
use super::*;
use conduit_ai::fixed_numeric_integer_conversion::FixedIntegerConversionBack;
use conduit_kernel::{
    scheduler::{FixedScheduler, SchedulerError, SchedulerStatus},
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, ValueStorage,
};
use std::{cell::RefCell, rc::Rc};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
enum Driver {
    Source {
        frames: Vec<ValueRef>,
        next: usize,
        staged: bool,
    },
    Conversion(FixedIntegerConversionBack<256>),
    Sink {
        seen: Rc<RefCell<Vec<Vec<u8>>>>,
        staged: Option<Vec<u8>>,
    },
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, input: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source {
                frames,
                next,
                staged,
            } => {
                if *next == frames.len() {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), frames[*next]).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Conversion(back) => back.step(io, input),
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
            Self::Conversion(back) => {
                <FixedIntegerConversionBack<256> as StepBack<PORTS>>::step_committed(back)
            }
            Self::Sink { seen, staged } => {
                if let Some(bytes) = staged.take() {
                    seen.borrow_mut().push(bytes);
                }
            }
            _ => {}
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if let Self::Conversion(back) = self {
            <FixedIntegerConversionBack<256> as StepBack<PORTS>>::prepared_output(back, port)
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        if let Self::Conversion(back) = self {
            <FixedIntegerConversionBack<256> as StepBack<PORTS>>::cancel(back);
        }
    }
}
fn run(count: usize, pressure: bool, cancel_before_conversion: bool) {
    let operation = IntegerConversion::FiniteVector(256);
    let plan = conversion_plan(operation, true);
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    // One slot is occupied by the queued original; producing a separately owned
    // output cannot overwrite it under pressure. No profile/resource bound changes.
    let capacity = if pressure { 1 } else { 8 };
    let mut values = HostedValueStore::new(capacity, 16384, u32::from(capacity) * 16384).unwrap();
    let mut codec = FixedF32VectorCodec::<256>::prepare_raw(
        &fixed_numeric_type("NumericRawF32Vector256").unwrap(),
    )
    .unwrap();
    let mut frames = Vec::new();
    let mut originals = Vec::new();
    for index in 0..count {
        let bytes = codec.encode(&[index as f32 + 0.25; 256]).unwrap().to_vec();
        frames.push(values.store(&bytes).unwrap());
        originals.push(bytes);
    }
    let seen = Rc::new(RefCell::new(Vec::new()));
    let drivers: Vec<Driver> = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(i, gear)| match gear.kind_id.as_str() {
            "dsp-fixture/left" => Driver::Source {
                frames: frames.clone(),
                next: 0,
                staged: false,
            },
            "dsp-fixture/sink" => Driver::Sink {
                seen: seen.clone(),
                staged: None,
            },
            _ => Driver::Conversion(
                FixedIntegerConversionBack::<256>::prepare_planned::<PORTS>(
                    gear,
                    lowered.node_specs[i].maximum_step_fuel,
                    operation,
                    true,
                )
                .unwrap(),
            ),
        })
        .collect();
    let mut routes = FixedRoutes::<3, 2>::new(1);
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
    let mut scheduler = FixedScheduler::<_, _, _, 3, 2, PORTS, 2, 3, 2>::new(
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
    let mut settled = false;
    for _ in 0..96 {
        let result = scheduler.step();
        if cancel_before_conversion
            && scheduler
                .drivers()
                .iter()
                .any(|driver| matches!(driver, Driver::Source { next, .. } if *next > 0))
        {
            scheduler.cancel().unwrap();
            assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Cancelled);
            settled = true;
            break;
        }
        match result {
            Ok(SchedulerStatus::Drained) => {
                assert!(!pressure);
                settled = true;
                break;
            }
            Err(SchedulerError::Storage(_)) => {
                assert!(pressure);
                // A staged proposal is not a commit; normal cancellation clears
                // it and releases the queued original through kernel ownership.
                scheduler.cancel().unwrap();
                assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Cancelled);
                settled = true;
                break;
            }
            Err(error) => panic!("unexpected kernel refusal: {error:?}"),
            _ => {}
        }
    }
    assert!(settled, "finite conversion closure/lifecycle must settle");
    let expected = if pressure || cancel_before_conversion {
        0
    } else {
        count
    };
    assert_eq!(seen.borrow().len(), expected);
    let raw = FixedF32VectorCodec::<256>::prepare_raw(
        &fixed_numeric_type("NumericRawF32Vector256").unwrap(),
    )
    .unwrap();
    let finite =
        FixedF32VectorCodec::<256>::prepare(&fixed_numeric_type("NumericF32Vector256").unwrap())
            .unwrap();
    for (original, output) in originals.iter().zip(seen.borrow().iter()) {
        let mut input_values = [0.; 256];
        let mut output_values = [0.; 256];
        raw.decode(original, &mut input_values).unwrap();
        finite.decode(output, &mut output_values).unwrap();
        assert_eq!(input_values, output_values);
    }
    for driver in scheduler.drivers() {
        if let Driver::Conversion(back) = driver {
            assert_eq!(back.committed_frames(), expected as u64);
            assert!(
                <FixedIntegerConversionBack<256> as StepBack<PORTS>>::prepared_output(
                    back,
                    PortId(0)
                )
                .is_none()
            );
        }
    }
}
#[test]
fn empty_conversion_flow_closes_without_value() {
    run(0, false, false);
}
#[test]
fn queued_conversion_values_drain_before_eof() {
    run(2, false, false);
}
#[test]
fn conversion_storage_pressure_cannot_publish_or_commit_output() {
    run(1, true, false);
}
#[test]
fn cancellation_after_source_commit_prevents_conversion_commit() {
    run(1, false, true);
}
