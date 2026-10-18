//! A pinned independent oracle supplies expected bits and exception flags.
use conduit_core::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static ALLOCATIONS: Cell<Option<usize>> = const {Cell::new(None)}; }
struct Allocator;
fn count() {
    let _ = ALLOCATIONS.try_with(|counter| {
        if let Some(n) = counter.get() {
            counter.set(Some(n + 1));
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
const MODES: [Binary64Rounding; 4] = [
    Binary64Rounding::NearestTiesEven,
    Binary64Rounding::TowardZero,
    Binary64Rounding::TowardPositive,
    Binary64Rounding::TowardNegative,
];
const OPS: [IeeeBinary64Operation; 4] = [
    IeeeBinary64Operation::Add,
    IeeeBinary64Operation::Subtract,
    IeeeBinary64Operation::Multiply,
    IeeeBinary64Operation::Divide,
];
const PREDICATES: [IeeeBinary64QuietPredicate; 4] = [
    IeeeBinary64QuietPredicate::Equal,
    IeeeBinary64QuietPredicate::Less,
    IeeeBinary64QuietPredicate::LessEqual,
    IeeeBinary64QuietPredicate::Unordered,
];

#[test]
fn binary64_bits_and_all_flags_match_the_independent_oracle_without_allocating() {
    struct Case {
        arithmetic: bool,
        operation: usize,
        mode: usize,
        left: IeeeF64,
        right: IeeeF64,
        bits: u64,
        flags: u8,
    }
    let cases: Vec<_> = include_str!("fixtures/ieee_binary64_softfloat.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let columns: Vec<_> = line.split_ascii_whitespace().collect();
            assert_eq!(columns.len(), 7);
            Case {
                arithmetic: columns[0] == "a",
                operation: columns[1].parse().unwrap(),
                mode: columns[2].parse().unwrap(),
                left: IeeeF64::from_bits(u64::from_str_radix(columns[3], 16).unwrap()),
                right: IeeeF64::from_bits(u64::from_str_radix(columns[4], 16).unwrap()),
                bits: u64::from_str_radix(columns[5], 16).unwrap(),
                flags: columns[6].parse().unwrap(),
            }
        })
        .collect();
    assert_eq!(cases.len(), 8000);
    ALLOCATIONS.with(|counter| assert_eq!(counter.replace(Some(0)), None));
    for (index, case) in cases.iter().enumerate() {
        let (bits, flags) = if case.arithmetic {
            let result = evaluate_ieee_binary64(
                OPS[case.operation],
                case.left,
                case.right,
                MODES[case.mode],
                &mut work(),
            )
            .unwrap();
            (result.value.bits(), result.flags.bits())
        } else {
            let result = compare_ieee_binary64_quiet(
                PREDICATES[case.operation],
                case.left,
                case.right,
                &mut work(),
            )
            .unwrap();
            (result.value as u64, result.flags.bits())
        };
        assert_eq!(
            (bits, flags),
            (case.bits, case.flags),
            "oracle case={index}"
        );
    }
    assert_eq!(ALLOCATIONS.with(|counter| counter.replace(None)), Some(0));
}

#[test]
fn operation_order_and_explicit_sticky_flags_do_not_become_exact_final_rounding() {
    let large = IeeeF64::from_bits(0x4340_0000_0000_0000);
    let one = IeeeF64::from_bits(0x3ff0_0000_0000_0000);
    let add = evaluate_ieee_binary64(OPS[0], large, one, MODES[0], &mut work()).unwrap();
    let subtract = evaluate_ieee_binary64(OPS[1], add.value, large, MODES[0], &mut work()).unwrap();
    assert_eq!(subtract.value.bits(), 0);
    assert_eq!(subtract.flags.bits(), 0);
    assert_eq!(add.flags.union(subtract.flags).bits(), 1);
    let cancelled = evaluate_ieee_binary64(OPS[1], large, large, MODES[0], &mut work()).unwrap();
    let reordered =
        evaluate_ieee_binary64(OPS[0], cancelled.value, one, MODES[0], &mut work()).unwrap();
    assert_eq!(reordered.value, one);
    assert_eq!(reordered.flags.bits(), 0);
}

#[test]
fn exhausted_work_refuses_even_for_exceptional_operands_instead_of_returning_false_flags() {
    for bits in [
        0,
        1,
        0x3ff0_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0x7ff0_0000_0000_0001,
    ] {
        let value = IeeeF64::from_bits(bits);
        for op in OPS {
            assert_eq!(
                evaluate_ieee_binary64(op, value, value, MODES[0], &mut ExactNumericWork::new(0)),
                Err(ExactNumericRefusal::WorkLimit)
            );
        }
        for predicate in PREDICATES {
            assert_eq!(
                compare_ieee_binary64_quiet(predicate, value, value, &mut ExactNumericWork::new(0)),
                Err(ExactNumericRefusal::WorkLimit)
            );
        }
    }
}
