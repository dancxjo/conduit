//! Private structural admission is not a publicly forged Native root.
use super::*;
use crate::rust_binding::primitive_into_structured;
use conduit_core::*;
fn owner() -> NativeLawWitness {
    crate::check_syntax_document_with_native_law_roots(
        &crate::parse_syntax_document("type Interval = {\n start: U64 in 0..=9\n end: U64 in 0..=9\n where .start <= .end\n}\n"),
        &crate::StartupCatalog::new(), &["Interval"],
    ).unwrap().native_law_witness("Interval").unwrap().clone()
}
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 16384,
        scopes: 32,
        invariants: 32,
        contracts: 32,
    }
}
fn interval(witness: &NativeLawWitness, start: u64, end: u64) -> StructuredInfoValue {
    let scalar = |n| {
        primitive_into_structured(StructuredInfoType::leaf(kind_id("value/u64")).unwrap(), &n)
            .unwrap()
    };
    StructuredInfoValue::record(
        witness.checked_type().value_type.clone(),
        alloc::vec![
            StructuredFieldValue::new("start", scalar(start)).unwrap(),
            StructuredFieldValue::new("end", scalar(end)).unwrap(),
        ],
    )
    .unwrap()
}
fn run(guard: &mut PreparedRegisteredValue, bytes: &[u8]) -> Result<(), PreparedNativeRefusal> {
    guard.engine.begin(bytes)?;
    loop {
        if guard.engine.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
#[test]
fn prepared_native_guards_share_the_exact_immutable_table() {
    let witness = owner();
    let registry = NativeLawRegistry::new(core::slice::from_ref(&witness)).unwrap();
    let first = registry
        .prepare_native(&witness.checked_type().identity, bounds())
        .unwrap();
    let second = registry
        .prepare_native(&witness.checked_type().identity, bounds())
        .unwrap();
    let metadata::Metadata::Shared(left) = &first.guard.engine.metadata else {
        panic!()
    };
    let metadata::Metadata::Shared(right) = &second.guard.engine.metadata else {
        panic!()
    };
    assert!(Shared::ptr_eq(left, right));
    assert!(Shared::ptr_eq(left, &registry.data));
}
#[test]
fn exact_anonymous_record_root_checks_real_nested_native_laws() {
    let witness = owner();
    let registry = NativeLawRegistry::new(core::slice::from_ref(&witness)).unwrap();
    let root = crate::expression_check::anonymous_record(
        "record",
        alloc::vec![
            StructuredFieldType::new("value", witness.checked_type().value_type.clone()).unwrap(),
        ],
    )
    .unwrap();
    for (start, end, valid) in [(2, 7, true), (7, 2, false)] {
        let bytes = StructuredInfoValue::record(
            root.clone(),
            alloc::vec![
                StructuredFieldValue::new("value", interval(&witness, start, end)).unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let mut guard = registry.prepare_type(&root, bounds(), 4096).unwrap();
        let result = run(&mut guard, &bytes);
        if valid {
            result.unwrap();
            let admitted = guard.admitted().unwrap();
            assert_eq!(admitted.value_type, &root);
            assert_eq!(admitted.bytes, bytes);
        } else {
            assert!(matches!(
                result,
                Err(PreparedNativeRefusal::Invariant { .. })
            ));
            assert!(guard.admitted().is_err());
        }
    }
    let Shape::Record { fields, .. } = root.shape() else {
        panic!()
    };
    let forged = StructuredInfoType::record(
        kind_id("conduitese/anonymous-record-forged@1"),
        fields.to_vec(),
    )
    .unwrap();
    assert!(registry.prepare_type(&forged, bounds(), 4096).is_err());
}
#[test]
fn exact_optional_tuple_and_intrinsic_roots_refuse_foreign_or_missing_owners() {
    let witness = owner();
    let registry = NativeLawRegistry::new(core::slice::from_ref(&witness)).unwrap();
    let root = optional_info_type(witness.checked_type().value_type.clone()).unwrap();
    let none = StructuredInfoValue::variant(
        root.clone(),
        "none",
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
            alloc::vec![],
        )
        .unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut guard = registry.prepare_type(&root, bounds(), 4096).unwrap();
    run(&mut guard, &none).unwrap();
    assert_eq!(guard.engine.actual_occurrences(), 0);
    assert_eq!(guard.admitted().unwrap().value_type, &root);
    let empty = NativeLawRegistry::new(&[]).unwrap();
    assert!(empty.prepare_type(&root, bounds(), 4096).is_err());
    let tuple = tuple_info_type(alloc::vec![witness.checked_type().value_type.clone()]).unwrap();
    assert!(registry.prepare_type(&tuple, bounds(), 4096).is_ok());
    let Shape::Variant { cases, .. } = root.shape() else {
        panic!()
    };
    let forged =
        StructuredInfoType::variant(kind_id("core/optional-forged"), cases.to_vec()).unwrap();
    assert!(registry.prepare_type(&forged, bounds(), 4096).is_err());
    assert!(empty
        .prepare_type(
            &StructuredInfoType::leaf(kind_id("value/u64-foreign")).unwrap(),
            bounds(),
            4096
        )
        .is_err());
    assert!(empty
        .prepare_type(
            &StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
            bounds(),
            4096
        )
        .is_ok());
}

#[test]
fn registered_predicates_select_checked_arithmetic_and_retain_original_bytes() {
    let document = crate::check_syntax_document_with_native_law_roots(
        &crate::parse_syntax_document("type Counter = {\n value: U64\n where .value + 1 > 0\n}\n"),
        &crate::StartupCatalog::new(),
        &["Counter"],
    )
    .unwrap();
    let witness = document.native_law_witness("Counter").unwrap();
    let original = witness.checked_type().invariants[0]
        .canonical_bytes()
        .unwrap();
    let registry = NativeLawRegistry::new(core::slice::from_ref(witness)).unwrap();
    let shared = metadata::Metadata::Shared(registry.data.clone());
    assert!(shared.checked_arithmetic());
    assert!(!metadata::Metadata::Borrowed(&document).checked_arithmetic());
    assert!(!metadata::Metadata::Owned(alloc::vec![]).checked_arithmetic());
    let scalar = primitive_into_structured(
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
        &u64::MAX,
    )
    .unwrap();
    let bytes = StructuredInfoValue::record(
        witness.checked_type().value_type.clone(),
        alloc::vec![StructuredFieldValue::new("value", scalar).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut guard = registry
        .prepare_native(&witness.checked_type().identity, bounds())
        .unwrap();
    guard.begin(&bytes).unwrap();
    assert!(matches!(
        guard.step(),
        Err(PreparedNativeRefusal::Evaluation { .. })
    ));
    assert!(guard.admitted().is_err());
    assert_eq!(
        registry.data.types[0].invariants[0]
            .canonical_bytes()
            .unwrap(),
        original
    );
}
