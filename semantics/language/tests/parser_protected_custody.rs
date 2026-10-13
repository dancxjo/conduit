//! Typed custody regression draft; generated Source prerequisites are required.
//! The empty predecessor is a newly admitted reference fixture. The original
//! actual acquisition receipt remains unchanged and already has active VOC2.
extern crate alloc;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "../src/parser_custody_budget.rs"]
mod parser_custody_budget;
#[path = "../src/parser_protected_custody.rs"]
mod parser_protected_custody;
#[path = "../src/parser_protected_origin.rs"]
mod parser_protected_origin;
use parser_custody_budget::{Limits, Usage};
use parser_protected_custody::ProtectedCustody;
use parser_protected_origin::PreparedProtectedOrigin;

fn bytes(row: &serde_json::Value, field: &str) -> Vec<u8> {
    serde_json::from_value(row[field].clone()).expect("whole canonical native byte array")
}
fn limits(rebases: u32) -> Limits {
    Limits {
        retained: Usage {
            origins: 4,
            rebases,
            bytes: 2_000_000,
            ..Usage::default()
        },
        peak_bytes: 4_000_000,
    }
}
fn reference_first(
    original: &LanguageParserIndependentProtectedAdmission,
) -> LanguageParserIndependentProtectedAdmission {
    let old = original.insert().previous();
    let empty = LanguageParserProtectedSetProposal::new(
        [false; 4],
        old.basis().clone(),
        old.edge0().clone(),
        old.edge1().clone(),
        old.edge2().clone(),
        old.edge3().clone(),
    )
    .unwrap();
    let insertion =
        LanguageParserProtectedInsertContext::new(original.insert().context().clone(), empty)
            .unwrap();
    LanguageParserIndependentProtectedAdmission::new(
        original.admission().clone(),
        insertion,
        original.output().clone(),
    )
    .unwrap()
}
#[test]
#[ignore = "requires original retained receipts via CONDUIT_PROTECTED_CUSTODY_RECEIPT_PATH"]
fn exact_initial_reference_rebase_pressure_and_cancel_preserve_full_current_set() {
    let path = std::env::var("CONDUIT_PROTECTED_CUSTODY_RECEIPT_PATH").unwrap();
    let row: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let actual_bytes = bytes(&row, "independent_admission_bytes");
    let actual = LanguageParserIndependentProtectedAdmission::decode(&actual_bytes).unwrap();
    assert!(actual.insert().previous().active()[2]);
    let actual_origin = PreparedProtectedOrigin::prepare(actual.clone()).unwrap();
    assert!(actual_origin.initial().is_err());
    assert_eq!(actual.clone().encode().unwrap(), actual_bytes);
    let first = reference_first(&actual);
    assert_eq!(*first.insert().previous().active(), [false; 4]);
    let context =
        LanguageParserProtectedSetRebaseContext::decode(&bytes(&row, "rebase_context_bytes"))
            .unwrap();
    let next =
        LanguageParserProtectedSetProposal::decode(&bytes(&row, "rebased_set_bytes")).unwrap();
    let receipt = LanguageParserProtectedRebaseReceipt::new(context, next.clone()).unwrap();

    let mut accepted = ProtectedCustody::prepare(
        limits(1),
        PreparedProtectedOrigin::prepare(first.clone()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        *accepted.initial().insertion().previous().active(),
        [false; 4]
    );
    let stage = accepted.stage_rebase(200_000).unwrap();
    stage.check_before_consumption().unwrap();
    stage.finish(receipt.clone()).unwrap();
    assert_eq!(accepted.current(), &next);
    assert!(accepted.acquire(actual_origin).is_err()); // stale whole prior after rebase
    assert_eq!(accepted.current(), &next);
    assert!(accepted.rebase(receipt.clone()).is_err()); // stale whole predecessor
    assert_eq!(accepted.current(), &next);

    let mut pressure = ProtectedCustody::prepare(
        limits(0),
        PreparedProtectedOrigin::prepare(first.clone()).unwrap(),
    )
    .unwrap();
    let previous = pressure.current().clone();
    assert!(pressure.stage_rebase(200_000).is_err());
    assert!(pressure.rebase(receipt.clone()).is_err());
    assert_eq!(pressure.current(), &previous);

    let mut cancelled =
        ProtectedCustody::prepare(limits(1), PreparedProtectedOrigin::prepare(first).unwrap())
            .unwrap();
    let previous = cancelled.current().clone();
    let cancellation = cancelled.cancellation();
    let stage = cancelled.stage_rebase(200_000).unwrap();
    cancellation.cancel();
    assert!(stage.check_before_consumption().is_err());
    assert!(stage.finish(receipt).is_err());
    assert_eq!(cancelled.current(), &previous);
    // Neither this new reference fixture nor this custody test invokes a model,
    // advances a played frontier, or changes the original actual receipt.
    assert_eq!(actual.encode().unwrap(), actual_bytes);
}
