#[allow(dead_code)]
#[path = "trajectory/common.rs"]
mod common;
use common::*;
use conduit_audio::*;
use conduit_data::{ClockRelation, ClockRelationQuality};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn clock() -> PreparedAudioClockBinding {
    let relation = ClockRelation::new(
        "control-clock-v1".into(),
        ClockRelationQuality::Exact,
        7,
        "elapsed-seconds".into(),
        1,
        19,
        "declared-ticks".into(),
        1000,
    )
    .unwrap();
    let binding = AudioClockBinding::new(
        anchor(1, 1),
        relation,
        "elapsed-seconds".into(),
        7,
        AudioClockSourceUnit::Seconds,
        "declared-ticks".into(),
    )
    .unwrap();
    PreparedAudioClockBinding::new(&binding.encode().unwrap()).unwrap()
}
fn schedule(rate: u64, start: AudioExactTimeOffset, count: u16) -> AudioControlProjectionSchedule {
    AudioControlProjectionSchedule::new(
        anchor(1, 1),
        AudioControlCadence::new(rate).unwrap(),
        AudioControlReconstruction::PointSamplesOnly,
        start,
        count,
    )
    .unwrap()
}
fn original(interpolation: AudioTrajectoryInterpolation) -> AudioControlTrajectorySet {
    let value = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        hz(100, 1),
        hz(200, 1),
        interpolation,
    )]);
    AudioControlTrajectorySet::new(
        AudioControlConflictPolicy::RefuseOverlap,
        BoundedSequence::try_from_iter(vec![AudioScopedControlTrajectory::new(
            "pitch".into(),
            AudioTrajectoryDomain::Frequency,
            0,
            "source/pitch".into(),
            value,
        )
        .unwrap()])
        .unwrap(),
        AudioControlGapPolicy::RefuseAnyGap,
    )
    .unwrap()
}
fn replay(executions: &[AudioSourceExecution]) {
    for execution in executions {
        assert_eq!(
            conduit_plot::PortableExpressionProgram::from_canonical_hex(
                execution.source_program_hex()
            )
            .unwrap()
            .evaluate(execution.input_canonical())
            .unwrap(),
            execution.output_canonical()
        );
    }
}
#[test]
fn control_cadences_are_independent_of_pcm_and_retain_exact_originals() {
    let original = original(AudioTrajectoryInterpolation::Linear);
    let canonical = original.clone().encode().unwrap();
    let prepared = PreparedAudioControlProjection::new(&canonical).unwrap();
    for rate in [100, 200] {
        let schedule = schedule(rate, time(0, 1), 3);
        let bytes = schedule.clone().encode().unwrap();
        let receipt = prepared.project(&bytes, &clock()).unwrap();
        assert_eq!(receipt.original_canonical(), canonical);
        assert_eq!(receipt.original(), &original);
        assert_eq!(receipt.schedule_canonical(), bytes);
        assert_eq!(receipt.schedule(), &schedule);
        assert_eq!(receipt.points().len(), 3);
        replay(receipt.executions());
        for (index, point) in receipt.points().iter().enumerate() {
            assert_eq!(*point.query().time().numerator_seconds(), index as u64);
            assert_eq!(*point.query().time().denominator(), rate);
            assert_eq!(
                point.raw().numerator_seconds(),
                point.query().time().numerator_seconds()
            );
            assert_eq!(
                point.raw().denominator(),
                point.query().time().denominator()
            );
            assert_eq!(point.admission().query(), point.query());
            assert_eq!(
                AudioControlQueryTimeRaw::decode(
                    receipt.executions()[index + 1].output_canonical()
                )
                .unwrap(),
                *point.raw()
            );
            let (n, d) = ratio(point.control().evaluations()[0].result());
            assert_eq!(
                n * rate as u128,
                d * (100 * rate as u128 + 100 * index as u128)
            );
            assert_eq!(point.control().original_canonical(), canonical);
            replay(point.clock().executions());
            replay(point.clock().binding_executions());
            for evaluation in point.control().evaluations() {
                replay(evaluation.executions());
            }
            for pcm_rate in [8000, 16000, 48000] {
                let request = AudioSampleProjectionRequest::new(
                    AudioSampleRateBasis::new(
                        anchor(1, 1),
                        AudioFrameQuantization::Floor,
                        pcm_rate,
                    )
                    .unwrap(),
                    AudioSampleProjectionQuantity::duration(rate, index as u64).unwrap(),
                )
                .unwrap();
                let pcm = PreparedAudioSampleProjection::new()
                    .unwrap()
                    .project(&request.encode().unwrap())
                    .unwrap();
                assert_eq!(
                    *pcm.result().raw().whole_frames(),
                    index as u64 * pcm_rate / rate
                );
                assert_eq!(*point.query().time().denominator(), rate);
            }
        }
        assert_eq!(
            receipt.final_query(),
            receipt.points().last().unwrap().query()
        );
        assert_eq!(
            AudioControlQueryTimeRaw::decode(receipt.executions()[0].output_canonical()).unwrap(),
            *receipt.final_raw()
        );
        assert_eq!(receipt.final_admission().query(), receipt.final_query());
    }
}
#[test]
fn thirds_and_final_endpoint_are_exact_for_step_and_linear() {
    for interpolation in [
        AudioTrajectoryInterpolation::Step,
        AudioTrajectoryInterpolation::Linear,
    ] {
        let value = original(interpolation);
        let receipt = PreparedAudioControlProjection::new(&value.encode().unwrap())
            .unwrap()
            .project(&schedule(3, time(0, 1), 4).encode().unwrap(), &clock())
            .unwrap();
        assert_eq!(*receipt.final_query().time().numerator_seconds(), 3);
        assert_eq!(*receipt.final_query().time().denominator(), 3);
        let (n, d) = ratio(receipt.points()[3].control().evaluations()[0].result());
        assert_eq!(n, 200 * d);
        assert_eq!(
            receipt.points()[1]
                .clock()
                .result()
                .raw()
                .remainder_numerator(),
            &1
        );
    }
}
#[test]
fn schedule_bounds_foreign_anchor_and_missing_coverage_refuse() {
    assert!(AudioControlCadence::new(0).is_err());
    assert!(AudioControlCadence::new(192001).is_err());
    for count in [0, 17] {
        assert!(AudioControlProjectionSchedule::new(
            anchor(1, 1),
            AudioControlCadence::new(100).unwrap(),
            AudioControlReconstruction::PointSamplesOnly,
            time(0, 1),
            count
        )
        .is_err());
    }
    assert!(AudioControlProjectionSchedule::new(
        anchor(1, 1),
        AudioControlCadence::new(192000).unwrap(),
        AudioControlReconstruction::PointSamplesOnly,
        time(0, u64::MAX),
        1
    )
    .is_err());
    let value = original(AudioTrajectoryInterpolation::Step);
    let p = PreparedAudioControlProjection::new(&value.encode().unwrap()).unwrap();
    assert!(p
        .project(&schedule(100, time(2, 1), 1).encode().unwrap(), &clock())
        .is_err());
    let foreign = AudioControlProjectionSchedule::new(
        anchor(1, 2),
        AudioControlCadence::new(100).unwrap(),
        AudioControlReconstruction::PointSamplesOnly,
        time(0, 1),
        1,
    )
    .unwrap();
    assert!(matches!(
        p.project(&foreign.encode().unwrap(), &clock()),
        Err(AudioControlProjectionRefusal::Clock(_))
    ));
}
#[test]
fn gaps_overlaps_and_numeric_profile_refusals_survive_schedule_composition() {
    let make = |segments| {
        AudioControlTrajectorySet::new(
            AudioControlConflictPolicy::RefuseOverlap,
            BoundedSequence::try_from_iter(vec![AudioScopedControlTrajectory::new(
                "pitch".into(),
                AudioTrajectoryDomain::Frequency,
                0,
                "source/pitch".into(),
                trajectory(segments),
            )
            .unwrap()])
            .unwrap(),
            AudioControlGapPolicy::RefuseAnyGap,
        )
        .unwrap()
    };
    let segment_at = |a, b| {
        segment(
            a,
            b,
            hz(100, 1),
            hz(200, 1),
            AudioTrajectoryInterpolation::Step,
        )
    };
    let gapped = make(vec![
        segment_at(time(0, 1), time(1, 3)),
        segment_at(time(2, 3), time(1, 1)),
    ]);
    let p = PreparedAudioControlProjection::new(&gapped.encode().unwrap()).unwrap();
    assert!(matches!(
        p.project(&schedule(2, time(0, 1), 3).encode().unwrap(), &clock()),
        Err(AudioControlProjectionRefusal::Control(
            AudioControlRefusal::Trajectory {
                refusal: AudioTrajectoryRefusal::MissingCoverage,
                ..
            }
        ))
    ));
    let overlapping = make(vec![
        segment_at(time(0, 1), time(2, 3)),
        segment_at(time(1, 3), time(1, 1)),
    ]);
    assert!(PreparedAudioControlProjection::new(&overlapping.encode().unwrap()).is_err());
    // A legal cadence may still exceed the deliberately finite interpolation
    // profile. It refuses rather than switching to a host floating-point path.
    let linear = original(AudioTrajectoryInterpolation::Linear);
    let p = PreparedAudioControlProjection::new(&linear.encode().unwrap()).unwrap();
    assert!(p
        .project(&schedule(1000, time(0, 1), 1).encode().unwrap(), &clock())
        .is_err());
}

#[test]
fn same_point_one_second_200hz_intent_survives_independent_pcm_and_control_grids() {
    let authored = AudioControlTrajectorySet::new(
        AudioControlConflictPolicy::RefuseOverlap,
        BoundedSequence::try_from_iter(vec![AudioScopedControlTrajectory::new(
            "frequency".into(),
            AudioTrajectoryDomain::Frequency,
            0,
            "source/point-one-second-200hz".into(),
            trajectory(vec![segment(
                time(0, 1),
                time(1, 10),
                hz(200, 1),
                hz(200, 1),
                AudioTrajectoryInterpolation::Step,
            )]),
        )
        .unwrap()])
        .unwrap(),
        AudioControlGapPolicy::RefuseAnyGap,
    )
    .unwrap();
    let original = authored.clone().encode().unwrap();
    let prepared = PreparedAudioControlProjection::new(&original).unwrap();
    // Explicit caller-declared relation: numeric rates never select a clock.
    let declared_clock = clock();
    for cadence in [100, 200] {
        // A bounded finite view inside the unchanged 1/10-second trajectory.
        let requested = schedule(cadence, time(0, 1), 11);
        let receipt = prepared
            .project(&requested.clone().encode().unwrap(), &declared_clock)
            .unwrap();
        assert_eq!(receipt.original(), &authored);
        assert_eq!(receipt.original_canonical(), original);
        assert_eq!(receipt.schedule(), &requested);
        let original_segment = &receipt.original().controls().as_slice()[0]
            .trajectory()
            .segments()
            .as_slice()[0];
        assert_eq!(original_segment.end(), &time(1, 10));
        assert_eq!(original_segment.left(), &hz(200, 1));
        assert_eq!(original_segment.right(), &hz(200, 1));
        for (index, point) in receipt.points().iter().enumerate() {
            assert_eq!(point.query().time(), &time(index as u64, cadence));
            assert_eq!(point.control().original_canonical(), original);
            assert_eq!(point.control().evaluations()[0].result(), &hz(200, 1));
            assert_eq!(
                point.clock().binding_canonical(),
                declared_clock.binding_canonical()
            );
        }
        for pcm_rate in [8000, 16000, 48000] {
            let sample_request = AudioSampleProjectionRequest::new(
                AudioSampleRateBasis::new(anchor(1, 1), AudioFrameQuantization::Floor, pcm_rate)
                    .unwrap(),
                AudioSampleProjectionQuantity::duration(10, 1).unwrap(),
            )
            .unwrap();
            let sample = PreparedAudioSampleProjection::new()
                .unwrap()
                .project(&sample_request.clone().encode().unwrap())
                .unwrap();
            assert_eq!(sample.original(), &sample_request);
            assert_eq!(*sample.result().fraction().numerator_seconds(), 1);
            assert_eq!(*sample.result().fraction().denominator(), 10);
            assert_eq!(*sample.result().raw().whole_frames(), pcm_rate / 10);
            assert_eq!(*sample.result().raw().remainder_numerator(), 0);
            assert_eq!(receipt.original_canonical(), original);
            assert_ne!(*receipt.schedule().cadence().updates_per_second(), pcm_rate);
        }
    }
}
