use conduit_core::{ExactDecimalQuantity as Exact, EXACT_DECIMAL_QUANTITY_INFO_ID};
use conduit_plot::{
    check_expression, parse_syntax_document, BackStatement, CheckedExpressionType, CordStage,
    ExpressionTypeContext, PortableExpressionProgram, PreparedPortableExpressionEvaluator,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
};
thread_local! {
    static TRACK:Cell<bool>=const {Cell::new(false)};
    static COUNT:Cell<usize>=const {Cell::new(0)};
}
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|count| count.set(count.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|count| count.set(count.get() + 1));
        }
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
fn prepared_exact_transport_admits_twenty_bytes_and_never_grows_during_evaluation() {
    let source="plot transport (\n >> input: ExactQuantity\n output: ExactQuantity >>\n) {\n input >> (.) >> output\n}\n";
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let BackStatement::Cord(cord) = &parsed.plots[0].back[0] else {
        panic!("cord");
    };
    let CordStage::PureExpression(expression) = &cord.stages[1] else {
        panic!("expression");
    };
    let input = CheckedExpressionType::semantic(EXACT_DECIMAL_QUANTITY_INFO_ID);
    let context = ExpressionTypeContext {
        input: &input,
        immutable_values: &BTreeMap::new(),
        structured_types: &BTreeMap::new(),
        literal_types: &BTreeMap::new(),
        numeric_types: &BTreeSet::new(),
        semantic_kinds: &BTreeMap::new(),
    };
    let checked = check_expression(&expression.syntax, &context).unwrap();
    let program = PortableExpressionProgram::from_checked(&checked).unwrap();
    assert_eq!(program.maximum_prepared_input_bytes(), Ok(20));
    assert_eq!(program.maximum_prepared_output_bytes(), Ok(20));
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let encoded = Exact::parse_plot_literal("1Qm³").unwrap().encode();
    let mut forged = encoded;
    forged[0] = 2;
    assert!(evaluator.evaluate(&forged).is_err());
    assert!(evaluator.evaluate(&encoded[..9]).is_err());
    COUNT.with(|count| count.set(0));
    TRACK.with(|track| track.set(true));
    let mut correct = true;
    for _ in 0..1000 {
        correct &= evaluator
            .evaluate(&encoded)
            .is_ok_and(|bytes| bytes == encoded);
    }
    TRACK.with(|track| track.set(false));
    assert!(correct);
    assert_eq!(COUNT.with(Cell::get), 0);
}
