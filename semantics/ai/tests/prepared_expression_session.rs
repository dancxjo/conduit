#![cfg(feature = "kernel-operation-owners")]
use conduit_core::{ConfigurationValue, PortTemporal};
use conduit_kernel::{
    scheduler::{
        prepared_session::ReusableStepBack, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    FixedValueStore, PortId, ValueRef,
};
use conduit_semantic_catalog::PreparedExpressionBack;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn program() -> conduit_plot::PortableExpressionProgram {
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            "plot expression (\n input: I32 >> output: I32\n) = (. + 1)\n",
        ),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let authoring = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "expression",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &authoring.expanded.gears[0].configuration[0].value
    else {
        panic!("program");
    };
    conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn frame(input: &[u8], ready: bool) -> (StepIo<1>, StepInputBytes<'_, 1>) {
    (
        StepIo::test_frame(
            [Some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: input.len() as u32,
            })],
            [false],
            [ready.then_some(4)],
            None,
            8,
        ),
        StepInputBytes::test_frame([Some(input)], None),
    )
}
#[test]
fn value_reuse_is_allocation_free_and_requires_committed_output() {
    let mut back = PreparedExpressionBack::prepare(&program()).unwrap();
    let values = FixedValueStore::<1, 4>::new(4).unwrap();
    let capacity = back.output_capacity();
    let (_, allocation) = allocation_probe::observe(|| {
        for value in 0..64i32 {
            let input = value.to_le_bytes();
            let (mut io, bytes) = frame(&input, false);
            assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
            assert!(
                !<PreparedExpressionBack as ReusableStepBack<1>>::preflight_next(
                    &back,
                    &(),
                    &values
                )
            );
            let (mut io, bytes) = frame(&input, true);
            assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
            assert!(
                !<PreparedExpressionBack as ReusableStepBack<1>>::preflight_next(
                    &back,
                    &(),
                    &values
                )
            );
            assert_eq!(
                <PreparedExpressionBack as StepBack<1>>::prepared_output(&back, PortId(0)),
                Some((value + 1).to_le_bytes().as_slice())
            );
            <PreparedExpressionBack as StepBack<1>>::step_committed(&mut back);
            assert!(
                <PreparedExpressionBack as ReusableStepBack<1>>::preflight_next(
                    &back,
                    &(),
                    &values
                )
            );
            <PreparedExpressionBack as ReusableStepBack<1>>::begin_next(&mut back, ());
        }
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(back.output_capacity(), capacity);
}
#[test]
fn cancellation_and_closing_flow_refuse_value_rearm() {
    let program = program();
    let values = FixedValueStore::<1, 4>::new(4).unwrap();
    let mut cancelled = PreparedExpressionBack::prepare(&program).unwrap();
    <PreparedExpressionBack as StepBack<1>>::cancel(&mut cancelled);
    assert!(
        !<PreparedExpressionBack as ReusableStepBack<1>>::preflight_next(&cancelled, &(), &values)
    );
    let flow = PreparedExpressionBack::prepare_with_temporal(
        &program,
        PortTemporal::Flow { closes: true },
    )
    .unwrap();
    assert!(!<PreparedExpressionBack as ReusableStepBack<1>>::preflight_next(&flow, &(), &values));
    assert!(
        PreparedExpressionBack::prepare_with_temporal(&program, PortTemporal::Current).is_err()
    );
}
