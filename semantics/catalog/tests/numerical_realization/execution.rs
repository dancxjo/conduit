mod allocations;

use super::*;
use conduit_kernel::{scheduler::*, FixedRoutes, FixedSignLog, FixedValueStore, PortId};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
enum Back {
    Numeric(NumericalRealizationBack),
    Receipt {
        bytes: Box<[u8; 8192]>,
        length: usize,
        complete: bool,
    },
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, input: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Numeric(back) => back.step(io, input),
            Self::Receipt {
                bytes,
                length,
                complete,
            } => {
                if *complete {
                    return StepOutcome::Complete;
                }
                if let Some(value) = input.input(PortId(0)) {
                    bytes[..value.len()].copy_from_slice(value);
                    *length = value.len();
                    io.consume(PortId(0)).unwrap();
                    *complete = true;
                    StepOutcome::Progress
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Numeric(back) => {
                <NumericalRealizationBack as StepBack<PORTS>>::prepared_output(back, port)
            }
            Self::Receipt { .. } => None,
        }
    }
    fn step_committed(&mut self) {
        if let Self::Numeric(back) = self {
            <NumericalRealizationBack as StepBack<PORTS>>::step_committed(back);
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Numeric(back) => <NumericalRealizationBack as StepBack<PORTS>>::cancel(back),
            Self::Receipt { complete, .. } => *complete = true,
        }
    }
}
#[test]
fn selected_receipts_run_on_actual_plan_lowered_kernel_routes() {
    let plan = plan();
    assert_eq!(plan.fragments.len(), 2);
    for fragment in &plan.fragments {
        let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
            &plan,
            &fragment.fragment_id,
        )
        .unwrap();
        assert_eq!(lowered.nodes.len(), 2);
        assert_eq!(lowered.cords.len(), 1);
        let mut routes = FixedRoutes::<32, 1>::new(PORTS as u16);
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
        let mut expected = vec![];
        let drivers: [Back; 2] = lowered
            .nodes
            .iter()
            .map(|node| {
                let gear = fragment
                    .placements
                    .iter()
                    .find(|g| g.placement_id == node.placement_id)
                    .unwrap();
                if gear.kind_id.as_str() == NUMERICAL_REALIZE_KIND {
                    let certificate = PreparedNumericalRealization::from_property(
                        &gear.realization_properties[0],
                        &mut ExactNumericWork::new(100_000_000),
                    )
                    .unwrap();
                    let back =
                        NumericalRealizationBack::prepare(gear, certificate.algorithm()).unwrap();
                    expected = back.result_bytes().to_vec();
                    Back::Numeric(back)
                } else {
                    Back::Receipt {
                        bytes: Box::new([0; 8192]),
                        length: 0,
                        complete: false,
                    }
                }
            })
            .collect::<Vec<_>>()
            .try_into()
            .ok()
            .unwrap();
        type Play = FixedScheduler<
            Back,
            FixedValueStore<4, 8192>,
            FixedSignLog<256>,
            2,
            1,
            PORTS,
            1,
            32,
            1,
        >;
        let mut play = Play::new(
            lowered.node_specs.try_into().unwrap(),
            lowered
                .cords
                .iter()
                .map(|c| c.spec)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            routes,
            drivers,
            FixedValueStore::new(32768).unwrap(),
            FixedSignLog::new((256 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32)
                .unwrap(),
        )
        .unwrap();
        let mut drained = false;
        allocations::start();
        for _ in 0..16 {
            match play.step().unwrap() {
                SchedulerStatus::Drained => {
                    drained = true;
                    break;
                }
                SchedulerStatus::Progress { .. } => (),
                other => panic!("unexpected numeric Play state: {other:?}"),
            }
        }
        let allocations = allocations::finish();
        assert_eq!(
            allocations, 0,
            "numerical Play must use its admitted storage"
        );
        assert!(drained);
        let (bytes, length) = play
            .drivers()
            .iter()
            .find_map(|back| match back {
                Back::Receipt { bytes, length, .. } => Some((bytes, length)),
                _ => None,
            })
            .unwrap();
        assert_eq!(&bytes[..*length], expected);
        let result = StructuredInfoValue::from_canonical_bytes(&bytes[..*length]).unwrap();
        assert_eq!(result.value_type(), &numerical_result_type());
    }
}

#[test]
fn pressure_discard_retry_commit_and_cancel_preserve_one_frozen_receipt() {
    let plan = plan();
    let gear = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|g| g.kind_id.as_str() == NUMERICAL_REALIZE_KIND)
        .unwrap();
    let certificate = PreparedNumericalRealization::from_property(
        &gear.realization_properties[0],
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let mut back = NumericalRealizationBack::prepare(gear, certificate.algorithm()).unwrap();
    let expected = back.result_bytes().to_vec();
    let input = StepInputBytes::<1>::test_frame([None], None);
    let mut blocked = StepIo::test_frame([None], [false], [None], None, 1);
    assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
    assert!(<NumericalRealizationBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none());
    let mut ready = StepIo::test_frame([None], [false], [Some(8192)], None, 1);
    assert_eq!(back.step(&mut ready, &input), StepOutcome::Progress);
    assert_eq!(
        ready.test_prepared_output(),
        Some((PortId(0), expected.len() as u32))
    );
    assert_eq!(
        <NumericalRealizationBack as StepBack<1>>::prepared_output(&back, PortId(0)),
        Some(expected.as_slice())
    );
    // Discard the transaction: no step_committed notification was delivered.
    let mut retry = StepIo::test_frame([None], [false], [Some(8192)], None, 1);
    assert_eq!(back.step(&mut retry, &input), StepOutcome::Progress);
    assert_eq!(
        <NumericalRealizationBack as StepBack<1>>::prepared_output(&back, PortId(0)),
        Some(expected.as_slice())
    );
    <NumericalRealizationBack as StepBack<1>>::step_committed(&mut back);
    let mut after = StepIo::test_frame([None], [false], [Some(8192)], None, 1);
    assert_eq!(back.step(&mut after, &input), StepOutcome::Complete);
    assert_eq!(after.test_prepared_output(), None);
    let mut cancelled = NumericalRealizationBack::prepare(gear, certificate.algorithm()).unwrap();
    <NumericalRealizationBack as StepBack<1>>::cancel(&mut cancelled);
    assert_eq!(cancelled.step(&mut after, &input), StepOutcome::Complete);
    let mut small = NumericalRealizationBack::prepare(gear, certificate.algorithm()).unwrap();
    let mut frame = StepIo::test_frame([None], [false], [Some(1)], None, 1);
    assert!(matches!(
        small.step(&mut frame, &input),
        StepOutcome::Fail(_)
    ));
    assert_eq!(frame.test_prepared_output(), None);
}
