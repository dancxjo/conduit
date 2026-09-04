use super::*;
use alloc::vec;

fn quantity_definition() -> CharacteristicDefinition {
    CharacteristicDefinition {
        characteristic_id: CharacteristicId::from("llm/context-byte-ceiling@1"),
        subject: CharacteristicSubject::Realization,
        stability: CharacteristicStability::Stable,
        value_kind: CharacteristicValueKind::UnsignedQuantity {
            unit: CharacteristicUnit::Bytes,
            maximum: 1_048_576,
        },
        human_name: "Context byte ceiling".into(),
        help: "Maximum admitted context bytes for this realization.".into(),
    }
}

#[test]
fn quantity_validation_keeps_units_subject_and_stability_exact() {
    let definition = quantity_definition();
    assert_eq!(
        definition.validate_realization_value(&CharacteristicValue::UnsignedQuantity {
            value: 65_536,
            unit: CharacteristicUnit::Bytes,
        }),
        Ok(())
    );
    assert_eq!(
        definition.validate_realization_value(&CharacteristicValue::UnsignedQuantity {
            value: 65_536,
            unit: CharacteristicUnit::Tokens,
        }),
        Err(CharacteristicDefinitionError::UnitMismatch)
    );
    let mut wrong_subject = definition.clone();
    wrong_subject.subject = CharacteristicSubject::ComputeTopology;
    assert_eq!(
        wrong_subject.validate_realization_value(&CharacteristicValue::UnsignedQuantity {
            value: 65_536,
            unit: CharacteristicUnit::Bytes,
        }),
        Err(CharacteristicDefinitionError::WrongSubject)
    );
    let mut observed = definition;
    observed.stability = CharacteristicStability::CurrentObservation;
    assert_eq!(
        observed.validate_realization_value(&CharacteristicValue::UnsignedQuantity {
            value: 65_536,
            unit: CharacteristicUnit::Bytes,
        }),
        Err(CharacteristicDefinitionError::ObservedFactAdvertisedAsStable)
    );
}

#[test]
fn planner_characteristic_quantities_converge_only_for_reviewed_units() {
    assert_eq!(
        CharacteristicQuantity::from_quantity(
            Quantity::new(2, QuantityUnit::Kibibyte),
            CharacteristicUnit::Bytes,
        ),
        Ok(CharacteristicQuantity {
            value: 2_048,
            unit: CharacteristicUnit::Bytes,
        })
    );
    assert_eq!(
        CharacteristicQuantity {
            value: 2_048,
            unit: CharacteristicUnit::Bytes,
        }
        .quantity(),
        Some(Quantity::new(2_048, QuantityUnit::Byte))
    );
    assert_eq!(
        CharacteristicQuantity::from_quantity(
            Quantity::new(440, QuantityUnit::Hertz),
            CharacteristicUnit::Millihertz,
        ),
        Ok(CharacteristicQuantity {
            value: 440_000,
            unit: CharacteristicUnit::Millihertz,
        })
    );
    assert_eq!(
        CharacteristicQuantity {
            value: 1,
            unit: CharacteristicUnit::Tokens,
        }
        .quantity(),
        None
    );
    assert_eq!(
        CharacteristicQuantity::from_quantity(
            Quantity::new(1, QuantityUnit::Millisecond),
            CharacteristicUnit::Microseconds,
        ),
        Ok(CharacteristicQuantity {
            value: 1_000,
            unit: CharacteristicUnit::Microseconds,
        })
    );
    assert_eq!(
        CharacteristicQuantity::from_quantity(
            Quantity::new(1, QuantityUnit::One),
            CharacteristicUnit::Tokens,
        ),
        Err(CharacteristicDefinitionError::UnsupportedQuantityUnit)
    );
    assert_eq!(
        CharacteristicQuantity::from_quantity(
            Quantity::new(-1, QuantityUnit::Byte),
            CharacteristicUnit::Bytes,
        ),
        Err(CharacteristicDefinitionError::NegativeQuantity)
    );
    assert_eq!(
        CharacteristicQuantity::from_quantity(
            Quantity::new(1, QuantityUnit::Second),
            CharacteristicUnit::Hertz,
        ),
        Err(CharacteristicDefinitionError::QuantityConversion(
            QuantityConversionRefusal::IncompatibleDimensions
        ))
    );
}

#[test]
fn finite_categories_reject_unknown_labels_and_implicit_ordering() {
    let definition = CharacteristicDefinition {
        characteristic_id: CharacteristicId::from("presentation/text-layout@1"),
        subject: CharacteristicSubject::Realization,
        stability: CharacteristicStability::Stable,
        value_kind: CharacteristicValueKind::Categorical {
            allowed_labels: vec!["fixed-cell".into(), "proportional".into()],
            ordered: false,
        },
        human_name: "Text layout".into(),
        help: "Stable layout behavior of a presentation realization.".into(),
    };
    assert_eq!(
        definition.validate_realization_value(&CharacteristicValue::Categorical("fluid".into())),
        Err(CharacteristicDefinitionError::UnknownLabel)
    );
    assert_eq!(
        definition.categorical_rank(&CharacteristicValue::Categorical("fixed-cell".into())),
        None
    );
}
