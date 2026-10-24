#[path = "trajectory/common.rs"]
mod common;
use common::*;
use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn common_prepare(
    value: &AudioQuantityTrajectory,
) -> Result<PreparedAudioQuantityTrajectory, AudioTrajectoryRefusal> {
    PreparedAudioQuantityTrajectory::new_with_linear_profile(
        &value.clone().encode().unwrap(),
        AudioLinearTrajectoryProfile::CommonBasisU32,
    )
}
fn linear(
    left: AudioTrajectoryQuantity,
    right: AudioTrajectoryQuantity,
    end: u64,
) -> AudioQuantityTrajectory {
    trajectory(vec![segment(
        time(0, 20000),
        time(end, 20000),
        left,
        right,
        AudioTrajectoryInterpolation::Linear,
    )])
}
#[test]
fn real_acoustic_values_keep_original_fractions_and_replay_source() {
    for (left, right) in [
        (hz(270, 1), hz(3010, 1)),
        (hz(2290, 2), hz(3010, 2)),
        (amplitude(0, 1000), amplitude(900, 1000)),
    ] {
        let authored = linear(left.clone(), right.clone(), 4000);
        let canonical = authored.clone().encode().unwrap();
        assert!(prepare(&authored).is_err());
        let prepared = common_prepare(&authored).unwrap();
        for q in [0, 2000, 4000] {
            let receipt = prepared.evaluate(&query(time(q, 20000))).unwrap();
            assert_eq!(
                receipt.linear_profile(),
                AudioLinearTrajectoryProfile::CommonBasisU32
            );
            assert_eq!(receipt.original_canonical(), canonical);
            if q == 0 {
                assert_eq!(receipt.result(), &left);
            } else if q == 4000 {
                assert_eq!(receipt.result(), &right);
            } else {
                let (n, d) = ratio(receipt.result());
                let (ln, ld) = ratio(&left);
                let (rn, rd) = ratio(&right);
                assert_eq!(n * 2 * ld * rd, (ln * rd + rn * ld) * d);
            }
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
        assert_eq!(authored.encode().unwrap(), canonical);
    }
}
#[test]
fn all_domains_match_independent_u128_oracle_at_u32_boundary() {
    let m = u32::MAX as u64;
    for (left, right) in [
        (hz(m, m), hz(m - 2, m)),
        (cycle(m, m), cycle(m - 2, m)),
        (amplitude(m, m), amplitude(0, m)),
        (
            AudioTrajectoryQuantity::power(m, m).unwrap(),
            AudioTrajectoryQuantity::power(m, 0).unwrap(),
        ),
    ] {
        let q = m / 2;
        let (ln, ld) = ratio(&left);
        let (rn, rd) = ratio(&right);
        assert_eq!(ld, rd);
        let prepared = common_prepare(&linear(left, right, m)).unwrap();
        let result = prepared.evaluate(&query(time(q, 20000))).unwrap();
        let (n, d) = ratio(result.result());
        let expected_n = ln * (m - q) as u128 + rn * q as u128;
        let expected_d = ld * m as u128;
        assert_eq!(n, expected_n);
        assert_eq!(d, expected_d);
        assert!(n <= u64::MAX as u128 && d <= u64::MAX as u128);
    }
}
#[test]
fn unsupported_inputs_refuse_without_changing_authored_evidence() {
    let authored = linear(hz(270, 1), hz(3010, 1), 4000);
    let bytes = authored.clone().encode().unwrap();
    let prepared = common_prepare(&authored).unwrap();
    for q in [time(1, 10), time(u32::MAX as u64 + 1, 20000)] {
        assert!(prepared.evaluate(&query(q)).is_err());
    }
    assert!(matches!(
        prepared.evaluate(&query(time(4001, 20000))),
        Err(AudioTrajectoryRefusal::MissingCoverage)
    ));
    let foreign = AudioTrajectoryQuery::new(anchor(2, 1), time(2000, 20000))
        .unwrap()
        .encode()
        .unwrap();
    assert!(matches!(
        prepared.evaluate(&foreign),
        Err(AudioTrajectoryRefusal::ForeignAnchor)
    ));
    for value in [
        linear(hz(270, 1), hz(3010, 2), 4000),
        linear(hz(u32::MAX as u64 + 1, 1), hz(3010, 1), 4000),
        linear(hz(270, 1), amplitude(1, 1), 4000),
        linear(hz(270, 1), hz(3010, 1), 0),
    ] {
        assert!(common_prepare(&value).is_err());
    }
    let different_basis = trajectory(vec![
        segment(
            time(0, 20000),
            time(4000, 20000),
            hz(270, 1),
            hz(3010, 1),
            AudioTrajectoryInterpolation::Linear,
        ),
        segment(
            time(8000, 40000),
            time(16000, 40000),
            hz(3010, 1),
            hz(2290, 1),
            AudioTrajectoryInterpolation::Linear,
        ),
    ]);
    assert!(common_prepare(&different_basis).is_err());
    assert_eq!(authored.encode().unwrap(), bytes);
}
#[test]
fn mixed_step_boundary_and_maximum_trace_are_bounded() {
    let mixed = trajectory(vec![
        segment(
            time(0, 20000),
            time(4000, 20000),
            hz(270, 1),
            hz(2290, 1),
            AudioTrajectoryInterpolation::Linear,
        ),
        segment(
            time(4000, 20000),
            time(8000, 20000),
            hz(3010, 1),
            hz(4000, 1),
            AudioTrajectoryInterpolation::Step,
        ),
    ]);
    let result = common_prepare(&mixed)
        .unwrap()
        .evaluate(&query(time(4000, 20000)))
        .unwrap();
    assert_eq!(result.selected_segment(), 1);
    assert_eq!(ratio(result.result()), (3010, 1));
    let full = trajectory(
        (0..16)
            .map(|i| {
                segment(
                    time(i * 100, 20000),
                    time((i + 1) * 100, 20000),
                    hz(270, 1),
                    hz(3010, 1),
                    AudioTrajectoryInterpolation::Linear,
                )
            })
            .collect(),
    );
    let result = common_prepare(&full)
        .unwrap()
        .evaluate(&query(time(1600, 20000)))
        .unwrap();
    assert_eq!(result.selected_segment(), 15);
    assert_eq!(result.executions().len(), 112);
}

#[test]
fn authored_timing_projects_separately_at_8k_and_48k() {
    let authored = linear(hz(270, 1), hz(3010, 1), 4000);
    let original = authored.clone().encode().unwrap();
    let prepared = common_prepare(&authored).unwrap();
    let exact = prepared.evaluate(&query(time(2000, 20000))).unwrap();
    let projection = PreparedAudioSampleProjection::new().unwrap();
    for (rate, expected) in [(8000, 800), (48000, 4800)] {
        let basis =
            AudioSampleRateBasis::new(anchor(1, 1), AudioFrameQuantization::Floor, rate).unwrap();
        let quantity = AudioSampleProjectionQuantity::duration(20000, 2000).unwrap();
        let request = AudioSampleProjectionRequest::new(basis, quantity).unwrap();
        let result = projection.project(&request.encode().unwrap()).unwrap();
        assert_eq!(*result.result().raw().whole_frames(), expected);
        assert_eq!(*result.result().raw().remainder_numerator(), 0);
        assert_eq!(exact.original_canonical(), original);
    }
    // This checks timing projection, not PCM rendering or auditory quality.
}

#[test]
fn neighboring_linear_segments_are_continuous_and_reversed_span_refuses() {
    let authored = trajectory(vec![
        segment(
            time(0, 20000),
            time(4000, 20000),
            hz(270, 1),
            hz(2290, 1),
            AudioTrajectoryInterpolation::Linear,
        ),
        segment(
            time(4000, 20000),
            time(8000, 20000),
            hz(2290, 1),
            hz(3010, 1),
            AudioTrajectoryInterpolation::Linear,
        ),
    ]);
    let result = common_prepare(&authored)
        .unwrap()
        .evaluate(&query(time(4000, 20000)))
        .unwrap();
    assert_eq!(result.selected_segment(), 1);
    assert_eq!(result.result(), authored.segments().as_slice()[0].right());
    assert_eq!(result.result(), authored.segments().as_slice()[1].left());
    let reversed = trajectory(vec![segment(
        time(4000, 20000),
        time(0, 20000),
        hz(270, 1),
        hz(3010, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    assert!(matches!(
        common_prepare(&reversed),
        Err(AudioTrajectoryRefusal::InvalidSegment)
    ));
    assert!(AudioExactTimeOffset::new(0, 1).is_err());
    assert!(AudioTrajectoryQuantity::amplitude(0, 1).is_err());
}
