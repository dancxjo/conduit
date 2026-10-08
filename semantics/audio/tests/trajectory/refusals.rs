use super::*;

#[test]
fn ordering_coincident_and_domain_refusals_are_source_owned() {
    let ordinary = |start, end| {
        segment(
            time(start, 1),
            time(end, 1),
            hz(100, 1),
            hz(200, 1),
            AudioTrajectoryInterpolation::Linear,
        )
    };
    assert!(matches!(
        prepare(&trajectory(vec![ordinary(1, 1)])),
        Err(AudioTrajectoryRefusal::InvalidSegment)
    ));
    assert!(matches!(
        prepare(&trajectory(vec![ordinary(2, 1)])),
        Err(AudioTrajectoryRefusal::InvalidSegment)
    ));
    assert!(matches!(
        prepare(&trajectory(vec![ordinary(0, 2), ordinary(1, 3)])),
        Err(AudioTrajectoryRefusal::UnorderedOrOverlapping)
    ));
    assert!(matches!(
        prepare(&trajectory(vec![ordinary(2, 3), ordinary(0, 1)])),
        Err(AudioTrajectoryRefusal::UnorderedOrOverlapping)
    ));
    let mixed = segment(
        time(0, 1),
        time(1, 1),
        hz(100, 1),
        amplitude(1, 1),
        AudioTrajectoryInterpolation::Linear,
    );
    assert!(matches!(
        prepare(&trajectory(vec![mixed])),
        Err(AudioTrajectoryRefusal::InvalidSegment)
    ));
    let amplitude_segment = segment(
        time(1, 1),
        time(2, 1),
        amplitude(0, 1),
        amplitude(1, 1),
        AudioTrajectoryInterpolation::Linear,
    );
    assert!(matches!(
        prepare(&trajectory(vec![ordinary(0, 1), amplitude_segment])),
        Err(AudioTrajectoryRefusal::MixedDomain)
    ));
}

#[test]
fn gaps_outside_and_foreign_anchors_refuse_without_extrapolation() {
    let authored = trajectory(vec![
        segment(
            time(1, 1),
            time(2, 1),
            hz(100, 1),
            hz(100, 1),
            AudioTrajectoryInterpolation::Step,
        ),
        segment(
            time(3, 1),
            time(4, 1),
            hz(200, 1),
            hz(200, 1),
            AudioTrajectoryInterpolation::Step,
        ),
    ]);
    let prepared = prepare(&authored).unwrap();
    for time in [time(0, 1), time(2, 1), time(5, 2), time(5, 1)] {
        assert!(matches!(
            prepared.evaluate(&query(time)),
            Err(AudioTrajectoryRefusal::MissingCoverage)
        ));
    }
    for foreign in [anchor(2, 1), anchor(1, 2)] {
        let bytes = AudioTrajectoryQuery::new(foreign, time(3, 2))
            .unwrap()
            .encode()
            .unwrap();
        assert!(matches!(
            prepared.evaluate(&bytes),
            Err(AudioTrajectoryRefusal::ForeignAnchor)
        ));
    }
    assert_eq!(
        ratio(prepared.evaluate(&query(time(4, 1))).unwrap().result()),
        (200, 1)
    );
}

#[test]
fn shared_boundary_is_right_continuous_and_final_endpoint_retains_authored_fraction() {
    let authored = trajectory(vec![
        segment(
            time(0, 1),
            time(1, 1),
            hz(100, 1),
            hz(150, 1),
            AudioTrajectoryInterpolation::Step,
        ),
        segment(
            time(1, 1),
            time(2, 1),
            hz(200, 1),
            hz(200, 2),
            AudioTrajectoryInterpolation::Step,
        ),
    ]);
    let prepared = prepare(&authored).unwrap();
    let boundary = prepared.evaluate(&query(time(1, 1))).unwrap();
    assert_eq!(boundary.selected_segment(), 1);
    assert_eq!(ratio(boundary.result()), (200, 1));
    assert_eq!(
        ratio(prepared.evaluate(&query(time(2, 1))).unwrap().result()),
        (200, 2)
    );
}

#[test]
fn full_domain_values_are_not_silently_reduced_into_arithmetic_profile() {
    let authored = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        hz(u64::MAX, u64::MAX),
        hz(100, 1),
        AudioTrajectoryInterpolation::Step,
    )]);
    assert!(matches!(
        prepare(&authored),
        Err(AudioTrajectoryRefusal::Admission(_))
    ));
    let authored = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        hz(100, 1),
        hz(200, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let prepared = prepare(&authored).unwrap();
    // Exactly 1/2 second, but authored denominator exceeds profile. No reduction.
    assert!(matches!(
        prepared.evaluate(&query(time(256, 512))),
        Err(AudioTrajectoryRefusal::Admission(_))
    ));
    assert!(prepared.evaluate(&[]).is_err());
    assert!(AudioExactTimeOffset::new(0, 1).is_err());
    assert!(AudioTrajectoryQuantity::frequency(1, 0).is_err());
    assert!(AudioTrajectoryQuantity::cycle(1, 0).is_err());
    assert!(AudioTrajectoryQuantity::amplitude(0, 1).is_err());
}

#[test]
fn segment_count_and_provenance_bounds_are_native_admission() {
    use conduit_plot::rust_binding::BoundedSequence;
    let one = segment(
        time(0, 1),
        time(1, 1),
        hz(100, 1),
        hz(200, 1),
        AudioTrajectoryInterpolation::Linear,
    );
    assert!(
        BoundedSequence::<AudioTrajectorySegment, 16>::try_from_iter(vec![one.clone(); 17])
            .is_err()
    );
    assert!(AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "".into(),
        None
    )
    .is_err());
    assert!(AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Derived,
        "method".into(),
        Some("".into())
    )
    .is_err());
    let all = (0..16)
        .map(|i| {
            segment(
                time(i, 1),
                time(i + 1, 1),
                hz(100, 1),
                hz(200, 1),
                AudioTrajectoryInterpolation::Linear,
            )
        })
        .collect();
    let authored = trajectory(all);
    let prepared = prepare(&authored).unwrap();
    let result = prepared.evaluate(&query(time(31, 2))).unwrap();
    assert_eq!(result.selected_segment(), 15);
    assert_eq!(result.executions().len(), 97);
    let max_input_type = result
        .executions()
        .iter()
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
    let total_frames: usize = result
        .executions()
        .iter()
        .map(|execution| execution.input_canonical().len() + execution.output_canonical().len())
        .sum();
    println!(
        "largest Source input Type={max_input_type}B; retained executed frame bytes={total_frames}"
    );
    println!("AudioQuantityTrajectory Type={}B, 16segment value={}B; query Type={}B value={}B; sourceexecutions={}",AudioQuantityTrajectory::semantic_type().unwrap().canonical_bytes().unwrap().len(),authored.encode().unwrap().len(),AudioTrajectoryQuery::semantic_type().unwrap().canonical_bytes().unwrap().len(),result.query_canonical().len(),result.executions().len());
}

#[test]
fn forged_typed_zero_denominator_refuses_native_boundary() {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};
    fn zero_denominator(value: &StructuredInfoValue) -> StructuredInfoValue {
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            return value.clone();
        };
        let fields = fields
            .iter()
            .map(|field| {
                let next = if field.name() == "denominator" {
                    StructuredInfoValue::leaf(
                        field.value().value_type().clone(),
                        0_u64.to_le_bytes().to_vec(),
                    )
                    .unwrap()
                } else {
                    zero_denominator(field.value())
                };
                StructuredFieldValue::new(field.name(), next).unwrap()
            })
            .collect();
        StructuredInfoValue::record(value.value_type().clone(), fields).unwrap()
    }
    let authored = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        hz(100, 1),
        hz(200, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let prepared = prepare(&authored).unwrap();
    let value = AudioTrajectoryQuery::new(anchor(1, 1), time(1, 2))
        .unwrap()
        .into_structured()
        .unwrap();
    let forged = zero_denominator(&value).canonical_bytes().unwrap();
    assert!(matches!(
        prepared.evaluate(&forged),
        Err(AudioTrajectoryRefusal::Admission(_))
    ));
}
