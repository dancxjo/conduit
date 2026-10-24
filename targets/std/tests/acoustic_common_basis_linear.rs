//! Already-admitted Audio Source inputs execute through ordinary Plan/Play.
#[path = "common/angle_host.rs"]
mod installed_expression;
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[test]
fn actual_common_basis_source_matches_preparation_through_kernel() {
    let source = [
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/trajectory_common_basis_types.conduit"),
        include_str!("../../../semantics/audio/trajectory_common_basis.conduit"),
    ]
    .join("\n");
    let (mut host, plan) = installed_expression::prepare(
        &source,
        "audio/trajectory-common-basis-interpolate",
        "value",
    );
    for (left, right) in [
        (
            AudioTrajectoryQuantity::frequency(1, 270).unwrap(),
            AudioTrajectoryQuantity::frequency(1, 3010).unwrap(),
        ),
        (
            AudioTrajectoryQuantity::amplitude(1000, 0).unwrap(),
            AudioTrajectoryQuantity::amplitude(1000, 900).unwrap(),
        ),
    ] {
        let anchor = AudioTrajectoryAnchor::new(
            AudioOriginIdentity::new(1).unwrap(),
            AudioTimelineIdentity::new(1).unwrap(),
        )
        .unwrap();
        let time = |n| AudioExactTimeOffset::new(20000, n).unwrap();
        let segment = AudioTrajectorySegment::new(
            time(4000),
            AudioTrajectoryInterpolation::Linear,
            left,
            right,
            time(0),
        )
        .unwrap();
        let trajectory = AudioQuantityTrajectory::new(
            anchor.clone(),
            AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
            AudioTrajectoryOutside::Refuse,
            AudioTrajectoryProvenance::new(
                AudioTrajectoryProvenanceKind::Authored,
                "common-basis-kernel-conformance".into(),
                Some("v1".into()),
            )
            .unwrap(),
            BoundedSequence::try_from_iter([segment]).unwrap(),
        )
        .unwrap();
        let prepared = PreparedAudioQuantityTrajectory::new_with_linear_profile(
            &trajectory.encode().unwrap(),
            AudioLinearTrajectoryProfile::CommonBasisU32,
        )
        .unwrap();
        for q in [0, 2000, 4000] {
            let result = prepared
                .evaluate(
                    &AudioTrajectoryQuery::new(anchor.clone(), time(q))
                        .unwrap()
                        .encode()
                        .unwrap(),
                )
                .unwrap();
            let execution = result.executions().last().unwrap();
            let output = installed_expression::run(
                &mut host,
                &plan,
                "value",
                vec![execution.input_canonical().to_vec()],
            );
            assert_eq!(output, vec![execution.output_canonical().to_vec()]);
        }
    }
}
