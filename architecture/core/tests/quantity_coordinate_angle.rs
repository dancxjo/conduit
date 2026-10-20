//! The general coordinate boundary must retain the explicit circle-reference law.
use conduit_core::{projection::*, *};
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}

#[test]
fn turn_to_radian_coordinates_keep_symbolic_pi_and_replayable_reference_evidence() {
    for (target_unit, coefficient) in [
        (Unit::Radian, 2),
        (Quantity::parse_plot_literal("1mrad").unwrap().unit(), 2000),
    ] {
        reference_projection(target_unit, ExactRational::from_i128(coefficient));
    }
}
fn reference_projection(target_unit: Unit, coefficient: ExactRational) {
    let source = Quantity::parse_plot_literal("1turn").unwrap().encode();
    let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let mut verifier = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let mut target = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut scratch = target.clone();
    let limits = QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS;
    let mut evidence = vec![0; limits.total_bytes];
    let policy = QuantityCoordinateConversionPolicy { target_unit };
    let (_, allocations) = allocation_probe::observe(|| {
        let receipt = converter
            .execute(&source, target_unit, &mut target, &mut work())
            .unwrap();
        let bytes = &target[..receipt.encoded_bytes()];
        let value = MathematicalQuantityView::decode(bytes, &mut work()).unwrap();
        assert_eq!(value.unit(), target_unit);
        assert_eq!(
            value
                .coordinate()
                .rational_pi_coefficient(&mut work())
                .unwrap(),
            Some(coefficient)
        );
        let domain = QuantityCoordinateConversionDomain::new();
        let facts = domain.facts(&receipt);
        assert!(facts.iter().any(|fact| matches!(fact,
            ProjectionFact::Transformed { law, .. } if law.as_str() == TURN_RADIAN_REFERENCE_LAW)));
        let target = QuantityValueProfile::Mathematical
            .decode(QUANTITY_INFO_ID, bytes, &mut work())
            .unwrap();
        let mut forged = domain.facts(&receipt);
        forged[2] = ProjectionFact::Transformed {
            obligation: ProjectionText::new("unit-coordinates").unwrap(),
            law: ProjectionText::new(MATHEMATICAL_QUANTITY_CONVERSION_LAW).unwrap(),
        };
        assert!(!domain.validate(
            &receipt,
            Some(&target),
            &forged,
            &[],
            &[],
            ProjectionMechanism::Completed
        ));
        let ids = QuantityCoordinateContentIds::new(&source, bytes, target_unit);
        let report = ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis: ids.basis(),
                source: &receipt,
                target: Some(&target),
                facts: &facts,
                native: &[],
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
        .unwrap();
        let n = encode_projection_evidence(
            &report,
            &QuantityCoordinateConversionEvidenceCodec::new(policy),
            limits,
            &mut evidence,
        )
        .unwrap();
        verifier
            .verify_evidence(
                &source,
                bytes,
                policy,
                200_000_000,
                ids.basis(),
                &evidence[..n],
                &mut scratch,
                &mut ExactNumericWork::new(900_000_000),
            )
            .unwrap();
    });
    assert_eq!(allocations, 0);
}
