#[allow(dead_code)]
#[path = "trajectory/common.rs"]
mod common;
use common::*;
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
fn control(
    scope: &str,
    id: &str,
    domain: AudioTrajectoryDomain,
    priority: u16,
    a: AudioTrajectoryQuantity,
    b: AudioTrajectoryQuantity,
) -> AudioScopedControlTrajectory {
    AudioScopedControlTrajectory::new(
        id.into(),
        domain,
        priority,
        scope.into(),
        trajectory(vec![segment(
            time(0, 1),
            time(1, 1),
            a,
            b,
            AudioTrajectoryInterpolation::Linear,
        )]),
    )
    .unwrap()
}
fn set(
    controls: Vec<AudioScopedControlTrajectory>,
    conflict: AudioControlConflictPolicy,
) -> AudioControlTrajectorySet {
    AudioControlTrajectorySet::new(
        conflict,
        BoundedSequence::try_from_iter(controls).unwrap(),
        AudioControlGapPolicy::RefuseAnyGap,
    )
    .unwrap()
}
fn prepare_set(
    value: &AudioControlTrajectorySet,
) -> Result<PreparedAudioControlTrajectories, AudioControlRefusal> {
    PreparedAudioControlTrajectories::new(&value.clone().encode().unwrap())
}
#[test]
fn simultaneous_independent_frequency_formant_power_and_envelope_scopes_survive_transport() {
    let controls = vec![
        control(
            "source/pitch",
            "frequency",
            AudioTrajectoryDomain::Frequency,
            0,
            hz(100, 1),
            hz(200, 1),
        ),
        control(
            "source/formant",
            "formant/1",
            AudioTrajectoryDomain::Frequency,
            0,
            hz(200, 1),
            hz(100, 1),
        ),
        control(
            "source/energy",
            "relative-power",
            AudioTrajectoryDomain::Power,
            0,
            AudioTrajectoryQuantity::power(1, 1).unwrap(),
            AudioTrajectoryQuantity::power(1, 3).unwrap(),
        ),
        control(
            "source/envelope",
            "amplitude-envelope",
            AudioTrajectoryDomain::Amplitude,
            0,
            amplitude(0, 1),
            amplitude(1, 1),
        ),
    ];
    let original = set(controls, AudioControlConflictPolicy::RefuseOverlap);
    let prepared = prepare_set(&original).unwrap();
    for (t, expected) in [
        (time(0, 1), [(100, 1), (200, 1), (1, 1), (0, 1)]),
        (time(1, 2), [(150, 1), (150, 1), (2, 1), (1, 2)]),
        (time(1, 1), [(200, 1), (100, 1), (3, 1), (1, 1)]),
    ] {
        let input = query(t);
        let receipt = prepared.evaluate(&input).unwrap();
        assert_eq!(receipt.original(), &original);
        assert_eq!(
            receipt.original_canonical(),
            original.clone().encode().unwrap()
        );
        assert_eq!(receipt.query_canonical(), input);
        assert_eq!(receipt.selected_indices(), [0, 1, 2, 3]);
        for (r, (n, d)) in receipt.evaluations().iter().zip(expected) {
            let (nn, dd) = ratio(r.result());
            assert_eq!(nn * d, n * dd);
            for e in r.executions() {
                assert_eq!(
                    conduit_plot::PortableExpressionProgram::from_canonical_hex(
                        e.source_program_hex()
                    )
                    .unwrap()
                    .evaluate(e.input_canonical())
                    .unwrap(),
                    e.output_canonical()
                );
            }
        }
        for e in receipt.executions() {
            assert_eq!(
                conduit_plot::PortableExpressionProgram::from_canonical_hex(e.source_program_hex())
                    .unwrap()
                    .evaluate(e.input_canonical())
                    .unwrap(),
                e.output_canonical()
            );
        }
    }
}
#[test]
fn conflict_priority_and_ties_are_deterministic_in_all_input_orders() {
    let low = control(
        "source/low",
        "pitch",
        AudioTrajectoryDomain::Frequency,
        1,
        hz(100, 1),
        hz(100, 1),
    );
    let high = control(
        "source/high",
        "pitch",
        AudioTrajectoryDomain::Frequency,
        2,
        hz(200, 1),
        hz(200, 1),
    );
    for controls in [
        vec![low.clone(), high.clone()],
        vec![high.clone(), low.clone()],
    ] {
        let value = set(
            controls.clone(),
            AudioControlConflictPolicy::HighestPriorityRefuseTie,
        );
        let receipt = prepare_set(&value)
            .unwrap()
            .evaluate(&query(time(1, 2)))
            .unwrap();
        let (n, d) = ratio(receipt.evaluations()[receipt.selected_indices()[0]].result());
        assert_eq!(n, 200 * d);
        assert_eq!(receipt.evaluations().len(), 2);
        assert!(matches!(
            prepare_set(&set(controls, AudioControlConflictPolicy::RefuseOverlap))
                .unwrap()
                .evaluate(&query(time(1, 2))),
            Err(AudioControlRefusal::Conflict { .. })
        ));
    }
    let equal = control(
        "source/equal",
        "pitch",
        AudioTrajectoryDomain::Frequency,
        1,
        hz(150, 1),
        hz(150, 1),
    );
    let value = set(
        vec![low.clone(), equal.clone()],
        AudioControlConflictPolicy::HighestPriorityRefuseTie,
    );
    assert!(matches!(
        prepare_set(&value).unwrap().evaluate(&query(time(1, 2))),
        Err(AudioControlRefusal::Conflict { .. })
    ));
    // Equal lower-priority controls do not conflict with a unique higher winner.
    let value = set(
        vec![low, equal, high],
        AudioControlConflictPolicy::HighestPriorityRefuseTie,
    );
    assert_eq!(
        prepare_set(&value)
            .unwrap()
            .evaluate(&query(time(1, 2)))
            .unwrap()
            .selected_indices(),
        [2]
    );
}
#[test]
fn gaps_foreign_anchors_duplicate_scopes_and_bad_profiles_refuse() {
    let c = control(
        "source/a",
        "pitch",
        AudioTrajectoryDomain::Frequency,
        0,
        hz(100, 1),
        hz(200, 1),
    );
    assert!(matches!(
        prepare_set(&set(
            vec![c.clone(), c.clone()],
            AudioControlConflictPolicy::RefuseOverlap
        )),
        Err(AudioControlRefusal::DuplicateScope)
    ));
    let p = prepare_set(&set(vec![c], AudioControlConflictPolicy::RefuseOverlap)).unwrap();
    assert!(matches!(
        p.evaluate(&query(time(2, 1))),
        Err(AudioControlRefusal::Trajectory {
            refusal: AudioTrajectoryRefusal::MissingCoverage,
            ..
        })
    ));
    let foreign = AudioTrajectoryQuery::new(anchor(1, 2), time(1, 2))
        .unwrap()
        .encode()
        .unwrap();
    assert!(matches!(
        p.evaluate(&foreign),
        Err(AudioControlRefusal::Trajectory {
            refusal: AudioTrajectoryRefusal::ForeignAnchor,
            ..
        })
    ));
    let bad = control(
        "source/b",
        "pitch",
        AudioTrajectoryDomain::Power,
        0,
        hz(1, 1),
        hz(2, 1),
    );
    assert!(matches!(
        prepare_set(&set(vec![bad], AudioControlConflictPolicy::RefuseOverlap)),
        Err(AudioControlRefusal::Domain)
    ));
}

#[test]
fn finite_collection_admission_and_evaluation_preserve_all_eight_originals() {
    let controls: Vec<_> = (0..8)
        .map(|i| {
            control(
                &format!("source/{i}"),
                &format!("formant/{i}"),
                AudioTrajectoryDomain::Frequency,
                i,
                hz(1, 1),
                hz(2, 1),
            )
        })
        .collect();
    let value = set(
        controls.clone(),
        AudioControlConflictPolicy::HighestPriorityRefuseTie,
    );
    let receipt = prepare_set(&value)
        .unwrap()
        .evaluate(&query(time(1, 2)))
        .unwrap();
    assert_eq!(receipt.evaluations().len(), 8);
    assert_eq!(receipt.selected_indices().len(), 8);
    assert!(AudioControlTrajectorySet::new(
        AudioControlConflictPolicy::RefuseOverlap,
        BoundedSequence::try_from_iter(Vec::<AudioScopedControlTrajectory>::new()).unwrap(),
        AudioControlGapPolicy::RefuseAnyGap
    )
    .is_err());
    let mut oversized = controls;
    oversized.push(oversized[0].clone());
    assert!(BoundedSequence::<AudioScopedControlTrajectory, 8>::try_from_iter(oversized).is_err());
}

#[test]
fn internal_gap_and_out_of_numeric_profile_identify_the_original_control() {
    let complete = control(
        "source/complete",
        "pitch",
        AudioTrajectoryDomain::Frequency,
        0,
        hz(100, 1),
        hz(200, 1),
    );
    let mut gap = control(
        "source/gap",
        "formant",
        AudioTrajectoryDomain::Frequency,
        0,
        hz(100, 1),
        hz(200, 1),
    );
    let gapped = trajectory(vec![
        segment(
            time(0, 1),
            time(1, 3),
            hz(100, 1),
            hz(100, 1),
            AudioTrajectoryInterpolation::Linear,
        ),
        segment(
            time(2, 3),
            time(1, 1),
            hz(200, 1),
            hz(200, 1),
            AudioTrajectoryInterpolation::Linear,
        ),
    ]);
    gap = AudioScopedControlTrajectory::new(
        gap.control_identity().clone(),
        *gap.domain(),
        *gap.priority(),
        gap.source_scope().clone(),
        gapped,
    )
    .unwrap();
    let value = set(
        vec![complete, gap],
        AudioControlConflictPolicy::RefuseOverlap,
    );
    assert!(matches!(
        prepare_set(&value).unwrap().evaluate(&query(time(1, 2))),
        Err(AudioControlRefusal::Trajectory {
            index: 1,
            refusal: AudioTrajectoryRefusal::MissingCoverage
        })
    ));
    let outside = control(
        "source/out-of-profile",
        "pitch",
        AudioTrajectoryDomain::Frequency,
        0,
        hz(256, 1),
        hz(257, 1),
    );
    assert!(matches!(
        prepare_set(&set(
            vec![outside],
            AudioControlConflictPolicy::RefuseOverlap
        )),
        Err(AudioControlRefusal::Trajectory { index: 0, .. })
    ));
}

#[test]
fn independent_real_formant_step_updates_use_exact_time_not_rounded_frames() {
    let mut controls = Vec::new();
    for (index, (left, right)) in [(500, 700), (1500, 1800), (2500, 2800)]
        .into_iter()
        .enumerate()
    {
        let authored = trajectory(vec![segment(
            time(0, 1),
            time(1, 1),
            hz(left, 1),
            hz(right, 1),
            AudioTrajectoryInterpolation::Step,
        )]);
        controls.push(
            AudioScopedControlTrajectory::new(
                format!("formant/{index}"),
                AudioTrajectoryDomain::Frequency,
                0,
                format!("source/formant/{index}"),
                authored,
            )
            .unwrap(),
        );
    }
    let value = set(controls, AudioControlConflictPolicy::RefuseOverlap);
    let prepared = prepare_set(&value).unwrap();
    for (offset, expected) in [
        (time(1, 44100), [500, 1500, 2500]),
        (time(1, 1), [700, 1800, 2800]),
    ] {
        let receipt = prepared.evaluate(&query(offset)).unwrap();
        for (result, expected) in receipt.evaluations().iter().zip(expected) {
            let (n, d) = ratio(result.result());
            assert_eq!(n, expected * d);
        }
    }
}

#[test]
fn maximum_segment_storage_and_simultaneous_boundaries_remain_finite() {
    let mut controls = Vec::new();
    for i in 0..8 {
        let segments = (0..16)
            .map(|j| {
                segment(
                    time(j, 1),
                    time(j + 1, 1),
                    hz(100 + j, 1),
                    hz(101 + j, 1),
                    AudioTrajectoryInterpolation::Step,
                )
            })
            .collect();
        controls.push(
            AudioScopedControlTrajectory::new(
                format!("formant/{i}"),
                AudioTrajectoryDomain::Frequency,
                u16::MAX,
                format!("source/{i}"),
                trajectory(segments),
            )
            .unwrap(),
        );
    }
    let original = set(controls, AudioControlConflictPolicy::RefuseOverlap);
    let prepared = prepare_set(&original).unwrap();
    for (offset, segment_index, value) in [
        (time(1, 1), 1, 101),
        (time(31, 2), 15, 115),
        (time(16, 1), 15, 116),
    ] {
        let receipt = prepared.evaluate(&query(offset)).unwrap();
        assert_eq!(receipt.evaluations().len(), 8);
        for r in receipt.evaluations() {
            assert_eq!(r.selected_segment(), segment_index);
            let (n, d) = ratio(r.result());
            assert_eq!(n, value * d);
        }
    }
}
