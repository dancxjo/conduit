//! Baseline the existing prepared evaluator, without implementing optimization.
use conduit_core::ConfigurationValue;
use conduit_plot::*;
use std::{hint::black_box, time::Instant};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const SOURCE: &str = include_str!("fixtures/expression_baseline.conduit");
const ITERATIONS: u64 = 1024;
#[test]
fn checked_corpus_preserves_parity_overflow_and_source_with_cold_warm_rows() {
    for row in 0..3 {
        let start = Instant::now();
        let syntax = parse_syntax_document(SOURCE);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        assert_eq!(syntax.round_trip(), SOURCE);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let parse_check_ns = start.elapsed().as_nanos();
        for operations in [1_u64, 3, 8, 32] {
            let root = format!("baseline-{operations}");
            let start = Instant::now();
            let authored =
                expand_canonical_plot_for_authoring(&checked, &root, &ProfileCatalog::new())
                    .unwrap();
            let expand_ns = start.elapsed().as_nanos();
            let ConfigurationValue::Text(encoded) =
                &authored.expanded.gears[0].configuration[0].value
            else {
                panic!("checked program")
            };
            let start = Instant::now();
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
            let prepare_ns = start.elapsed().as_nanos();
            let input = 7_u64.to_le_bytes();
            let expected = 7 * (operations + 1);
            let reference = program.evaluate(&input).unwrap();
            assert_eq!(reference, expected.to_le_bytes());
            assert_eq!(evaluator.evaluate(&input).unwrap(), reference);
            assert!(program.evaluate(&u64::MAX.to_le_bytes()).is_err());
            assert!(evaluator.evaluate(&u64::MAX.to_le_bytes()).is_err());
            assert!(evaluator.evaluate(&[0]).is_err());
            // One explicit untimed warm-up is above; timed loops include decode/assert.
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                let bytes = evaluator.evaluate(black_box(&input)).unwrap();
                assert_eq!(bytes, reference);
            }
            let warm_ns = start.elapsed().as_nanos();
            let (_, allocations) = allocation_probe::observe(|| {
                for _ in 0..ITERATIONS {
                    assert_eq!(evaluator.evaluate(&input).unwrap(), reference);
                }
            });
            assert_eq!(allocations.allocations, 0);
            assert_eq!(allocations.reallocations, 0);
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                let mut value = black_box(7_u64);
                for _ in 0..black_box(operations) {
                    value = value.checked_add(black_box(7)).unwrap();
                }
                assert_eq!(black_box(value), expected);
            }
            let native_ns = start.elapsed().as_nanos();
            println!(
                "EXPRESSION_BASELINE_ROW={}",
                serde_json::json!({"row":row,"operations":operations,"iterations":ITERATIONS,"parse_check_full_corpus_ns":parse_check_ns,"expand_selected_plot_ns":expand_ns,"decode_prepare_ns":prepare_ns,"warm_evaluate_decode_assert_ns":warm_ns,"native_checked_add_assert_ns":native_ns,"source_document_id":checked.source_document_id.as_str(),"checked_plot_id":authored.expanded.checked_plot_id.as_str(),"expanded_plot_id":authored.expanded.expanded_plot_id.as_str(),"program_hex_bytes":encoded.len(),"output_bytes":8,"tree_layout":"balanced","warm_allocations":allocations.allocations,"warm_reallocations":allocations.reallocations,"profile":"local test profile; no kernel/browser timing"})
            );
        }
    }
}

#[test]
fn left_associated_32_operations_refuse_the_current_program_depth_budget() {
    let expression = format!(". {}", "+ . ".repeat(32));
    let source =
        format!("plot beyond-depth (\n >> value: U64\n result: U64 >>\n) = ({expression})\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let error =
        expand_canonical_plot_for_authoring(&checked, "beyond-depth", &ProfileCatalog::new())
            .unwrap_err();
    assert_eq!(error.code, "CND-FRM-046");
}
