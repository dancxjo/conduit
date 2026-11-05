//! Checked inline owners survive ordinary Native admission and closed imports.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn document() -> CheckedSyntaxDocument {
    check_syntax_document(
        &parse_syntax_document("type Reply =\n ready {\n value: U8 in 1..=7\n }\n | absent\n"),
        &StartupCatalog::new(),
    )
    .unwrap()
}
fn native<'a>(d: &'a CheckedSyntaxDocument, name: &str) -> &'a CheckedNativeType {
    d.native_types.iter().find(|n| n.name == name).unwrap()
}
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 4096,
        scopes: 64,
        invariants: 64,
        contracts: 64,
    }
}
fn value(ty: &StructuredInfoType, tag: &str, n: u8) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let payload = cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type();
    let payload_value = match payload.shape() {
        StructuredInfoTypeShape::Leaf(_) => {
            StructuredInfoValue::leaf(payload.clone(), vec![]).unwrap()
        }
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            payload.clone(),
            vec![StructuredFieldValue::new(
                "value",
                StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![n]).unwrap(),
            )
            .unwrap()],
        )
        .unwrap(),
        _ => panic!("payload"),
    };
    StructuredInfoValue::variant(ty.clone(), tag, payload_value).unwrap()
}
fn complete(admission: &mut PreparedNativeAdmission<'_>) -> Result<(), PreparedNativeRefusal> {
    loop {
        if admission.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
#[test]
fn locally_checked_inline_payload_keeps_its_owner_and_predicates_without_runtime_allocation() {
    let d = document();
    let root = native(&d, "Reply");
    assert_eq!(
        d.native_types.len(),
        1,
        "inline owners must not invent public declarations"
    );
    let good = value(&root.value_type, "ready", 1)
        .canonical_bytes()
        .unwrap();
    let bad = value(&root.value_type, "ready", 0)
        .canonical_bytes()
        .unwrap();
    let absent = value(&root.value_type, "absent", 0)
        .canonical_bytes()
        .unwrap();
    let mut admitted = PreparedNativeAdmission::new(&d, &root.identity, bounds()).unwrap();
    let (_, cost) = allocation_probe::observe(|| {
        admitted.begin(&good).unwrap();
        complete(&mut admitted).unwrap();
        assert_eq!(admitted.admitted().unwrap().canonical_bytes(), good);
        admitted.retire_admitted().unwrap();
        admitted.begin(&absent).unwrap();
        complete(&mut admitted).unwrap();
        admitted.retire_admitted().unwrap();
        admitted.begin(&bad).unwrap();
        assert!(matches!(
            complete(&mut admitted),
            Err(PreparedNativeRefusal::Contract { .. })
        ));
        admitted.cancel();
        assert_eq!(admitted.begin(&absent), Err(PreparedNativeRefusal::State));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
#[test]
fn checked_document_import_retains_private_inline_closure_and_preserves_scope_limits() {
    let source = document();
    let root = native(&source, "Reply");
    let mut catalogue = StartupCatalog::new();
    catalogue
        .insert_checked_native_type_from_document("library/Reply", &source, &root.identity)
        .unwrap();
    let imported = check_syntax_document(
        &parse_syntax_document("with library/Reply as Reply\ntype Holder = {\n reply: Reply\n}\n"),
        &catalogue,
    )
    .unwrap();
    let holder = native(&imported, "Holder");
    let holder_bytes = |n| {
        StructuredInfoValue::record(
            holder.value_type.clone(),
            vec![StructuredFieldValue::new("reply", value(&root.value_type, "ready", n)).unwrap()],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    };
    let good = holder_bytes(7);
    for invalid in [0, 8] {
        let mut admitted =
            PreparedNativeAdmission::new(&imported, &holder.identity, bounds()).unwrap();
        assert_eq!(admitted.scope_count(), 3);
        admitted.begin(&good).unwrap();
        complete(&mut admitted).unwrap();
        admitted.retire_admitted().unwrap();
        // Reusing the imported owner must preserve the inline 1..=7 predicate.
        admitted.begin(&holder_bytes(invalid)).unwrap();
        assert!(matches!(
            complete(&mut admitted),
            Err(PreparedNativeRefusal::Contract { .. })
        ));
    }
    let mut small = PreparedNativeAdmission::new_with_occurrence_limit(
        &imported,
        &holder.identity,
        bounds(),
        1,
    )
    .unwrap();
    small.begin(&good).unwrap();
    assert_eq!(complete(&mut small), Err(PreparedNativeRefusal::Bounds));
}
#[test]
fn closed_import_is_transactional_and_inspection_cannot_grant_or_replace_owners() {
    let source = document();
    let identity = native(&source, "Reply").identity.clone();
    let mut catalogue = StartupCatalog::new();
    let before = catalogue.clone();
    let mut conflict = source.clone();
    conflict.native_types[0].value_contracts.clear();
    assert!(catalogue
        .insert_checked_native_type_from_document("library/Reply", &conflict, &identity)
        .is_err());
    assert_eq!(catalogue, before);
    let mut hidden = source.clone();
    hidden.native_types.clear();
    catalogue
        .insert_checked_native_type_from_document("library/Reply", &hidden, &identity)
        .unwrap();
    let before = catalogue.clone();
    let fake = check_syntax_document(
        &parse_syntax_document("type Forged = U8 in 1..=2\n"),
        &StartupCatalog::new(),
    )
    .unwrap()
    .native_types
    .remove(0);
    let fake_id = fake.identity.clone();
    hidden.native_types.push(fake);
    assert!(catalogue
        .insert_checked_native_type_from_document("library/Forged", &hidden, &fake_id)
        .is_err());
    assert_eq!(catalogue, before);
    assert!(catalogue
        .insert_checked_native_type_from_document("library/Reply", &source, &identity)
        .is_err());
    assert_eq!(catalogue, before);
}
