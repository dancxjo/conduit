#![cfg(feature = "authenticated-admission")]
//! Canonical six-Plot CPU reference shared with the actual host Start proof.
#[path = "continuation_scope/reference.rs"]
mod reference;

#[test]
fn full_six_reference_authorizes_only_fresh_local_three_and_refuses_scope_tampering() {
    reference::with_full_reference(|scope, _, _, _, _, _| {
        for i in 0..scope.fragment_count() {
            let document = scope.document(i).unwrap();
            let (metadata, projection) =
                conduit_body::decode_body_fragment_document(document).unwrap();
            let mut legacy = postcard::to_allocvec(&metadata).unwrap();
            legacy.extend_from_slice(projection);
            assert_eq!(legacy, document, "same original Postcard model/bytes");
        }
    });
}

#[test]
fn full_six_typed_content_rebinds_owner_boot_and_preserves_static_contract() {
    reference::with_typed_reference(|scope, _, _| assert_eq!(scope.fragment_count(), 3));
}

#[test]
fn canonical_two_operation_continuation_matches_pinned_authority_not_row_or_grant_sort() {
    reference::with_native_reference(2);
}

#[test]
fn canonical_single_operation_continuation_remains_supported() {
    reference::with_native_reference(1);
}
