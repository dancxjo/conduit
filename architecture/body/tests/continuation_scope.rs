#![cfg(feature = "authenticated-admission")]
//! Canonical six-Plot CPU reference shared with the actual host Start proof.
#[path = "continuation_scope/reference.rs"]
mod reference;

#[test]
fn full_six_reference_authorizes_only_fresh_local_three_and_refuses_scope_tampering() {
    reference::with_reference(|_, _, _| {});
}
