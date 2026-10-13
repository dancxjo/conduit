#![cfg(feature = "kernel-step")]
use conduit_core::numeric_realization::*;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};
use conduit_semantic_catalog::NumericEvaluationBack;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountedAllocator;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

unsafe impl GlobalAlloc for CountedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountedAllocator = CountedAllocator;

fn measured<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|active| active.set(true));
    let result = f();
    COUNTING.with(|active| active.set(false));
    (result, ALLOCATIONS.with(Cell::get))
}

#[test]
fn prepared_numeric_output_waits_retries_and_emits_without_play_allocation() {
    let graph = NumericComputation::new(
        vec![NumericComputationNode::literal(
            MATH_VALUE_INFO_ID.to_string(),
            None,
            &MathValue::integer(42),
        )
        .unwrap()],
        0,
    )
    .unwrap();
    let requirement = NumericRequirement {
        semantics: NumericSemantics::Mathematical,
        representation: NumericRepresentation::CheckedI64,
        rounding: Binary64Rounding::NearestEven,
        accuracy: NumericAccuracy::Exact,
        parameters: Vec::new(),
        budget: NumericExecutionBudget {
            storage_bytes: 1_048_576,
            work_steps: 100_000,
            limbs: 128,
            symbolic_nodes: 128,
        },
        reproducible: true,
        exceptions: NumericExceptionPolicy::default(),
    };
    let witness = certify_computation(&graph, &requirement).unwrap();
    let output = PreparedNumericOutput::new(&graph, witness).unwrap();
    let expected = output.canonical().to_vec();
    let mut back = NumericEvaluationBack::new(output);
    let ((), allocations) = measured(|| {
        for _ in 0..100 {
            let mut io = StepIo::test_frame([None], [false], [None], None, 4);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None], None)),
                StepOutcome::Await
            );
            assert_eq!(
                <NumericEvaluationBack as StepBack<1>>::prepared_output(&back, PortId(0)),
                Some(expected.as_slice())
            );
        }
        let mut io = StepIo::test_frame([None], [true], [Some(4096)], None, 4);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
    });
    assert_eq!(allocations, 0);
}
