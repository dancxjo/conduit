//! A coordinate identity must not overflow through an unneeded reference Unit.
use conduit_core::*;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}

#[test]
fn identical_scaled_unit_retains_maximum_extent_without_reference_overflow() {
    let meter = Unit::resolve("m").unwrap();
    let scaled = Unit::from_definition(
        UnitDefinition::related_exact(
            meter.definition(),
            "ten-meter",
            DefinitionScalar::new(10, 1, 0).unwrap(),
            DefinitionScalar::new(0, 1, 0).unwrap(),
            PrefixPolicy::NONE,
        )
        .unwrap(),
    );
    let coordinate = ExactRational::from_i128(2)
        .checked_power(4095, &mut work())
        .unwrap();
    let source = RationalQuantity::new(coordinate, scaled, QuantityRole::Linear).unwrap();
    let mut identity_work = ExactNumericWork::new(256);
    assert_eq!(
        source.convert_to_unit(scaled, &mut identity_work),
        Ok(source)
    );
    assert_eq!(identity_work.spent(), 256);
    // An actual change of coordinates still needs an out-of-extent product.
    assert_eq!(
        source.convert_to_unit(meter, &mut work()),
        Err(RationalQuantityRefusal::Numeric(
            ExactNumericRefusal::ExtentExceeded
        ))
    );
}

#[test]
fn fractional_time_and_affine_point_delta_identities_preserve_capsules() {
    for (unit, role) in [
        (Unit::resolve("s").unwrap(), QuantityRole::Linear),
        (Unit::resolve("°C").unwrap(), QuantityRole::Point),
        (Unit::resolve("°C").unwrap(), QuantityRole::Delta),
    ] {
        let source =
            RationalQuantity::new(ExactRational::ratio(1, 3, &mut work()).unwrap(), unit, role)
                .unwrap();
        let result = source
            .convert_to_unit(unit, &mut ExactNumericWork::new(256))
            .unwrap();
        assert_eq!(result, source);
        assert_eq!(result.semantic_digest(), source.semantic_digest());
        let mut original_bytes = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
        let mut result_bytes = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
        let length = source.encode(&mut original_bytes).unwrap();
        assert_eq!(result.encode(&mut result_bytes).unwrap(), length);
        assert_eq!(&original_bytes[..length], &result_bytes[..length]);
    }
}

#[test]
fn identity_is_still_fuel_bounded_and_incompatible_units_still_refuse() {
    let source = RationalQuantity::new(
        ExactRational::ONE,
        Unit::resolve("s").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    for maximum in [0, 255] {
        let mut budget = ExactNumericWork::new(maximum);
        assert_eq!(
            source.convert_to_unit(source.unit(), &mut budget),
            Err(RationalQuantityRefusal::Numeric(
                ExactNumericRefusal::WorkLimit
            ))
        );
        assert_eq!(budget.spent(), 0);
    }
    assert_eq!(
        source.convert_to_unit(Unit::resolve("m").unwrap(), &mut work()),
        Err(RationalQuantityRefusal::Physical(
            QuantityConversionRefusal::IncompatibleDimensions
        ))
    );
}
