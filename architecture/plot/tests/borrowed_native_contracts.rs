//! Owned and borrowed Native checks share Source laws and refusal identities.
use conduit_core::{
    validate_canonical_structured_value, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_plot::rust_binding::{
    validate_native_contracts, validate_native_contracts_borrowed, NativeBindingRefusal,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static ENABLED: Cell<bool> = const { Cell::new(false) }; static CALLS: Cell<usize> = const { Cell::new(0) }; }
struct Counted;
fn record() {
    let _ = ENABLED.try_with(|v| {
        if v.get() {
            let _ = CALLS.try_with(|v| v.set(v.get() + 1));
        }
    });
}
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
fn representation(ty: &StructuredInfoType) -> &StructuredInfoType {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal {
            representation: v, ..
        } => representation(v),
        _ => ty,
    }
}
fn wrap(ty: &StructuredInfoType, value: StructuredInfoValue) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), wrap(representation, value)).unwrap()
        }
        _ => {
            assert_eq!(value.value_type(), ty);
            value
        }
    }
}
fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> StructuredInfoValue {
    wrap(
        ty,
        StructuredInfoValue::leaf(representation(ty).clone(), bytes.to_vec()).unwrap(),
    )
}
fn item(
    ty: &StructuredInfoType,
    label: &str,
    count: u16,
    optional: Option<&str>,
) -> StructuredInfoValue {
    let record_type = representation(ty);
    let StructuredInfoTypeShape::Record { fields, .. } = record_type.shape() else {
        panic!("record");
    };
    let values = fields
        .iter()
        .map(|field| {
            let value = match field.name() {
                "label" => leaf(field.value_type(), label.as_bytes()),
                "count" => leaf(field.value_type(), &count.to_le_bytes()),
                "optional" => {
                    let variant = representation(field.value_type());
                    let StructuredInfoTypeShape::Variant { cases, .. } = variant.shape() else {
                        panic!("option");
                    };
                    let tag = if optional.is_some() { "some" } else { "none" };
                    let payload = cases
                        .iter()
                        .find(|v| v.tag() == tag)
                        .unwrap()
                        .payload_type();
                    wrap(
                        field.value_type(),
                        StructuredInfoValue::variant(
                            variant.clone(),
                            tag,
                            leaf(payload, optional.unwrap_or("").as_bytes()),
                        )
                        .unwrap(),
                    )
                }
                _ => panic!("unexpected Source field"),
            };
            StructuredFieldValue::new(field.name(), value).unwrap()
        })
        .collect();
    wrap(
        ty,
        StructuredInfoValue::record(record_type.clone(), values).unwrap(),
    )
}
#[test]
fn borrowed_native_checks_preserve_source_laws_without_allocating() {
    let checked = check_syntax_document(&parse_syntax_document(
        "type Item = {\n label: Text <= 8B not in [\"\"]\n count: U16 in 1..=7\n optional: Text <= 8B not in [\"\"]?\n}\ntype Items = sequence Item in 0..=4\n"
    ), &StartupCatalog::new()).unwrap();
    let native = checked
        .native_types
        .iter()
        .find(|n| n.name == "Items")
        .unwrap();
    assert!(
        !native.value_contracts.is_empty(),
        "the test must carry Source refinements"
    );
    let sequence = representation(&native.value_type);
    let StructuredInfoTypeShape::Sequence { element, .. } = sequence.shape() else {
        panic!("sequence");
    };
    let header = native.value_type.canonical_bytes().unwrap();
    for (label, count, optional, valid) in [
        ("ok", 1, None, true),
        ("", 1, None, false),
        ("123456789", 1, None, false),
        ("ok", 0, None, false),
        ("ok", 8, None, false),
        ("éééé", 7, Some("yes"), true),
        ("ééééé", 7, None, false),
        ("ok", 1, Some(""), false),
        ("ok", 1, Some("123456789"), false),
    ] {
        let owned = wrap(
            &native.value_type,
            StructuredInfoValue::sequence(
                sequence.clone(),
                vec![
                    item(element, "first", 1, None),
                    item(element, label, count, optional),
                ],
            )
            .unwrap(),
        );
        let expected = validate_native_contracts(&owned, &native.value_contracts);
        assert_eq!(expected.is_ok(), valid);
        let bytes = owned.canonical_bytes().unwrap();
        CALLS.with(|v| v.set(0));
        ENABLED.with(|v| v.set(true));
        let borrowed = validate_canonical_structured_value(&bytes).unwrap();
        assert_eq!(borrowed.type_bytes(), header);
        let actual = validate_native_contracts_borrowed(borrowed, &native.value_contracts);
        ENABLED.with(|v| v.set(false));
        assert_eq!(CALLS.with(Cell::get), 0);
        match (expected, actual) {
            (Ok(()), Ok(())) => {}
            (
                Err(NativeBindingRefusal::ViolatedConstraint {
                    representation_path,
                    refusal,
                }),
                Err(actual),
            ) => {
                assert_eq!(actual.representation_path, representation_path);
                assert_eq!(actual.refusal, refusal);
            }
            _ => panic!("owned and borrowed Source checks disagree"),
        }
    }
}
