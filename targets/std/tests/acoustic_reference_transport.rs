//! Candidate-dependent development proof of authored acoustic reference transport.
use conduit_audio::*;
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue, Unit,
};
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod common_acoustic_reference;
#[path = "common_acoustic_value/kernel.rs"]
mod kernel;
use common_acoustic_reference::{field, prepare};

#[test]
fn authored_db_reference_and_original_ratio_cross_the_actual_child_kernel() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(check_db)
        .unwrap()
        .join()
        .unwrap();
}
fn check_db() {
    let fixture = include_str!("fixtures/acoustic-referenced-db-value.conduit");
    for power in [false, true] {
        let basis = AudioDecibelBasis::new(
            if power {
                AudioDecibelConvention::PowerTenLog10
            } else {
                AudioDecibelConvention::AmplitudeTwentyLog10
            },
            AudioDecibelReference::new(
                7,
                "reference/exact".into(),
                3,
                AudioTrajectoryProvenance::new(
                    AudioTrajectoryProvenanceKind::Authored,
                    "declared reference".into(),
                    Some("v1".into()),
                )
                .unwrap(),
                if power {
                    AudioDecibelReferenceRole::Power
                } else {
                    AudioDecibelReferenceRole::AmplitudeMagnitude
                },
                Unit::Second,
            )
            .unwrap(),
        )
        .unwrap();
        for (n, d, exponent) in [(100, 1, Some(2)), (1, 1000, Some(-3)), (0, 1, None)] {
            let ratio = if power {
                AudioLevelRatio::power(d, n)
            } else {
                AudioLevelRatio::amplitude(d, n)
            }
            .unwrap();
            let original = AudioReferencedLevelRatio::new(basis.clone(), ratio).unwrap();
            let encoded = original.clone().encode().unwrap();
            let (plan, front, pair) = prepare(
                fixture,
                "AcousticDecibelStage",
                "acoustic-referenced-db-value",
            );
            let admission = PreparedExactPowerOfTenDecibels::new()
                .unwrap()
                .ratio_to_decibels(&encoded)
                .unwrap();
            assert_eq!(admission.original_canonical(), encoded);
            let execution = admission.executions().last().unwrap();
            assert!(plan.fragments[0].placements.iter().any(|gear| gear.configuration.iter().any(|entry| matches!(&entry.value, ConfigurationValue::Text(program) if program == execution.source_program_hex()))));
            let input_type = plan.fragments[0]
                .placements
                .iter()
                .find_map(|gear| {
                    let ConfigurationValue::Text(encoded) = &gear.configuration.first()?.value
                    else {
                        return None;
                    };
                    let program = PortableExpressionProgram::from_canonical_hex(encoded).ok()?;
                    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape()
                    else {
                        return None;
                    };
                    fields
                        .iter()
                        .any(|field| field.name() == "eligible")
                        .then_some(program.input_type)
                })
                .expect("the authored front consumes the admitted owner profile");
            let input = StructuredInfoValue::record(
                input_type,
                vec![
                    StructuredFieldValue::new(
                        "eligible",
                        StructuredInfoValue::from_canonical_bytes(execution.input_canonical())
                            .unwrap(),
                    )
                    .unwrap(),
                    StructuredFieldValue::new(
                        "original",
                        StructuredInfoValue::from_canonical_bytes(&encoded).unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .canonical_bytes()
            .unwrap();
            let result = kernel::run(plan, &front, pair, vec![], input, false).unwrap();
            let result = StructuredInfoValue::from_canonical_bytes(&result).unwrap();
            assert_eq!(
                AudioReferencedLevelRatio::decode(
                    &field(&result, "original").canonical_bytes().unwrap()
                )
                .unwrap(),
                original
            );
            let projected =
                AudioDecibelLevel::decode(&field(&result, "projected").canonical_bytes().unwrap())
                    .unwrap();
            assert_eq!(projected.basis(), &basis);
            match (projected.value(), exponent) {
                (AudioDecibelValue::Finite(value), Some(exponent)) => {
                    assert_eq!(
                        *value.numerator_db(),
                        exponent * if power { 10 } else { 20 }
                    );
                    assert_eq!(*value.denominator(), 1);
                }
                (AudioDecibelValue::NegativeInfinity, None) => {}
                _ => panic!("exact reference"),
            }
        }
    }
}
