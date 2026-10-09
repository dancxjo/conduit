use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn basis(rate: u64) -> AudioSampleRateBasis {
    AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, rate).unwrap()
}
fn duration(n: u64, d: u64) -> AudioSampleProjectionQuantity {
    AudioSampleProjectionQuantity::duration(d, n).unwrap()
}
fn request(n: u64, d: u64, rate: u64) -> AudioSampleProjectionRequest {
    AudioSampleProjectionRequest::new(basis(rate), duration(n, d)).unwrap()
}
fn reproduce(executions: &[AudioSourceExecution]) {
    for execution in executions {
        let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(
            execution.source_program_hex(),
        )
        .unwrap();
        assert_eq!(
            program.evaluate(execution.input_canonical()).unwrap(),
            execution.output_canonical()
        );
    }
}
#[test]
fn one_authored_duration_and_200hz_cycle_project_at_8_16_48khz() {
    let authored_duration = duration(1, 10);
    let authored_frequency = AudioFrequencyHz::new(1, 200).unwrap();
    let cycle_conversion = PreparedAcousticReciprocal::new()
        .unwrap()
        .frequency_to_cycle(&authored_frequency.encode().unwrap())
        .unwrap();
    let cycle = *cycle_conversion.result();
    let authored_cycle =
        AudioSampleProjectionQuantity::cycle(*cycle.denominator(), *cycle.numerator_seconds())
            .unwrap();
    let original_duration = authored_duration.clone().encode().unwrap();
    let original_cycle = authored_cycle.clone().encode().unwrap();
    let prepared = PreparedAudioSampleProjection::new().unwrap();
    for (rate, frames, cycle_frames) in [(8000, 800, 40), (16000, 1600, 80), (48000, 4800, 240)] {
        for (quantity, expected, original) in [
            (authored_duration.clone(), frames, &original_duration),
            (authored_cycle.clone(), cycle_frames, &original_cycle),
        ] {
            let request = AudioSampleProjectionRequest::new(basis(rate), quantity).unwrap();
            let canonical = request.clone().encode().unwrap();
            let receipt = prepared.project(&canonical).unwrap();
            assert_eq!(receipt.original(), &request);
            assert_eq!(receipt.original_canonical(), canonical);
            assert_eq!(
                receipt.original().quantity().clone().encode().unwrap(),
                *original
            );
            assert_eq!(*receipt.result().raw().whole_frames(), expected);
            assert_eq!(*receipt.result().raw().remainder_numerator(), 0);
            assert_eq!(receipt.result().fidelity(), &AudioFrameGridFidelity::Exact);
            reproduce(receipt.executions());
            assert_eq!(
                receipt.result().clone().encode().unwrap(),
                receipt.admitted_canonical()
            );
        }
    }
}
#[test]
fn independent_u128_projection_reference_and_profile_extremes() {
    let prepared = PreparedAudioSampleProjection::new().unwrap();
    for (n, d, rate) in [
        (0, 1, 8000),
        (1, 3, 8000),
        (2, 6, 16000),
        (u32::MAX as u64, 1, 192000),
        (u32::MAX as u64, u32::MAX as u64, 192000),
        (1, u32::MAX as u64, 1),
    ] {
        let receipt = prepared
            .project(&request(n, d, rate).encode().unwrap())
            .unwrap();
        let product = n as u128 * rate as u128;
        assert_eq!(
            *receipt.result().raw().whole_frames() as u128,
            product / d as u128
        );
        assert_eq!(
            *receipt.result().raw().remainder_numerator() as u128,
            product % d as u128
        );
        assert_eq!(*receipt.result().fraction().denominator(), d);
        assert_eq!(*receipt.result().fraction().numerator_seconds(), n);
        assert!(product <= u64::MAX as u128);
    }
}
#[test]
fn repeated_thirds_carry_fractional_frames_without_drift() {
    let chain = AudioSampleProjectionChain::new(
        AudioCumulativeFrameBasis::new(basis(8000), 3).unwrap(),
        BoundedSequence::try_from_iter(vec![duration(1, 3); 3]).unwrap(),
    )
    .unwrap();
    let bytes = chain.clone().encode().unwrap();
    let result = PreparedAudioSampleProjection::new()
        .unwrap()
        .chain(&bytes)
        .unwrap();
    assert_eq!(result.original(), &chain);
    assert_eq!(result.original_canonical(), bytes);
    reproduce(result.origin_execution());
    for (step, (delta, whole, remainder)) in
        result
            .steps()
            .iter()
            .zip([(2666, 2666, 2), (2667, 5333, 1), (2667, 8000, 0)])
    {
        assert_eq!(*step.result().raw().frame_count(), delta);
        assert_eq!(*step.result().raw().whole_frames(), whole);
        assert_eq!(*step.result().raw().remainder_numerator(), remainder);
        assert_eq!(*step.result().cursor().basis().denominator(), 3);
        reproduce(step.executions());
    }
    assert_eq!(
        result.steps()[2].result().fidelity(),
        &AudioFrameGridFidelity::Exact
    );
}

#[path = "rate/refusals.rs"]
mod refusals;
#[path = "rate/reports.rs"]
mod reports;

#[test]
fn native_projection_storage_costs_are_inspectable() {
    let prepared = PreparedAudioSampleProjection::new().unwrap();
    let projection = prepared
        .project(&request(1, 3, 8000).encode().unwrap())
        .unwrap();
    let cursor = AudioCumulativeFrameCursor::new(
        AudioCumulativeFrameBasis::new(basis(8000), 3).unwrap(),
        2,
        2666,
    )
    .unwrap();
    let cumulative = prepared
        .append(
            &AudioCumulativeFrameRequest::new(request(1, 3, 8000), cursor)
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap();
    for (name, ty, bytes) in [
        (
            "time",
            AudioTimeFraction::semantic_type().unwrap(),
            AudioTimeFraction::new(3, 1)
                .unwrap()
                .encode()
                .unwrap()
                .len(),
        ),
        (
            "request",
            AudioSampleProjectionRequest::semantic_type().unwrap(),
            projection.original_canonical().len(),
        ),
        (
            "projection-result",
            AudioSampleProjectionResult::semantic_type().unwrap(),
            projection.admitted_canonical().len(),
        ),
        (
            "cumulative-request",
            AudioCumulativeFrameRequest::semantic_type().unwrap(),
            cumulative.original_canonical().len(),
        ),
        (
            "cumulative-result",
            AudioCumulativeFrameResult::semantic_type().unwrap(),
            cumulative.admitted_canonical().len(),
        ),
    ] {
        println!(
            "{name}: static Type={}B canonicalvalue={bytes}B",
            ty.canonical_bytes().unwrap().len()
        );
    }
    let max_type = projection
        .executions()
        .iter()
        .chain(cumulative.executions())
        .map(|execution| {
            conduit_core::StructuredInfoValue::from_canonical_bytes(execution.input_canonical())
                .unwrap()
                .value_type()
                .canonical_bytes()
                .unwrap()
                .len()
        })
        .max()
        .unwrap();
    println!("max executed Source input Type={max_type}B");
}
