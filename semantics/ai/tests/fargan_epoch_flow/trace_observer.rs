//! Source projection/reference parity and staged development observation.
use super::*;
// Shared development helpers are independently exercised by the Plot envelope tests.
#[allow(dead_code)]
#[path = "../../../../proof/fargan/development_trace_recorder.rs"]
mod recorder;
#[path = "../../../../proof/fargan/development_trace_sink.rs"]
mod sink;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};

fn frame(input: &[u8]) -> (StepIo<1>, StepInputBytes<'_, 1>) {
    (
        StepIo::test_frame(
            [Some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: input.len() as u32,
            })],
            [false],
            [None],
            None,
            16384,
        ),
        StepInputBytes::test_frame([Some(input)], None),
    )
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
pub(super) fn check_observer() {
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let anchor = format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let source = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../speech/fargan_epoch_trace.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_trace_flow.conduit");
    let source = source.replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {anchor}\n"),
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    for name in [
        "speech/flow-fargan-feature-condition-trace",
        "speech/flow-fargan-conditioning-history-trace",
        "speech/flow-fargan-committed-result-trace",
    ] {
        let graph =
            expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
        assert_eq!(graph.expanded.gears.len(), 1);
        let ConfigurationValue::Text(encoded) = &graph.expanded.gears[0].configuration[0].value
        else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let input = declarations::fixture_value(&program.input_type)
            .canonical_bytes()
            .unwrap();
        let output = program.evaluate(&input).unwrap();
        assert_eq!(
            PreparedPortableExpressionEvaluator::new(&program)
                .unwrap()
                .evaluate(&input)
                .unwrap(),
            output
        );
        let value = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        let selected = if name.ends_with("committed-result-trace") {
            field(field(&value, "result"), "model")
        } else {
            field(&value, "model")
        };
        let maximum =
            maximum_prepared_transport_value_bytes(&program.output_type).unwrap() as usize;
        let prepare = || {
            sink::DevelopmentTraceSink::prepare(
                recorder::DevelopmentTraceRecorder::prepare(
                    &program.output_type,
                    selected,
                    7,
                    1,
                    maximum,
                )
                .unwrap(),
                maximum,
            )
            .unwrap()
        };
        let mut observer = prepare();
        observer.pause(true);
        let (mut io, bytes) = frame(&output);
        assert_eq!(observer.step(&mut io, &bytes), StepOutcome::Await);
        assert!(!io.test_consumed(PortId(0)));
        assert_eq!(observer.recorder().recorded_rows(), 0);
        observer.pause(false);
        assert_eq!(observer.step(&mut io, &bytes), StepOutcome::Progress);
        assert_eq!(observer.recorder().recorded_rows(), 0);
        <sink::DevelopmentTraceSink as StepBack<1>>::cancel(&mut observer);
        <sink::DevelopmentTraceSink as StepBack<1>>::step_committed(&mut observer);
        assert_eq!(observer.recorder().recorded_rows(), 0);
        assert!(observer.recorder().finish().is_err());
        let mut observer = prepare();
        let (mut io, bytes) = frame(&output);
        let (outcome, allocations) = allocation_probe::measure(|| observer.step(&mut io, &bytes));
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(allocations, 0);
        assert!(io.test_consumed(PortId(0)));
        let (_, allocations) = allocation_probe::measure(|| {
            <sink::DevelopmentTraceSink as StepBack<1>>::step_committed(&mut observer)
        });
        assert_eq!(allocations, 0);
        assert_eq!(
            observer.recorder().finish().unwrap(),
            core::slice::from_ref(&output)
        );
        let (mut io, bytes) = frame(&output);
        assert!(matches!(
            observer.step(&mut io, &bytes),
            StepOutcome::Fail(_)
        ));
        assert!(!io.test_consumed(PortId(0)));
        assert_eq!(observer.recorder().finish().unwrap(), [output]);
    }
}
#[test]
fn source_trace_projection_rows_observe_only_committed_steps_under_pressure_cancel_and_duplicate_refusal(
) {
    check_observer();
}
