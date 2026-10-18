//! Executable portable-WASM proof. This is not an installed browser Back.
use conduit_core::ConfigurationValue;
use conduit_plot::*;
use std::sync::atomic::{AtomicUsize, Ordering};
#[path = "../tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
static STARTED: AtomicUsize = AtomicUsize::new(0);
static RECEIPT_POINTER: AtomicUsize = AtomicUsize::new(0);
static RECEIPT_LENGTH: AtomicUsize = AtomicUsize::new(0);

fn program(source: &str, entry: &str) -> (PortableExpressionProgram, serde_json::Value) {
    let document = parse_syntax_document(source);
    let checked = check_syntax_document(&document, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("exact program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let identity = serde_json::json!({
        "source_document_id": checked.source_document_id,
        "checked_plot_id": expanded.expanded.checked_plot_id,
        "expanded_plot_id": expanded.expanded.expanded_plot_id,
        "program": encoded,
    });
    (program, identity)
}

fn equivalent(
    actual: Result<&[u8], PortableExpressionEvaluationRefusal>,
    expected: &Result<Vec<u8>, PortableExpressionEvaluationRefusal>,
) -> bool {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => actual == expected,
        (Err(actual), Err(expected)) => &actual == expected,
        _ => false,
    }
}

fn matrix(boolean: bool, operations: u64) -> serde_json::Value {
    let (source, entry) = if boolean {
        (
            include_str!("../tests/fixtures/expression_boolean_baseline.conduit"),
            format!("boolean-{operations}"),
        )
    } else {
        (
            include_str!("../tests/fixtures/expression_baseline.conduit"),
            format!("baseline-{operations}"),
        )
    };
    let (program, identity) = program(source, &entry);
    let mut baseline = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let mut tape = PreparedPortableExpressionEvaluator::new_scalar_tape(&program).unwrap();
    let inputs: Vec<Vec<u8>> = if boolean {
        vec![vec![0], vec![1], vec![2], vec![], vec![1, 0]]
    } else {
        [0, 1, 7, u64::MAX / (operations + 1), u64::MAX]
            .map(|n| n.to_le_bytes().to_vec())
            .into_iter()
            .chain([vec![], vec![1]])
            .collect()
    };
    let mut results = Vec::new();
    for input in &inputs {
        let expected = program.evaluate(input);
        if boolean && matches!(input.as_slice(), [0] | [1]) {
            assert_eq!(expected.as_ref().unwrap(), input);
        } else if !boolean && input.len() == 8 {
            let n = u64::from_le_bytes(input.as_slice().try_into().unwrap());
            match n.checked_mul(operations + 1) {
                Some(value) => assert_eq!(expected.as_ref().unwrap(), &value.to_le_bytes()),
                None => assert!(expected.is_err()),
            }
        } else {
            assert!(expected.is_err());
        }
        let (equal, allocations) = allocation_probe::observe(|| {
            (0..16).all(|_| {
                equivalent(baseline.evaluate(input), &expected)
                    && equivalent(tape.evaluate(input), &expected)
            })
        });
        assert!(equal, "reference/tape mismatch");
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
        results.push(serde_json::json!({
            "input": input,
            "outcome": match expected { Ok(bytes) => serde_json::json!({"bytes": bytes}),
                Err(reason) => serde_json::json!({"refusal": format!("{reason:?}")}) },
            "warm_iterations": 16,
            "allocations": allocations.allocations,
            "reallocations": allocations.reallocations,
        }));
    }
    let (instructions, scratch) = tape.scalar_tape_limits().unwrap();
    serde_json::json!({"domain": if boolean {"boolean"} else {"integer"},
        "operations": operations, "identity": identity, "cases": results,
        "maximum_instructions": instructions, "scratch_payload_bytes": scratch,
        "output_capacity_bytes": tape.output_capacity()})
}

/// Run once in an isolated proof instance. A second call refuses without work.
#[no_mangle]
pub extern "C" fn scalar_tape_proof_run() -> u32 {
    if STARTED.swap(1, Ordering::AcqRel) != 0 {
        return 2;
    }
    let profiles: Vec<_> = [false, true]
        .into_iter()
        .flat_map(|boolean| [1, 3, 8, 32].map(|n| matrix(boolean, n)))
        .collect();
    let receipt = serde_json::to_vec(&serde_json::json!({
        "schema": "conduit.scalar-tape/wasm-parity@1",
        "proof_class": "portable-wasm-evaluator",
        "profiles": profiles,
    }))
    .unwrap()
    .into_boxed_slice();
    RECEIPT_LENGTH.store(receipt.len(), Ordering::Release);
    // One finite proof receipt lives until this isolated WASM instance is dropped.
    RECEIPT_POINTER.store(
        Box::into_raw(receipt) as *mut u8 as usize,
        Ordering::Release,
    );
    0
}
#[no_mangle]
pub extern "C" fn scalar_tape_proof_pointer() -> usize {
    RECEIPT_POINTER.load(Ordering::Acquire)
}
#[no_mangle]
pub extern "C" fn scalar_tape_proof_length() -> usize {
    RECEIPT_LENGTH.load(Ordering::Acquire)
}
