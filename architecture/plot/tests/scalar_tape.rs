//! Same checked corpus and refusal oracle; no machine-code or speed claim.
use conduit_core::ConfigurationValue;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn program(source: &str, name: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn checked_one_three_eight_thirty_two_operations_preserve_bytes_and_refusals_without_allocations() {
    let source = include_str!("fixtures/expression_baseline.conduit");
    for operations in [1u64, 3, 8, 32] {
        let program = program(source, &format!("baseline-{operations}"));
        let mut baseline = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let mut tape = PreparedPortableExpressionEvaluator::new_scalar_tape(&program).unwrap();
        let (instructions, scratch) = tape.scalar_tape_limits().unwrap();
        assert!(instructions <= 128);
        assert!(scratch > 0);
        for n in [0, 1, 7, u64::MAX / (operations + 1)] {
            let bytes = n.to_le_bytes();
            let expected = program.evaluate(&bytes).unwrap();
            assert_eq!(baseline.evaluate(&bytes).unwrap(), expected);
            assert_eq!(tape.evaluate(&bytes).unwrap(), expected);
            let (_, observed) = allocation_probe::observe(|| {
                for _ in 0..32 {
                    assert_eq!(tape.evaluate(&bytes).unwrap(), expected);
                }
            });
            assert_eq!(observed.allocations, 0);
            assert_eq!(observed.reallocations, 0);
        }
        for input in [u64::MAX.to_le_bytes().as_slice(), &[0][..]] {
            assert_eq!(
                tape.evaluate(input).unwrap_err(),
                baseline.evaluate(input).unwrap_err()
            );
        }
    }
}
#[test]
fn unselected_conditional_branch_preserves_lazy_refusal_semantics() {
    let p = program(
        "plot conditional (\n >> value: U64\n result: U64 >>\n) = (. == 0 ? (. + .) : .)\n",
        "conditional",
    );
    let mut tape = PreparedPortableExpressionEvaluator::new_scalar_tape(&p).unwrap();
    for n in [0u64, 1, u64::MAX] {
        let input = n.to_le_bytes();
        assert_eq!(tape.evaluate(&input).unwrap(), p.evaluate(&input).unwrap());
    }
}
#[test]
fn structured_and_text_profiles_refuse_explicitly() {
    let p = program(
        "plot identity (\n >> value: Text <= 8B\n result: Text <= 8B >>\n) = (.)\n",
        "identity",
    );
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new_scalar_tape(&p),
        Err(PortableExpressionEvaluationRefusal::UnsupportedType(_))
    ));
    assert!(PreparedPortableExpressionEvaluator::new(&p).is_ok());
}

#[test]
fn checked_fixed_multiplication_and_boolean_negation_match_reference() {
    let p = program(
        "plot square (
 >> value: U64
 result: U64 >>
) = (. * .)
",
        "square",
    );
    let mut tape = PreparedPortableExpressionEvaluator::new_scalar_tape(&p).unwrap();
    for n in [0u64, 1, 65535, u32::MAX as u64] {
        let bytes = n.to_le_bytes();
        assert_eq!(tape.evaluate(&bytes).unwrap(), p.evaluate(&bytes).unwrap());
    }
    let overflow = u64::MAX.to_le_bytes();
    assert_eq!(
        tape.evaluate(&overflow).unwrap_err(),
        p.evaluate(&overflow).unwrap_err()
    );
    let p = program(
        "plot negate (
 >> value: Boolean
 result: Boolean >>
) = (!.)
",
        "negate",
    );
    let mut tape = PreparedPortableExpressionEvaluator::new_scalar_tape(&p).unwrap();
    for input in [[0u8], [1u8]] {
        assert_eq!(tape.evaluate(&input).unwrap(), p.evaluate(&input).unwrap());
    }
}
