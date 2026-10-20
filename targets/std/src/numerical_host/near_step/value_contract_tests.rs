//! Consuming tolerance is enforced through the ordinary sealed value contract.
use super::*;
fn contract(kind: &str, limit: &str) -> Result<CheckedValueContract, ConstraintDefinitionError> {
    let schema = conduit_semantic_catalog::near_quantity_type();
    CheckedValueContract::new(
        schema.profile().unwrap().value_kind().clone(),
        NEAR_QUANTITY_EVIDENCE_MAX_BYTES as u32,
        vec![ValueConstraint::NearQuantity {
            source_kind: kind_id(kind),
            limit: Box::new(Quantity::parse_plot_literal(limit).unwrap()),
            schema: schema.canonical_bytes().unwrap(),
        }],
    )
}
#[test]
fn sealed_near_constraints_verify_actual_loss_without_runtime_allocation() {
    let mut work = ExactNumericWork::new(NEAR_NUMERICAL_WORK);
    let source = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut work).unwrap(),
        Unit::resolve("m").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut input = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(source, &mut input).unwrap();
    input.truncate(n);
    let weak = contract(DISTANCE_INFO_ID, "0.0001m").unwrap();
    let tight = contract(DISTANCE_INFO_ID, "0.000000001m").unwrap();
    let exact = contract(DISTANCE_INFO_ID, "0m").unwrap();
    let foreign = contract(FREQUENCY_INFO_ID, "0.0001Hz").unwrap();
    for algorithm in [
        NearQuantityAlgorithm::ExactRational,
        NearQuantityAlgorithm::Binary32,
    ] {
        let mut back =
            NearQuantityBack::new(Quantity::parse_plot_literal("0.0001m").unwrap(), algorithm)
                .unwrap();
        back.evaluate(&input).unwrap();
        let bytes = back.state.encoding.encoded();
        let guard = crate::allocation_probe::begin();
        for _ in 0..16 {
            assert_eq!(weak.validate(bytes), Ok(()));
            assert_eq!(
                tight.validate(bytes).is_ok(),
                algorithm == NearQuantityAlgorithm::ExactRational
            );
            assert_eq!(
                exact.validate(bytes).is_ok(),
                algorithm == NearQuantityAlgorithm::ExactRational
            );
            assert!(matches!(
                foreign.validate(bytes),
                Err(ValueConstraintRefusal::NearQuantity(
                    NearQuantityEvidenceRefusal::Physical
                ))
            ));
        }
        assert_eq!(guard.finish(), 0);
        assert!(weak.validate(&bytes[..bytes.len() - 1]).is_err());
    }
}
#[test]
fn invalid_tolerance_or_unrelated_schema_cannot_be_admitted() {
    for (kind, limit) in [
        (FREQUENCY_INFO_ID, "0.01m"),
        (FREQUENCY_INFO_ID, "-0.01Hz"),
        (TEMPERATURE_INFO_ID, "0.01°C"),
    ] {
        assert_eq!(
            contract(kind, limit),
            Err(ConstraintDefinitionError::InvalidNearQuantity)
        );
    }
    let mut valid = contract(DISTANCE_INFO_ID, "0.0001m").unwrap();
    let ValueConstraint::NearQuantity { schema, .. } = &mut valid.constraints[0] else {
        unreachable!()
    };
    *schema = StructuredInfoType::leaf(kind_id(COUNT_INFO_ID))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        valid.validate_definition(),
        Err(ConstraintDefinitionError::InvalidNearQuantity)
    );
}
