//! Capacity accounting is checked against actual requested live allocations.
use conduit_core::{ConfigurationValue, KindId, StructuredFieldType, StructuredInfoType};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
static LIVE: AtomicIsize = AtomicIsize::new(0);
struct Probe;
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        if !pointer.is_null() {
            LIVE.fetch_add(layout.size() as isize, Ordering::SeqCst);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        System.dealloc(pointer, layout);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let result = System.realloc(pointer, layout, new_size);
        if !result.is_null() {
            LIVE.fetch_add(new_size as isize - layout.size() as isize, Ordering::SeqCst);
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;
fn live() -> isize {
    LIVE.load(Ordering::SeqCst)
}
#[test]
fn capacity_slack_and_recursive_prepared_storage_bound_actual_live_allocations() {
    let before = live();
    let mut schema = String::with_capacity(128);
    schema.push_str("fixture/storage-record");
    let mut name = String::with_capacity(256);
    name.push_str("value");
    let mut fields = Vec::with_capacity(32);
    fields.push(
        StructuredFieldType::new(
            name,
            StructuredInfoType::leaf(KindId::from("value/u64")).unwrap(),
        )
        .unwrap(),
    );
    let ty = StructuredInfoType::record(KindId(schema), fields).unwrap();
    assert_eq!(ty.owned_heap_bytes(), (live() - before) as usize);
    assert!(ty.owned_heap_bytes() > 128 + 256);
    drop(ty);
    assert_eq!(live(), before);

    let source = "type Cell = {\n number: U64\n}\ntype Request = {\n cells: sequence Cell <= 4\n index: U64\n}\nplot read (\n value: Request >> result: U64\n) = (sequence/at(.cells, .index).number)\nplot equal (\n value: Request >> result: Boolean\n) = (.cells == .cells)\nplot length (\n value: Request >> result: U64\n) = (sequence/length(.cells))\nplot conditional (\n value: Request >> result: U64\n) = (.index + 1)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    for entry in ["read", "equal", "length", "conditional"] {
        let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
            .unwrap()
            .expanded;
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!()
        };
        let before_program = live();
        let law = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        assert!(law.owned_heap_bytes() >= (live() - before_program) as usize);
        let before = live();
        let evaluator = PreparedPortableExpressionEvaluator::new(&law).unwrap();
        let actual = (live() - before) as usize;
        assert!(
            evaluator.owned_heap_bytes() >= actual,
            "{entry}: bound {} actual {actual}",
            evaluator.owned_heap_bytes()
        );
        drop(evaluator);
        assert_eq!(live(), before);
    }
}
