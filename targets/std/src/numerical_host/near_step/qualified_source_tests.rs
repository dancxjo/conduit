//! Source/evidence/runner composition; not an installed Plan/Play acceptance claim.
use super::*;
use conduit_plot::{
    PreparedQualifiedQuantityBinary64, QualifiedQuantityBinary64Limits,
    QualifiedQuantityBinary64Refusal as R, *,
};

fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn limits() -> QualifiedQuantityBinary64Limits {
    QualifiedQuantityBinary64Limits {
        algorithm: Binary64AlgorithmLimits {
            maximum_steps: 32,
            maximum_retained_bytes: 2_000_000,
        },
        maximum_retained_payload_bytes: 8_000_000,
        maximum_numerical_work: 1_000_000_000,
    }
}
fn program(limit: &str, body: &str) -> PortableExpressionProgram {
    program_input("0.001Hz", limit, body)
}
fn program_input(input: &str, limit: &str, body: &str) -> PortableExpressionProgram {
    program_kind("Frequency", input, limit, body)
}
fn program_kind(kind: &str, input: &str, limit: &str, body: &str) -> PortableExpressionProgram {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = format!("plot fold (\n >> value: {kind} near {input}\n result: {kind} near {limit} >>\n) = ({body})\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "fold", &profile).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn prepare(program: &PortableExpressionProgram) -> PreparedQualifiedQuantityBinary64 {
    PreparedQualifiedQuantityBinary64::new(
        program,
        Unit::resolve("Hz").unwrap(),
        QuantityRole::Linear,
        Binary64Rounding::NearestTiesEven,
        limits(),
        &mut work(),
    )
    .unwrap()
}
fn incoming(value: &str, limit: &str, algorithm: NearQuantityAlgorithm) -> NearQuantityBack {
    let mut back =
        NearQuantityBack::new(Quantity::parse_plot_literal(limit).unwrap(), algorithm).unwrap();
    back.evaluate(&Quantity::parse_plot_literal(value).unwrap().encode())
        .unwrap();
    back
}

#[test]
fn checked_source_and_actual_incoming_representation_reach_the_ordered_runner_without_growth() {
    let broad_program = program("0.2Hz", "(. + 10000000000000000Hz) - 10000000000000000Hz");
    let tight_program = program("0.01Hz", "(. + 10000000000000000Hz) - 10000000000000000Hz");
    let canonical = broad_program.canonical_bytes().unwrap();
    let tenth = ExactRational::ratio(1, 10, &mut work()).unwrap();
    let hz = Unit::resolve("Hz").unwrap();
    for algorithm in [
        NearQuantityAlgorithm::ExactRational,
        NearQuantityAlgorithm::Binary32,
    ] {
        let supplier = incoming("0.1Hz", "0.001Hz", algorithm);
        let bytes = supplier.state.encoding.encoded();
        let mut broad = prepare(&broad_program);
        let mut tight = prepare(&tight_program);
        assert_eq!(broad.canonical_program(), canonical);
        assert_ne!(broad.program_digest(), tight.program_digest());
        let capacity = broad.retained_payload_bytes();
        let guard = crate::allocation_probe::begin();
        for _ in 0..4 {
            let mut used = work();
            let result = broad.execute(bytes, &mut used).unwrap();
            assert_eq!(result.source().coordinate(), tenth);
            assert_eq!(result.source().unit(), hz);
            assert_eq!(result.incoming_evidence(), bytes);
            assert_eq!(result.algorithm().target().bits(), 0);
            assert_eq!(
                result.algorithm().error(),
                Binary64ErrorClaim::ExactDistance(tenth)
            );
            assert_eq!(result.algorithm().steps().len(), 5);
            if algorithm == NearQuantityAlgorithm::Binary32 {
                let Binary64Operation::ProjectedConstant(input) =
                    result.algorithm().steps()[0].operation()
                else {
                    panic!()
                };
                assert_eq!(input.source(), tenth);
                assert_eq!(input.target().bits(), 0x3fb9_9999_a000_0000);
            } else {
                assert!(
                    matches!(result.algorithm().steps()[0].operation(), Binary64Operation::Constant(value) if **value == tenth)
                );
            }
            assert!(used.spent() > 0);
            let mut refused_work = work();
            assert!(matches!(
                tight.execute(bytes, &mut refused_work),
                Err(R::Numerical(Binary64AlgorithmRefusal::Projection(
                    Binary64Refusal::ToleranceExceeded
                )))
            ));
            assert!(refused_work.spent() > 0);
        }
        assert_eq!(guard.finish(), 0);
        assert_eq!(broad.retained_payload_bytes(), capacity);
        assert!(broad
            .execute(&bytes[..bytes.len() - 1], &mut work())
            .is_err());
        assert!(broad.execute(bytes, &mut work()).is_ok());
    }
}

#[test]
fn input_evidence_physical_frame_and_coordinate_policy_are_independent() {
    let mut bound = prepare(&program("0.2Hz", ". + 1Hz"));
    for (value, limit, coordinate) in [("0.1m", "0.001m", false), ("0.1kHz", "0.001kHz", true)] {
        let supplier = incoming(value, limit, NearQuantityAlgorithm::Binary32);
        let result = bound.execute(supplier.state.encoding.encoded(), &mut work());
        if coordinate {
            assert!(matches!(
                result,
                Err(R::ExplicitCoordinateConversionRequired)
            ));
        } else {
            assert!(matches!(
                result,
                Err(R::Evidence(NearQuantityEvidenceRefusal::Physical))
            ));
        }
    }
    let weak = incoming("16777217Hz", "1Hz", NearQuantityAlgorithm::Binary32);
    assert!(matches!(
        bound.execute(weak.state.encoding.encoded(), &mut work()),
        Err(R::Evidence(
            NearQuantityEvidenceRefusal::InsufficientTolerance
        ))
    ));
}

#[test]
fn source_binding_refuses_hidden_unit_changes_scalar_reinterpretation_and_false_operator_types() {
    let mixed = program("0.2Hz", ". + 1kHz");
    assert!(matches!(
        PreparedQualifiedQuantityBinary64::new(
            &mixed,
            Unit::resolve("Hz").unwrap(),
            QuantityRole::Linear,
            Binary64Rounding::NearestTiesEven,
            limits(),
            &mut work()
        ),
        Err(R::ExplicitCoordinateConversionRequired)
    ));
    let scalar = program("0.2Hz", ". * (0.000001 * 0.000001)");
    assert!(matches!(
        PreparedQualifiedQuantityBinary64::new(
            &scalar,
            Unit::resolve("Hz").unwrap(),
            QuantityRole::Linear,
            Binary64Rounding::NearestTiesEven,
            limits(),
            &mut work()
        ),
        Err(R::UnsupportedOperation)
    ));
    let mut forged = program("0.2Hz", ". + 1Hz");
    let PortableExpressionOperation::QualifiedQuantity { value, .. } = &mut forged.root.operation
    else {
        panic!()
    };
    let PortableExpressionOperation::Binary { operator, .. } = &mut value.operation else {
        panic!()
    };
    *operator = BinaryOperator::Multiply;
    assert!(forged.canonical_bytes().is_ok());
    assert!(matches!(
        PreparedQualifiedQuantityBinary64::new(
            &forged,
            Unit::resolve("Hz").unwrap(),
            QuantityRole::Linear,
            Binary64Rounding::NearestTiesEven,
            limits(),
            &mut work()
        ),
        Err(R::Program)
    ));
}

#[test]
fn exact_incoming_contract_does_not_forbid_the_computations_own_admitted_rounding() {
    let mut bound = prepare(&program_input(
        "0Hz",
        "0.2Hz",
        "(. + 10000000000000000Hz) - 10000000000000000Hz",
    ));
    let exact = incoming("0.1Hz", "0Hz", NearQuantityAlgorithm::ExactRational);
    let rounded = incoming("0.1Hz", "0.001Hz", NearQuantityAlgorithm::Binary32);
    let result = bound
        .execute(exact.state.encoding.encoded(), &mut work())
        .unwrap();
    assert_eq!(result.algorithm().target().bits(), 0);
    assert_ne!(result.algorithm().error().maximum(), ExactRational::ZERO);
    assert!(matches!(
        bound.execute(rounded.state.encoding.encoded(), &mut work()),
        Err(R::Evidence(
            NearQuantityEvidenceRefusal::InsufficientTolerance
        ))
    ));
}

#[test]
fn widening_retains_the_actual_negative_zero_after_incoming_underflow() {
    let mut bound = prepare(&program("0.2Hz", ". + 0Hz"));
    let denominator = ExactRational::from_i128(10)
        .checked_power(100, &mut work())
        .unwrap();
    let tiny = ExactRational::from_i128(-1)
        .checked_divide(denominator, &mut work())
        .unwrap();
    let source =
        RationalQuantity::new(tiny, Unit::resolve("Hz").unwrap(), QuantityRole::Linear).unwrap();
    let mut wire = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let length = encode_rational_quantity_value(source, &mut wire).unwrap();
    let mut supplier = NearQuantityBack::new(
        Quantity::parse_plot_literal("0.001Hz").unwrap(),
        NearQuantityAlgorithm::Binary32,
    )
    .unwrap();
    supplier.evaluate(&wire[..length]).unwrap();
    let guard = crate::allocation_probe::begin();
    let result = bound
        .execute(supplier.state.encoding.encoded(), &mut work())
        .unwrap();
    let Binary64Operation::ProjectedConstant(input) = result.algorithm().steps()[0].operation()
    else {
        panic!()
    };
    assert_eq!(input.target().bits(), 1 << 63);
    assert_ne!(input.source(), ExactRational::ZERO);
    assert_ne!(result.algorithm().error().maximum(), ExactRational::ZERO);
    assert_eq!(guard.finish(), 0);
}

#[test]
fn coordinate_choice_is_independent_of_the_units_used_to_state_error_bounds() {
    let source = program("0.2Hz", ". + 1kHz");
    let khz = Unit::resolve("kHz").unwrap();
    let mut bound = PreparedQualifiedQuantityBinary64::new(
        &source,
        khz,
        QuantityRole::Linear,
        Binary64Rounding::NearestTiesEven,
        limits(),
        &mut work(),
    )
    .unwrap();
    let supplier = incoming("0.1kHz", "0.001kHz", NearQuantityAlgorithm::Binary32);
    let expected = ExactRational::ratio(1, 5000, &mut work()).unwrap();
    let guard = crate::allocation_probe::begin();
    let result = bound
        .execute(supplier.state.encoding.encoded(), &mut work())
        .unwrap();
    assert_eq!(result.source().unit(), khz);
    assert_eq!(result.algorithm().maximum_error(), expected);
    assert_eq!(guard.finish(), 0);
    assert_eq!(bound.coordinate_unit(), khz);
    let foreign = program_kind("Quantity", "0.001m", "0.2m", ". + 1m");
    assert!(matches!(
        PreparedQualifiedQuantityBinary64::new(
            &foreign,
            Unit::resolve("Hz").unwrap(),
            QuantityRole::Linear,
            Binary64Rounding::NearestTiesEven,
            limits(),
            &mut work()
        ),
        Err(R::Physical)
    ));
}

#[test]
fn difference_coordinates_keep_their_role_and_do_not_receive_point_offsets() {
    let source = program_kind(
        "TemperatureDelta",
        "TemperatureDelta(0.001, °C)",
        "TemperatureDelta(0.2, °C)",
        ". + TemperatureDelta(1, °C)",
    );
    let unit = Unit::resolve("°C").unwrap();
    let mut bound = PreparedQualifiedQuantityBinary64::new(
        &source,
        unit,
        QuantityRole::Delta,
        Binary64Rounding::NearestTiesEven,
        limits(),
        &mut work(),
    )
    .unwrap();
    let supplier = incoming(
        "TemperatureDelta(0.1, °C)",
        "TemperatureDelta(0.001, °C)",
        NearQuantityAlgorithm::Binary32,
    );
    let expected = ExactRational::ratio(11, 10, &mut work()).unwrap();
    let guard = crate::allocation_probe::begin();
    let result = bound
        .execute(supplier.state.encoding.encoded(), &mut work())
        .unwrap();
    assert_eq!(result.source().coordinate(), expected);
    assert_eq!(result.source().role(), QuantityRole::Delta);
    assert_eq!(result.source().unit(), unit);
    assert_eq!(guard.finish(), 0);
    assert_eq!(bound.coordinate_role(), QuantityRole::Delta);
    assert!(matches!(
        PreparedQualifiedQuantityBinary64::new(
            &source,
            unit,
            QuantityRole::Point,
            Binary64Rounding::NearestTiesEven,
            limits(),
            &mut work()
        ),
        Err(R::UnsupportedRole)
    ));
}

#[test]
fn domain_faults_and_exhausted_shared_work_are_charged_and_remain_distinct() {
    let supplier = incoming("0.1Hz", "0.001Hz", NearQuantityAlgorithm::Binary32);
    let mut singular = prepare(&program("0.2Hz", ". / 0"));
    let normal = program("0.2Hz", ". + 1Hz");
    let mut small = limits();
    small.maximum_numerical_work = 1;
    let mut exhausted = PreparedQualifiedQuantityBinary64::new(
        &normal,
        Unit::resolve("Hz").unwrap(),
        QuantityRole::Linear,
        Binary64Rounding::NearestTiesEven,
        small,
        &mut work(),
    )
    .unwrap();
    let guard = crate::allocation_probe::begin();
    let mut charged = work();
    assert!(matches!(
        singular.execute(supplier.state.encoding.encoded(), &mut charged),
        Err(R::Numerical(Binary64AlgorithmRefusal::Projection(
            Binary64Refusal::Numeric(ExactNumericRefusal::DivisionByZero)
        )))
    ));
    assert!(charged.spent() > 0);
    let mut short = work();
    assert!(matches!(
        exhausted.execute(supplier.state.encoding.encoded(), &mut short),
        Err(R::Evidence(NearQuantityEvidenceRefusal::WorkLimit))
    ));
    assert_eq!(short.spent(), 1);
    assert_eq!(guard.finish(), 0);
}
