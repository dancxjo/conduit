#[path = "trajectory/common.rs"]
mod common;
use common::*;
use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn step_linear_and_exact_endpoint_receipts_execute_source() {
    for policy in [
        AudioTrajectoryInterpolation::Step,
        AudioTrajectoryInterpolation::Linear,
    ] {
        let authored = trajectory(vec![segment(
            time(0, 1),
            time(1, 1),
            hz(100, 1),
            hz(200, 1),
            policy,
        )]);
        let prepared = prepare(&authored).unwrap();
        for (q, expected) in [
            (time(0, 1), (100, 1)),
            (
                time(1, 2),
                if policy == AudioTrajectoryInterpolation::Step {
                    (100, 1)
                } else {
                    (150, 1)
                },
            ),
            (time(1, 1), (200, 1)),
        ] {
            let input = query(q);
            let receipt = prepared.evaluate(&input).unwrap();
            let (n, d) = ratio(receipt.result());
            assert_eq!(n * expected.1, expected.0 * d);
            assert_eq!(receipt.original(), &authored);
            assert_eq!(
                receipt.original_canonical(),
                authored.clone().encode().unwrap()
            );
            assert_eq!(receipt.query_canonical(), input);
            assert_eq!(
                receipt.result().clone().encode().unwrap(),
                receipt.admitted_result_canonical()
            );
            assert_eq!(receipt.selected_segment(), 0);
            assert_eq!(
                receipt.selected_segment_canonical(),
                authored.segments().as_slice()[0].clone().encode().unwrap()
            );
            for execution in receipt.executions() {
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
    }
}

#[test]
fn linear_fraction_results_match_independent_u128_reference() {
    for (a, b) in [
        ((1, 3), (2, 7)),
        ((200, 3), (100, 7)),
        ((255, 255), (255, 1)),
        ((0, 1), (255, 255)),
    ] {
        let authored = trajectory(vec![segment(
            time(1, 7),
            time(2, 3),
            amplitude(a.0, a.1),
            amplitude(b.0, b.1),
            AudioTrajectoryInterpolation::Linear,
        )]);
        let prepared = prepare(&authored).unwrap();
        for (qn, qd) in [(1, 7), (1, 3), (1, 2), (2, 3)] {
            let result = prepared.evaluate(&query(time(qn, qd))).unwrap();
            let (n, d) = ratio(result.result());
            // Independent rational reference: w=(q-1/7)/(2/3-1/7).
            let wn = (qn as u128 * 7 - qd as u128) * 3;
            let wd = 11 * qd as u128;
            let reference_n =
                (wd - wn) * a.0 as u128 * b.1 as u128 + wn * b.0 as u128 * a.1 as u128;
            let reference_d = wd * a.1 as u128 * b.1 as u128;
            assert_eq!(n * reference_d, reference_n * d);
            assert!(d > 0 && n <= u64::MAX as u128 && d <= u64::MAX as u128);
        }
    }
}

#[test]
fn hz_and_cycle_interpolation_are_distinct_named_domains() {
    let frequency = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        hz(100, 1),
        hz(200, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let cycles = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        cycle(1, 100),
        cycle(1, 200),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let frequency = prepare(&frequency)
        .unwrap()
        .evaluate(&query(time(1, 2)))
        .unwrap();
    let cycles = prepare(&cycles)
        .unwrap()
        .evaluate(&query(time(1, 2)))
        .unwrap();
    let (hn, hd) = ratio(frequency.result());
    let (cn, cd) = ratio(cycles.result());
    assert_eq!(hn, 150 * hd);
    assert_eq!(cn * 400, 3 * cd);
    assert_ne!(hn * cn, hd * cd); // cycle interpolation's reciprocal is 400/3 Hz.
}

#[test]
fn independent_overlapping_controls_and_fractional_time_preserve_identity() {
    let pitch = trajectory(vec![segment(
        time(0, 1),
        time(1, 10),
        hz(200, 1),
        hz(200, 1),
        AudioTrajectoryInterpolation::Step,
    )]);
    let loudness = trajectory(vec![segment(
        time(0, 1),
        time(1, 3),
        amplitude(0, 1),
        amplitude(1, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let pitch_result = prepare(&pitch)
        .unwrap()
        .evaluate(&query(time(1, 20)))
        .unwrap();
    let amplitude_result = prepare(&loudness)
        .unwrap()
        .evaluate(&query(time(1, 20)))
        .unwrap();
    assert_eq!(ratio(pitch_result.result()), (200, 1));
    let (n, d) = ratio(amplitude_result.result());
    assert_eq!(n * 20, 3 * d);
    assert_eq!(pitch_result.original(), &pitch);
    assert_eq!(amplitude_result.original(), &loudness);
    // No sample projection is claimed by this abstract timeline evaluation.
}

#[path = "trajectory/refusals.rs"]
mod refusals;

#[test]
fn power_ratio_and_zero_amplitude_keep_distinct_quantity_identity() {
    let powers = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        AudioTrajectoryQuantity::power(1, 0).unwrap(),
        AudioTrajectoryQuantity::power(1, 4).unwrap(),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let result = prepare(&powers)
        .unwrap()
        .evaluate(&query(time(1, 2)))
        .unwrap();
    assert!(matches!(result.result(), AudioTrajectoryQuantity::Power(_)));
    let (n, d) = ratio(result.result());
    assert_eq!(n, 2 * d);
    let zero = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        amplitude(0, 1),
        amplitude(0, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let result = prepare(&zero)
        .unwrap()
        .evaluate(&query(time(1, 2)))
        .unwrap();
    assert!(matches!(
        result.result(),
        AudioTrajectoryQuantity::Amplitude(_)
    ));
    assert_eq!(ratio(result.result()).0, 0);
}

#[test]
fn arithmetic_profile_extremes_remain_exact_and_within_u64() {
    let authored = trajectory(vec![segment(
        time(254, 255),
        time(255, 254),
        amplitude(255, 255),
        amplitude(255, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let result = prepare(&authored)
        .unwrap()
        .evaluate(&query(time(255, 255)))
        .unwrap();
    let (n, d) = ratio(result.result());
    let wn = (255_u128 * 255 - 254 * 255) * 254;
    let wd = (255_u128 * 255 - 254 * 254) * 255;
    let rn = (wd - wn) * 255 + wn * 255 * 255;
    let rd = wd * 255;
    assert_eq!(n * rd, rn * d);
    assert!(n < u64::MAX as u128 && d > 0 && d < u64::MAX as u128);
}
#[path = "trajectory/step_u32.rs"]
mod step_u32;
