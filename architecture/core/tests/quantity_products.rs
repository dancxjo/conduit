use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn q(text: &str) -> RationalQuantity {
    RationalQuantity::from_quantity(Quantity::parse_plot_literal(text).unwrap(), &mut work())
        .unwrap()
}
fn custom(name: &str, origin: &str) -> RationalQuantity {
    let family = QuantityFamilyDefinition::new(
        name,
        Unit::resolve("m").unwrap().dimension(),
        QuantityRoles::Linear,
    )
    .unwrap();
    let unit = UnitDefinition::new(family, origin, 1, 0, 1, 0, PrefixPolicy::NONE).unwrap();
    RationalQuantity::new(
        ExactRational::ONE,
        Unit::from_definition(unit),
        QuantityRole::Linear,
    )
    .unwrap()
}
#[test]
fn exact_reference_scales_derive_dimensions_without_named_family_coercion() {
    let result = q("2km").checked_quotient(q("4s"), &mut work()).unwrap();
    assert_eq!(result.coordinate(), ExactRational::from_i128(500));
    assert_eq!(
        result.dimension(),
        q("1m").dimension().quotient(q("1s").dimension()).unwrap()
    );
    let product = q("2km").checked_product(q("3m"), &mut work()).unwrap();
    assert_eq!(product.coordinate(), ExactRational::from_i128(6000));
    assert_eq!(product.dimension(), q("1m").dimension().power(2).unwrap());
    assert_eq!(
        product,
        q("3m").checked_product(q("2km"), &mut work()).unwrap()
    );
    assert_eq!(
        product.unit(),
        q("2m")
            .checked_product(q("3m"), &mut work())
            .unwrap()
            .unit()
    );
}
#[test]
fn nominal_families_independent_origins_and_rebindings_retain_custody() {
    let energy = custom("Energy", "root");
    let torque = custom("Torque", "root");
    let other_origin = custom("Energy", "other-root");
    let retained = energy.checked_product(q("1s"), &mut work()).unwrap();
    let nominal = torque.checked_product(q("1s"), &mut work()).unwrap();
    let rebound = other_origin.checked_product(q("1s"), &mut work()).unwrap();
    assert_eq!(retained.dimension(), nominal.dimension());
    assert_ne!(retained.family(), nominal.family());
    assert_eq!(retained.family(), rebound.family());
    assert_ne!(
        retained.unit().reference_anchor(),
        rebound.unit().reference_anchor()
    );
    assert!(retained
        .convert_to_unit(rebound.unit(), &mut work())
        .is_err());
    assert_eq!(
        retained,
        energy.checked_product(q("1s"), &mut work()).unwrap()
    );
}
#[test]
fn points_deltas_zero_and_finite_work_refuse_explicitly() {
    let point = q("21°C");
    let delta =
        RationalQuantity::new(ExactRational::ONE, point.unit(), QuantityRole::Delta).unwrap();
    for invalid in [point, delta] {
        assert_eq!(
            invalid.checked_product(q("1m"), &mut work()),
            Err(RationalQuantityRefusal::InvalidRole)
        );
        assert_eq!(
            q("1m").checked_quotient(invalid, &mut work()),
            Err(RationalQuantityRefusal::InvalidRole)
        );
    }
    assert!(q("1m").checked_quotient(q("0s"), &mut work()).is_err());
    assert!(q("1m")
        .checked_product(q("1s"), &mut ExactNumericWork::new(0))
        .is_err());
}
#[test]
fn derived_capsules_round_trip_and_refuse_modified_metadata() {
    let result = q("1m").checked_quotient(q("3s"), &mut work()).unwrap();
    let mut bytes = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
    let length = result.encode(&mut bytes).unwrap();
    assert_eq!(
        RationalQuantity::decode(&bytes[..length], &mut work()).unwrap(),
        result
    );
    bytes[20] ^= 1;
    assert!(RationalQuantity::decode(&bytes[..length], &mut work()).is_err());
}

#[test]
fn dimension_and_numeric_capacity_limits_remain_checked() {
    let dimension = q("1m").dimension().power(16).unwrap();
    let family =
        QuantityFamilyDefinition::new("PowerSixteen", dimension, QuantityRoles::Linear).unwrap();
    let unit = Unit::from_definition(
        UnitDefinition::new(family, "sixteenth", 1, 0, 1, 0, PrefixPolicy::NONE).unwrap(),
    );
    let value = RationalQuantity::new(ExactRational::ONE, unit, QuantityRole::Linear).unwrap();
    assert!(matches!(
        value.checked_product(q("1m"), &mut work()),
        Err(RationalQuantityRefusal::Unit(UnitRefusal::Definition(
            PhysicalDefinitionRefusal::InvalidDimensionPower
        )))
    ));
    // Build the largest power-of-two coordinate that fits the finite profile,
    // then ensure product overflow refuses rather than narrowing that value.
    let mut coordinate = ExactRational::ONE;
    loop {
        match coordinate.checked_multiply(ExactRational::from_i128(2), &mut work()) {
            Ok(next) => coordinate = next,
            Err(_) => break,
        }
    }
    let huge = RationalQuantity::new(
        coordinate,
        Unit::resolve("m").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    assert!(huge.checked_product(q("2s"), &mut work()).is_err());
}
