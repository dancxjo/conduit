//! Incoming numerical loss remains separate from mathematical source meaning.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn q(s: &str) -> ExactRational {
    ExactRational::parse_decimal(s, &mut work()).unwrap()
}
fn projected(source: ExactRational, actual: f64, limit: ExactRational) -> Binary64ProjectedInput {
    Binary64ProjectedInput::new(
        source,
        IeeeF64::from_bits(actual.to_bits()),
        limit,
        &mut work(),
    )
    .unwrap()
}
fn limits() -> Binary64AlgorithmLimits {
    Binary64AlgorithmLimits {
        maximum_steps: 32,
        maximum_retained_bytes: 1_000_000,
    }
}
fn template(input_limit: ExactRational, output_limit: ExactRational) -> PreparedBinary64Algorithm {
    PreparedBinary64Algorithm::prepare(
        &[
            Binary64Operation::ProjectedConstant(Box::new(projected(
                ExactRational::ONE,
                1.0,
                input_limit,
            ))),
            Binary64Operation::Constant(Box::new(ExactRational::ZERO)),
            Binary64Operation::Subtract { left: 0, right: 1 },
        ],
        Binary64Rounding::NearestTiesEven,
        output_limit,
        limits(),
        &mut work(),
    )
    .unwrap()
}

#[test]
fn incoming_error_survives_cancellation_and_total_tolerance_refusal_without_allocation() {
    let incoming = projected(
        q("9007199254740993"),
        9007199254740992.0,
        ExactRational::ONE,
    );
    let subtract = q("9007199254740992");
    let arguments = [
        Binary64AlgorithmInput::Projected(&incoming),
        Binary64AlgorithmInput::Exact(&subtract),
    ];
    let initial = template(ExactRational::ONE, ExactRational::ZERO);
    let mut runner = PreparedBinary64AlgorithmRunner::new(&initial, &[0, 1], 3_000_000).unwrap();
    let before = runner.last_success().clone();
    let exact = projected(ExactRational::from_i128(3), 3.0, ExactRational::ZERO);
    let one = ExactRational::ONE;
    let recovery = [
        Binary64AlgorithmInput::Projected(&exact),
        Binary64AlgorithmInput::Exact(&one),
    ];
    let (correct, allocations) = observe(|| {
        matches!(
            runner.execute_inputs(&arguments, &mut work()),
            Err(Binary64AlgorithmRefusal::Projection(
                Binary64Refusal::ToleranceExceeded
            ))
        ) && runner.last_success() == &before
            && runner
                .execute_inputs(&recovery, &mut work())
                .is_ok_and(|p| {
                    p.source() == ExactRational::from_i128(2)
                        && p.target().bits() == 2f64.to_bits()
                        && p.error() == Binary64ErrorClaim::ExactDistance(ExactRational::ZERO)
                })
    });
    assert!(correct);
    assert_eq!(allocations, 0);

    let loose = template(ExactRational::ONE, ExactRational::ONE);
    let mut runner = PreparedBinary64AlgorithmRunner::new(&loose, &[0, 1], 3_000_000).unwrap();
    let (correct, allocations) = observe(|| {
        runner
            .execute_inputs(&arguments, &mut work())
            .is_ok_and(|p| {
                p.source() == ExactRational::ONE
                    && p.target().bits() == 0
                    && p.error() == Binary64ErrorClaim::ExactDistance(ExactRational::ONE)
                    && p.steps()[0].semantic_value() == incoming.source()
                    && p.steps()[0].projection().source() == subtract
            })
    });
    assert!(correct);
    assert_eq!(allocations, 0);
}

#[test]
fn input_policy_is_sealed_and_exact_and_projected_slots_never_coerce() {
    let initial = template(ExactRational::ZERO, ExactRational::ONE);
    let mut runner = PreparedBinary64AlgorithmRunner::new(&initial, &[0, 1], 3_000_000).unwrap();
    let incoming = projected(
        q("9007199254740993"),
        9007199254740992.0,
        ExactRational::ONE,
    );
    let zero = ExactRational::ZERO;
    let before = runner.last_success().clone();
    let (correct, allocations) = observe(|| {
        matches!(
            runner.execute_inputs(
                &[
                    Binary64AlgorithmInput::Projected(&incoming),
                    Binary64AlgorithmInput::Exact(&zero)
                ],
                &mut work()
            ),
            Err(Binary64AlgorithmRefusal::Projection(
                Binary64Refusal::ToleranceExceeded
            ))
        ) && runner.last_success() == &before
            && matches!(
                runner.execute(&[zero, zero], &mut work()),
                Err(Binary64AlgorithmRefusal::InvalidProgram)
            )
    });
    assert!(correct);
    assert_eq!(allocations, 0);
    let exact_template = PreparedBinary64Algorithm::prepare(
        &[Binary64Operation::Constant(Box::new(zero))],
        Binary64Rounding::NearestTiesEven,
        ExactRational::ZERO,
        limits(),
        &mut work(),
    )
    .unwrap();
    let mut exact = PreparedBinary64AlgorithmRunner::new(&exact_template, &[0], 3_000_000).unwrap();
    let perfect = projected(zero, 0.0, zero);
    assert!(matches!(
        exact.execute_inputs(&[Binary64AlgorithmInput::Projected(&perfect)], &mut work()),
        Err(Binary64AlgorithmRefusal::InvalidProgram)
    ));
}

#[test]
fn incoming_distance_negative_limit_and_nonfinite_representation_are_checked() {
    assert_eq!(
        Binary64ProjectedInput::new(
            ExactRational::ONE,
            IeeeF64::from_bits(0),
            ExactRational::ZERO,
            &mut work()
        ),
        Err(Binary64AlgorithmRefusal::Projection(
            Binary64Refusal::ToleranceExceeded
        ))
    );
    assert_eq!(
        Binary64ProjectedInput::new(
            ExactRational::ONE,
            IeeeF64::from_bits(0),
            ExactRational::from_i128(-1),
            &mut work()
        ),
        Err(Binary64AlgorithmRefusal::Projection(
            Binary64Refusal::NegativeTolerance
        ))
    );
    for bits in [f64::INFINITY.to_bits(), f64::NAN.to_bits()] {
        assert_eq!(
            Binary64ProjectedInput::new(
                ExactRational::ZERO,
                IeeeF64::from_bits(bits),
                ExactRational::ONE,
                &mut work()
            ),
            Err(Binary64AlgorithmRefusal::Projection(
                Binary64Refusal::Numeric(ExactNumericRefusal::OutOfRange)
            ))
        );
    }
    let signed = projected(ExactRational::ZERO, -0.0, ExactRational::ZERO);
    let result = PreparedBinary64Algorithm::prepare(
        &[Binary64Operation::ProjectedConstant(Box::new(signed))],
        Binary64Rounding::NearestTiesEven,
        ExactRational::ZERO,
        limits(),
        &mut work(),
    )
    .unwrap();
    assert_eq!(result.source(), ExactRational::ZERO);
    assert_eq!(result.target().bits(), 1 << 63);
    assert_eq!(
        result.error(),
        Binary64ErrorClaim::ExactDistance(ExactRational::ZERO)
    );
}

#[test]
fn incoming_representation_is_never_reprojected_from_source_in_another_rounding_mode() {
    for input in [
        projected(
            q("9007199254740993"),
            9007199254740992.0,
            ExactRational::ONE,
        ),
        projected(
            q("-9007199254740993"),
            -9007199254740992.0,
            ExactRational::ONE,
        ),
    ] {
        for mode in [
            Binary64Rounding::NearestTiesEven,
            Binary64Rounding::TowardZero,
            Binary64Rounding::TowardPositive,
            Binary64Rounding::TowardNegative,
        ] {
            let p = PreparedBinary64Algorithm::prepare(
                &[Binary64Operation::ProjectedConstant(Box::new(input))],
                mode,
                ExactRational::ONE,
                limits(),
                &mut work(),
            )
            .unwrap();
            assert_eq!(p.source(), input.source());
            assert_eq!(p.target(), input.target());
            assert_eq!(
                p.error(),
                Binary64ErrorClaim::ExactDistance(ExactRational::ONE)
            );
        }
    }
}

#[test]
fn projected_input_heap_payload_is_inside_the_admitted_storage_bound() {
    let input = projected(ExactRational::ONE, 1.0, ExactRational::ZERO);
    let program = [Binary64Operation::ProjectedConstant(Box::new(input))];
    let prepared = PreparedBinary64Algorithm::prepare(
        &program,
        Binary64Rounding::NearestTiesEven,
        ExactRational::ZERO,
        limits(),
        &mut work(),
    )
    .unwrap();
    let required = prepared.retained_payload_bytes() as u32;
    let admitted = Binary64AlgorithmLimits {
        maximum_steps: 1,
        maximum_retained_bytes: required,
    };
    assert!(PreparedBinary64Algorithm::prepare(
        &program,
        Binary64Rounding::NearestTiesEven,
        ExactRational::ZERO,
        admitted,
        &mut work()
    )
    .is_ok());
    let refused = Binary64AlgorithmLimits {
        maximum_retained_bytes: required - 1,
        ..admitted
    };
    assert_eq!(
        PreparedBinary64Algorithm::prepare(
            &program,
            Binary64Rounding::NearestTiesEven,
            ExactRational::ZERO,
            refused,
            &mut work()
        ),
        Err(Binary64AlgorithmRefusal::StorageLimit)
    );
}

fn rational(q: ExactRational) -> Vec<u8> {
    let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let n = q.encode(&mut bytes).unwrap();
    let mut result = (n as u16).to_le_bytes().to_vec();
    result.extend_from_slice(&bytes[..n]);
    result
}
fn operation(source: i128, limit: i128) -> Vec<u8> {
    let mut result = vec![7];
    result.extend(rational(ExactRational::from_i128(source)));
    result.extend_from_slice(&2f64.to_bits().to_le_bytes());
    result.extend(rational(ExactRational::from_i128(limit)));
    result
}
fn forged(
    property: &StructuredConfigurationValue,
    before: &[u8],
    after: &[u8],
) -> StructuredConfigurationValue {
    let root = StructuredInfoValue::from_canonical_bytes(property.canonical_value()).unwrap();
    let StructuredInfoValueShape::Leaf(payload) = root.shape() else {
        panic!("property leaf")
    };
    let positions: Vec<_> = payload
        .windows(before.len())
        .enumerate()
        .filter_map(|(i, v)| (v == before).then_some(i))
        .collect();
    assert_eq!(positions.len(), 1);
    let mut changed = payload.to_vec();
    changed.splice(
        positions[0]..positions[0] + before.len(),
        after.iter().copied(),
    );
    let value = StructuredInfoValue::leaf(root.value_type().clone(), changed).unwrap();
    StructuredConfigurationValue::new(property.profile().clone(), value.canonical_bytes().unwrap())
        .unwrap()
}

#[test]
fn canonical_property_recomputes_incoming_loss_and_refuses_forged_source_or_ceiling() {
    let input = projected(ExactRational::from_i128(3), 2.0, ExactRational::ONE);
    let prepared = PreparedNumericalRealization::prepare_binary64(
        &MathematicalValue::rational(ExactRational::from_i128(3)),
        &[Binary64Operation::ProjectedConstant(Box::new(input))],
        Binary64Rounding::NearestTiesEven,
        ExactRational::ONE,
        limits(),
        &mut work(),
    )
    .unwrap();
    let property = prepared.property().unwrap();
    assert_eq!(
        PreparedNumericalRealization::from_property(&property, &mut work()).unwrap(),
        prepared
    );
    let weak = forged(&property, &operation(3, 1), &operation(3, 0));
    assert_eq!(
        PreparedNumericalRealization::from_property(&weak, &mut work()),
        Err(NumericalRealizationRefusal::Binary64(
            Binary64AlgorithmRefusal::Projection(Binary64Refusal::ToleranceExceeded)
        ))
    );
    let different_source = forged(&property, &operation(3, 1), &operation(2, 1));
    assert_eq!(
        PreparedNumericalRealization::from_property(&different_source, &mut work()),
        Err(NumericalRealizationRefusal::SourceMismatch)
    );
}
