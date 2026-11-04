#![cfg(feature = "prepared-expression-session")]
//! CPU Back/kernel conformance; no Body, physical IO or installed offer claim.
use conduit_core::{ConfigurationValue, PortTemporal, PrimitiveInfoKind as K};
use conduit_kernel::{
    scheduler::{
        prepared_session::ReusableStepBack, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    FixedValueStore, PortId, ValueRef,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, ScalarTapeInstruction as I,
    ScalarTapeOperation as O, StartupCatalog, StaticScalarExpression,
};
use conduit_semantic_catalog::{
    PreparedExpressionBack, StaticExpressionBackRefusal, StaticScalarExpressionBack,
};
use std::borrow::Cow;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
static CODE: &[I] = &[
    I {
        kind: K::U8,
        destination: 0,
        operation: O::Input,
    },
    I {
        kind: K::U8,
        destination: 1,
        operation: O::Literal(Cow::Borrowed(&[1])),
    },
    I {
        kind: K::U8,
        destination: 2,
        operation: O::Binary(conduit_plot::BinaryOperator::Add, false, 0, 1),
    },
];
fn resident(temporal: PortTemporal) -> StaticScalarExpressionBack {
    StaticScalarExpressionBack::from_resident(
        StaticScalarExpression::new(K::U8, K::U8, CODE, 2).unwrap(),
        temporal,
    )
    .unwrap()
}
fn program() -> PortableExpressionProgram {
    let checked = check_syntax_document(
        &parse_syntax_document("plot increment (\n input: U8 >> result: U8\n) = (. + 1)\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "increment", &ProfileCatalog::new())
            .unwrap()
            .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn frame(ready: bool, fuel: u16) -> StepIo<1> {
    StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 1,
        })],
        [false],
        [ready.then_some(1)],
        None,
        fuel,
    )
}
#[test]
fn resident_preparation_and_temporal_refusal_allocate_nothing() {
    let (_, cost) = allocation_probe::observe(|| {
        let back = resident(PortTemporal::Value);
        assert_eq!(back.computation_fuel(), 3);
        assert_eq!(back.required_step_fuel(), 5);
        assert!(back.scratch_bytes() > 0);
        let image = StaticScalarExpression::new(K::U8, K::U8, CODE, 2).unwrap();
        assert!(matches!(
            StaticScalarExpressionBack::from_resident(image, PortTemporal::Flow { closes: false }),
            Err(StaticExpressionBackRefusal::UnsupportedTemporal)
        ));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
fn lifecycle<B: StepBack<1>>(mut back: B, flow: bool) {
    let (_, cost) = allocation_probe::observe(|| {
        let input = StepInputBytes::test_frame([Some(&[41][..])], None);
        let mut blocked = frame(false, 16);
        assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
        assert_eq!(blocked.remaining_fuel(), 16);
        assert!(!blocked.test_consumed(PortId(0)));
        assert!(back.prepared_output(PortId(0)).is_none());
        // An aborted transaction has no commit and may be evaluated again.
        for _ in 0..2 {
            let mut io = frame(true, 16);
            assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
            assert_eq!(io.test_prepared_output(), Some((PortId(0), 1)));
            assert!(io.test_consumed(PortId(0)));
            assert_eq!(back.prepared_output(PortId(0)), Some(&[42][..]));
        }
        back.step_committed();
        assert!(back.prepared_output(PortId(0)).is_none());
        let mut closed = StepIo::test_frame([None], [true], [Some(1)], None, 16);
        assert_eq!(
            back.step(&mut closed, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert_eq!(closed.test_consumed_closed(PortId(0)), flow);
        back.cancel();
        assert!(back.prepared_output(PortId(0)).is_none());
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
#[test]
fn resident_and_owned_use_identical_value_and_flow_lifecycles() {
    for temporal in [PortTemporal::Value, PortTemporal::Flow { closes: true }] {
        let flow = matches!(temporal, PortTemporal::Flow { .. });
        lifecycle(resident(temporal), flow);
        lifecycle(
            PreparedExpressionBack::prepare_with_temporal(&program(), temporal).unwrap(),
            flow,
        );
    }
}
#[test]
fn failed_evaluation_and_insufficient_fuel_never_stage_or_consume() {
    let mut back = resident(PortTemporal::Value);
    let mut fuel = frame(true, 2);
    assert_eq!(
        back.step(
            &mut fuel,
            &StepInputBytes::test_frame([Some(&[41][..])], None)
        ),
        StepOutcome::Yield
    );
    assert!(!fuel.test_consumed(PortId(0)));
    assert!(
        <StaticScalarExpressionBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none()
    );
    let mut overflow = frame(true, 16);
    assert!(matches!(
        back.step(
            &mut overflow,
            &StepInputBytes::test_frame([Some(&[255][..])], None)
        ),
        StepOutcome::Fail(_)
    ));
    assert!(!overflow.test_consumed(PortId(0)));
    assert!(overflow.test_prepared_output().is_none());
    assert!(
        <StaticScalarExpressionBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none()
    );
}
fn reusable<B: ReusableStepBack<1, Next = ()>>(mut back: B) {
    let values = FixedValueStore::<1, 1>::new(1).unwrap();
    assert!(!back.preflight_next(&(), &values));
    let mut io = frame(true, 16);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&[1][..])], None)),
        StepOutcome::Progress
    );
    assert!(!back.preflight_next(&(), &values));
    back.step_committed();
    assert!(back.preflight_next(&(), &values));
    back.begin_next(());
    assert!(!back.preflight_next(&(), &values));
    back.cancel();
    assert!(!back.preflight_next(&(), &values));
    let mut io = frame(true, 16);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&[1][..])], None)),
        StepOutcome::Complete
    );
    assert!(!io.test_consumed(PortId(0)));
}
#[test]
fn resident_and_owned_share_committed_reuse_and_terminal_cancellation() {
    reusable(resident(PortTemporal::Value));
    reusable(PreparedExpressionBack::prepare(&program()).unwrap());
}

#[path = "resident_expression_back/kernel.rs"]
mod kernel;
