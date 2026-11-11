use super::*;
use conduit_core::{
    ConfigurationEntry, Quantity, StructuredInfoValue, TemporalInstant, TemporalScale, Unit,
};
use conduit_data::{
    BoundedMeasurementWindow, FullWindowPolicy, MeasurementRange, MeasurementSample,
    MeasurementWindowProfile,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, HostCallOutcome, PortId, RequestId, ValueRef,
};

fn placement() -> PlannedGear {
    let offer = measurement_plot_std_offer();
    conduit_core::planned_gear_from_parts! {
        semantic_contract: offer.semantic_contract,
        placement_id: "measurement-plot".into(), gear_id: "plot".into(),
        kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry { key: "points".into(), value: ConfigurationValue::U64(2) },
            ConfigurationEntry { key: "when-full".into(), value: ConfigurationValue::Text("evenly-spaced".into()) }],
        host_id: "host/measurement".into(), boot_id: "boot/measurement".into(), offer_generation: conduit_core::OfferGeneration(1),
        capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id, base: None, realization_characteristics: vec![],
        limits: offer.limits, inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: vec![],
        host_calls: offer.host_calls, resources: vec![], authority: vec![], pool_references: vec![],
    }
}
fn input() -> Vec<u8> {
    let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
        capacity: 4,
        range: MeasurementRange {
            minimum: Quantity::new(0, Unit::Millivolt),
            maximum: Quantity::new(100, Unit::Millivolt),
        },
        clock_basis: "source-boot".into(),
        full_policy: FullWindowPolicy::Reject,
    })
    .unwrap();
    for index in 0..4 {
        window
            .push(MeasurementSample {
                value: Quantity::new(index * 25, Unit::Millivolt),
                observed_at: TemporalInstant {
                    ticks: (index + 1) as u64,
                    scale: TemporalScale::Milliseconds,
                    clock_basis: "source-boot".into(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                }
                .try_into()
                .unwrap(),
                uncertainty: None,
            })
            .unwrap();
    }
    StructuredInfoValue::leaf(
        conduit_data::measurement_window_type(),
        conduit_data::encode_measurement_window(&window).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn value(slot: u16, len: usize) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len: len as u32,
    }
}
fn completed(output: ValueRef) -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: Some(BoundedValueRef::new(output, measurement_plot_value_bounds().1).unwrap()),
        failure: None,
    }
}

#[test]
fn exact_placement_and_configuration_are_checked_before_provider_preparation() {
    let correct = placement();
    PreparedMeasurementPlot::prepare(&correct).unwrap();
    let mut wrong = correct.clone();
    wrong.artifact_id = "foreign".into();
    assert!(PreparedMeasurementPlot::prepare(&wrong).is_err());
    let mut wrong = correct.clone();
    wrong.configuration[0].value = ConfigurationValue::U64(33);
    assert!(PreparedMeasurementPlot::prepare(&wrong).is_err());
    let mut wrong = correct.clone();
    wrong.configuration[1].value = ConfigurationValue::Text("truncate".into());
    assert!(PreparedMeasurementPlot::prepare(&wrong).is_err());
    let mut wrong = correct.clone();
    wrong.outputs[0].temporal = conduit_core::PortTemporal::Current;
    assert!(PreparedMeasurementPlot::prepare(&wrong).is_err());
    let mut wrong = correct;
    wrong.configuration.push(wrong.configuration[0].clone());
    assert!(PreparedMeasurementPlot::prepare(&wrong).is_err());
}

#[test]
fn shared_semantic_projection_emits_exact_typed_series_and_preserves_refused_output() {
    let (_, mut provider) = PreparedMeasurementPlot::prepare(&placement()).unwrap();
    let capacity = provider.retained_backing_bytes();
    let encoded = provider.execute(&input()).unwrap().to_vec();
    let structured = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
    assert_eq!(
        structured.value_type(),
        &conduit_data::measurement_plot_series_type()
    );
    let conduit_core::StructuredInfoValueShape::Leaf(bytes) = structured.shape() else {
        panic!("series leaf")
    };
    let series = conduit_data::decode_measurement_plot_series(bytes).unwrap();
    assert_eq!(
        (
            series.source_samples(),
            series.omitted_samples(),
            series.points().len()
        ),
        (4, 2, 2)
    );
    assert_eq!(
        (
            *series.point(0).unwrap().source_index(),
            *series.point(1).unwrap().source_index()
        ),
        (0, 3)
    );
    // Another well-formed exact value is not a MeasurementWindow.
    assert_eq!(provider.execute(&encoded).err().unwrap().detail, 2);
    assert!(provider.execute(&[0; 1]).is_err());
    assert_eq!(provider.output, encoded);
    let too_large = vec![0; measurement_plot_value_bounds().0 as usize + 1];
    assert_eq!(provider.execute(&too_large).err().unwrap().detail, 1);
    assert_eq!(provider.output, encoded);
    assert_eq!(provider.retained_backing_bytes(), capacity);
}

#[test]
fn production_kernel_back_correlates_one_host_result_and_waits_under_output_pressure() {
    let (mut back, mut provider) = PreparedMeasurementPlot::prepare(&placement()).unwrap();
    let bytes = input();
    let input = value(0, bytes.len());
    let mut frame = StepIo::test_frame([Some(input)], [false], [None], None, 8);
    assert_eq!(
        back.step(
            &mut frame,
            &StepInputBytes::test_frame([Some(&bytes)], None)
        ),
        StepOutcome::Progress
    );
    assert!(frame.test_consumed(PortId(0)));
    let (id, operation, request) = frame.test_host_request().unwrap();
    assert_eq!((id, operation), (RequestId(0), HostCallId(0)));
    assert_eq!(request.value, input);
    // Provider execution is the explicit Host Call boundary, not kernel Step.
    let output = value(1, provider.execute(&bytes).unwrap().len());
    let completion = Some((id, completed(output)));
    let mut blocked = StepIo::test_frame([None], [false], [None], completion, 8);
    assert_eq!(
        back.step(&mut blocked, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Await
    );
    assert!(!blocked.test_host_completion_consumed());
    let mut ready = StepIo::test_frame(
        [None],
        [false],
        [Some(measurement_plot_value_bounds().1)],
        completion,
        8,
    );
    assert_eq!(
        back.step(&mut ready, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(ready.test_output(PortId(0)), Some(output));
    assert!(ready.test_host_completion_consumed());
}

#[test]
fn cancelled_kernel_request_cannot_emit_a_late_completed_projection() {
    let (mut back, _) = PreparedMeasurementPlot::prepare(&placement()).unwrap();
    let mut initial = StepIo::test_frame([Some(value(0, 8))], [false], [None], None, 8);
    assert_eq!(
        back.step(&mut initial, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    <HostCallBack as StepBack<1>>::cancel(&mut back);
    let mut late = StepIo::test_frame(
        [None],
        [false],
        [Some(measurement_plot_value_bounds().1)],
        Some((RequestId(0), completed(value(1, 8)))),
        8,
    );
    assert!(matches!(
        back.step(&mut late, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
    assert_eq!(late.test_output(PortId(0)), None);
}

#[test]
fn declared_reject_policy_does_not_silently_decimate_a_larger_window() {
    let mut selected = placement();
    selected.configuration[1].value = ConfigurationValue::Text("reject".into());
    let (_, mut provider) = PreparedMeasurementPlot::prepare(&selected).unwrap();
    let bytes = input();
    assert_eq!(provider.execute(&bytes).err().unwrap().detail, 6);
    assert!(provider.output.is_empty());
}
