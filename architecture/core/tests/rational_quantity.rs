//! Rational coordinates preserve physical laws independently of realization.
use conduit_core::*;
use core::cmp::Ordering;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn meter() -> Unit {
    Unit::resolve("m").unwrap()
}
fn scalar(n: i128, d: i128, e: i16) -> DefinitionScalar {
    DefinitionScalar::new(n, d, e).unwrap()
}
fn related(root: Unit, symbol: &str, scale: DefinitionScalar) -> Unit {
    Unit::from_definition(
        UnitDefinition::related_exact(
            root.definition(),
            symbol,
            scale,
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap(),
    )
}
fn projection_input<'a>(
    source: &'a RationalQuantityIntegerProjectionRequest,
    target: &'a RationalQuantityInteger,
    facts: &'a [projection::ProjectionFact<'a, RationalQuantityIntegerProjectionDomain>],
) -> projection::ProjectionInput<'a, RationalQuantityIntegerProjectionDomain> {
    use projection::*;
    let text = |value| ProjectionText::new(value).unwrap();
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("quantity/report@1"),
            source: text("quantity/rational@1"),
            target: Some(text("quantity/integer@1")),
            projector: text("numeric/fixed-integer@1"),
            requested_route: text("exact@1"),
            boundary: None,
        },
        source,
        target: Some(target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}
#[test]
fn nonterminating_unit_coordinate_is_exact_and_source_correlated() {
    let root = meter();
    let triple = related(root, "triple", scalar(3, 1, 0));
    let source = Quantity::parse_plot_literal("1m").unwrap();
    assert_eq!(
        source.convert_to_unit(triple),
        Err(QuantityConversionRefusal::Inexact)
    );
    let mut budget = work();
    let receipt = RationalQuantityConversionReceipt::from_checked(
        source,
        triple,
        "1m",
        "triple",
        &mut budget,
    )
    .unwrap();
    let result = receipt.result().unwrap();
    assert_eq!(
        result.coordinate(),
        ExactRational::ratio(1, 3, &mut budget).unwrap()
    );
    assert_eq!(result.unit(), triple);
    assert_eq!(result.dimension(), source.dimension());
    assert_eq!(result.family(), source.family());
    assert_eq!(result.role(), source.role());
    assert_eq!(receipt.source(), source);
    assert_eq!(receipt.original(), "1m");
    assert_eq!(receipt.source_suffix().unit(), root);
    assert_eq!(receipt.target().unit(), triple);
    let original = RationalQuantity::from_quantity(source, &mut budget).unwrap();
    assert_eq!(result.convert_to_unit(root, &mut budget).unwrap(), original);
    assert_eq!(result.compare(original, &mut budget), Ok(Ordering::Equal));
    assert_ne!(result.semantic_digest(), original.semantic_digest());
    assert_eq!(
        RationalQuantityConversionReceipt::from_checked(
            source,
            triple,
            "2m",
            "triple",
            &mut budget,
        ),
        Err(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch)
    );
    assert_eq!(
        RationalQuantityConversionReceipt::from_checked(source, triple, "1m", "m", &mut budget,),
        Err(ExactQuantityConversionRequestRefusal::TargetEvidenceMismatch)
    );
}
#[test]
fn affine_points_deltas_decimal_and_binary_prefixes_keep_their_laws() {
    let mut budget = work();
    for (source, target, n, d) in [
        ("1°F", "°C", -155, 9),
        ("TemperatureDelta(1, °F)", "°C", 5, 9),
        ("0.25turn", "°", 90, 1),
        ("1KiB", "B", 1024, 1),
        ("0.001m", "mm", 1, 1),
    ] {
        let decimal = Quantity::parse_plot_literal(source).unwrap();
        let rational = RationalQuantity::from_quantity(decimal, &mut budget).unwrap();
        let converted = rational
            .convert_to_unit(Unit::resolve(target).unwrap(), &mut budget)
            .unwrap();
        assert_eq!(
            converted.coordinate(),
            ExactRational::ratio(n, d, &mut budget).unwrap()
        );
        assert_eq!(converted.role(), decimal.role());
        assert_eq!(
            converted
                .convert_to_unit(decimal.unit(), &mut budget)
                .unwrap(),
            rational
        );
    }
    let point =
        RationalQuantity::from_quantity(Quantity::parse_plot_literal("1°F").unwrap(), &mut budget)
            .unwrap();
    let delta = RationalQuantity::from_quantity(
        Quantity::parse_plot_literal("TemperatureDelta(1, °F)").unwrap(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        point.compare(delta, &mut budget),
        Err(RationalQuantityRefusal::Physical(
            QuantityConversionRefusal::IncompatibleQuantityRoles,
        ))
    );
}
#[test]
fn dimension_family_role_anchor_and_work_refusals_remain_distinct() {
    let root = meter();
    let mut budget = work();
    let source = RationalQuantity::from_quantity(Quantity::new(1, root), &mut budget).unwrap();
    assert_eq!(
        source.convert_to_unit(Unit::resolve("s").unwrap(), &mut budget),
        Err(RationalQuantityRefusal::Physical(
            QuantityConversionRefusal::IncompatibleDimensions
        ))
    );
    let other_family =
        QuantityFamilyDefinition::new("Altitude", root.dimension(), QuantityRoles::Linear).unwrap();
    let other = Unit::from_definition(
        UnitDefinition::new_exact(
            other_family,
            "alt",
            scalar(1, 1, 0),
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap(),
    );
    assert_eq!(
        source.convert_to_unit(other, &mut budget),
        Err(RationalQuantityRefusal::Physical(
            QuantityConversionRefusal::IncompatibleQuantityFamilies
        ))
    );
    let independent = Unit::from_definition(
        UnitDefinition::new_exact(
            root.family(),
            "independent",
            scalar(1, 1, 0),
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap(),
    );
    assert_eq!(
        source.convert_to_unit(independent, &mut budget),
        Err(RationalQuantityRefusal::Physical(
            QuantityConversionRefusal::Inexact
        ))
    );
    assert_eq!(
        RationalQuantity::new(ExactRational::ONE, root, QuantityRole::Point),
        Err(RationalQuantityRefusal::InvalidRole)
    );
    assert_eq!(
        source.convert_to_unit(root, &mut ExactNumericWork::new(0)),
        Err(RationalQuantityRefusal::Numeric(
            ExactNumericRefusal::WorkLimit
        ))
    );
    let receipt = RationalQuantityConversionReceipt::from_checked(
        Quantity::new(1, root),
        root,
        "1m",
        "m",
        &mut ExactNumericWork::new(0),
    )
    .unwrap();
    assert_eq!(receipt.original(), "1m");
    assert_eq!(
        receipt.result(),
        Err(RationalQuantityRefusal::Numeric(
            ExactNumericRefusal::WorkLimit
        ))
    );
}
#[test]
fn canonical_transport_rejects_forged_role_unit_and_rational_capsules() {
    let mut budget = work();
    let original = RationalQuantity::new(
        ExactRational::ratio(-1, 3, &mut budget).unwrap(),
        meter(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut bytes = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
    let length = original.encode(&mut bytes).unwrap();
    assert_eq!(
        RationalQuantity::decode(&bytes[..length], &mut budget),
        Ok(original)
    );
    let mut forged = bytes;
    forged[1] = 1;
    assert_eq!(
        RationalQuantity::decode(&forged[..length], &mut budget),
        Err(RationalQuantityRefusal::InvalidRole)
    );
    forged = bytes;
    forged[2 + UNIT_ENCODED_LEN - 1] ^= 1;
    assert!(matches!(
        RationalQuantity::decode(&forged[..length], &mut budget),
        Err(RationalQuantityRefusal::Unit(_))
    ));
    assert!(RationalQuantity::decode(&bytes[..length + 1], &mut budget).is_err());
    assert_eq!(
        RationalQuantity::decode(&bytes[..length], &mut ExactNumericWork::new(0)),
        Err(RationalQuantityRefusal::Numeric(
            ExactNumericRefusal::WorkLimit
        ))
    );
}
#[test]
fn independent_unit_and_integer_projections_preserve_physical_contract() {
    use conduit_core::projection::*;
    let mut budget = work();
    let original = Quantity::parse_plot_literal("0.001m").unwrap();
    let receipt = RationalQuantityConversionReceipt::from_checked(
        original,
        Unit::resolve("mm").unwrap(),
        "0.001m",
        "mm",
        &mut budget,
    )
    .unwrap();
    let request = RationalQuantityIntegerProjectionRequest {
        source: receipt.result().unwrap(),
        target_kind: PrimitiveInfoKind::U16,
    };
    let target = request.project().unwrap();
    assert_eq!(target.coordinate().unsigned().unwrap(), 1);
    assert_eq!(target.unit().dimension(), original.dimension());
    assert_eq!(target.unit().family(), original.family());
    assert_eq!(target.role(), original.role());
    let domain = RationalQuantityIntegerProjectionDomain;
    let facts = domain.facts();
    let policy = RationalQuantityIntegerProjectionPolicy;
    let report = ProjectionReport::new(
        &domain,
        &policy,
        projection_input(&request, &target, &facts),
    )
    .unwrap();
    assert_eq!(report.require_exact(), Ok(&target));
    assert!(domain.validate(
        &request,
        Some(&target),
        &facts,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    assert!(RationalQuantityIntegerProjectionPolicy.permits(&request, &target, &facts));
    assert!(!domain.validate(
        &request,
        Some(&target),
        &facts[..2],
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    let wrong_unit = RationalQuantityIntegerProjectionRequest {
        source: RationalQuantity::new(ExactRational::ONE, meter(), QuantityRole::Linear).unwrap(),
        ..request
    }
    .project()
    .unwrap();
    assert!(matches!(
        ProjectionReport::new(
            &domain,
            &policy,
            projection_input(&request, &wrong_unit, &facts)
        ),
        Err(ProjectionRefusal::Domain)
    ));
    let text = |value| ProjectionText::new(value).unwrap();
    let wrong_law = [
        ProjectionFact::Preserved {
            obligation: text("exact-denotation"),
        },
        ProjectionFact::Transformed {
            obligation: text("numeric-representation"),
            law: text("unit-coordinate-conversion@1"),
        },
        ProjectionFact::Preserved {
            obligation: text("physical-coordinate-contract"),
        },
    ];
    assert!(matches!(
        ProjectionReport::new(
            &domain,
            &policy,
            projection_input(&request, &target, &wrong_law)
        ),
        Err(ProjectionRefusal::Domain)
    ));
    let wrong = RationalQuantityIntegerProjectionRequest {
        source: RationalQuantity::new(
            ExactRational::ONE,
            Unit::resolve("s").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        target_kind: request.target_kind,
    }
    .project()
    .unwrap();
    assert!(!domain.validate(
        &request,
        Some(&wrong),
        &facts,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    let wrong_kind = RationalQuantityIntegerProjectionRequest {
        target_kind: PrimitiveInfoKind::I64,
        ..request
    }
    .project()
    .unwrap();
    assert!(!domain.validate(
        &request,
        Some(&wrong_kind),
        &facts,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    let fractional = RationalQuantityIntegerProjectionRequest {
        source: RationalQuantity::from_quantity(original, &mut budget).unwrap(),
        ..request
    };
    assert_eq!(
        fractional.project(),
        Err(ExactIntegerProjectionRefusal::Numeric(
            ExactNumericRefusal::InexactProjection
        ))
    );
}
#[test]
fn powers_and_decimal_parts_obey_exact_domain_extent_and_work_limits() {
    let mut budget = work();
    assert_eq!(
        ExactRational::from_i128(3)
            .checked_power(-3, &mut budget)
            .unwrap(),
        ExactRational::ratio(1, 27, &mut budget).unwrap()
    );
    assert_eq!(
        ExactRational::from_i128(-1).checked_power(i16::MIN, &mut budget),
        Ok(ExactRational::ONE)
    );
    assert_eq!(
        ExactRational::ZERO.checked_power(-1, &mut budget),
        Err(ExactNumericRefusal::DivisionByZero)
    );
    assert_eq!(
        ExactRational::from_decimal_parts(1, 2000, &mut budget),
        Err(ExactNumericRefusal::ExtentExceeded)
    );
    assert_eq!(
        ExactRational::from_i128(10).checked_power(1000, &mut ExactNumericWork::new(0)),
        Err(ExactNumericRefusal::WorkLimit)
    );
    let huge = ExactRational::from_decimal_parts(1, 1000, &mut budget)
        .unwrap()
        .checked_add(ExactRational::ONE, &mut budget)
        .unwrap();
    assert_eq!(
        huge,
        ExactRational::parse_decimal(&format!("1{}1", "0".repeat(999)), &mut budget).unwrap()
    );
}
