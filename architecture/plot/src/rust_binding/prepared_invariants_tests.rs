use super::{
    validate_native_invariants, NativeBindingRefusal, PreparedNativeInvariantAdmission,
    PreparedNativeInvariantRefusal,
};
use crate::{check_syntax_document, parse_syntax_document, StartupCatalog};
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};

#[test]
fn prepared_native_bank_preserves_order_and_law_refusals() {
    let checked = check_syntax_document(&parse_syntax_document("type Interval = {\n start: U32\n end: U32\n where .start <= .end\n where .end < 10\n}\n"), &StartupCatalog::new()).unwrap();
    let ty = &checked.native_types[0];
    let mut prepared =
        PreparedNativeInvariantAdmission::new(&ty.value_type, &ty.invariants, 2, 65536).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.value_type.shape() else {
        panic!()
    };
    for (start, end, expected) in [
        (4u32, 4u32, Ok(())),
        (
            5,
            4,
            Err(NativeBindingRefusal::ViolatedInvariant { index: 0 }),
        ),
        (
            4,
            10,
            Err(NativeBindingRefusal::ViolatedInvariant { index: 1 }),
        ),
        (
            11,
            10,
            Err(NativeBindingRefusal::ViolatedInvariant { index: 0 }),
        ),
    ] {
        let values = fields
            .iter()
            .map(|field| {
                let value = if field.name() == "start" { start } else { end };
                StructuredFieldValue::new(
                    field.name(),
                    super::primitive_into_structured(field.value_type().clone(), &value).unwrap(),
                )
                .unwrap()
            })
            .collect();
        let value = StructuredInfoValue::record(ty.value_type.clone(), values).unwrap();
        assert_eq!(validate_native_invariants(&value, &ty.invariants), expected);
        assert_eq!(
            prepared.validate(&value.canonical_bytes().unwrap()),
            expected
        );
    }
    assert!(matches!(
        PreparedNativeInvariantAdmission::new(&ty.value_type, &ty.invariants, 1, 65536),
        Err(PreparedNativeInvariantRefusal::Capacity)
    ));
    assert!(matches!(
        PreparedNativeInvariantAdmission::new(&ty.value_type, &ty.invariants, 2, 1),
        Err(PreparedNativeInvariantRefusal::Capacity)
    ));
    assert!(matches!(
        prepared.validate(&[0]),
        Err(NativeBindingRefusal::InvalidInvariant(
            crate::PortableExpressionEvaluationRefusal::InvalidInput
        ))
    ));
}

#[test]
fn prepared_native_bank_checks_nested_members_and_foreign_exact_types() {
    let source = "type Inner = {\n value: U32\n}\ntype Outer = {\n child: Inner\n where .child.value < 10\n}\ntype Foreign = {\n different: U32\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let outer = checked
        .native_types
        .iter()
        .find(|ty| ty.invariants.len() == 1)
        .unwrap();
    let mut prepared =
        PreparedNativeInvariantAdmission::new(&outer.value_type, &outer.invariants, 1, 65536)
            .unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = outer.value_type.shape() else {
        panic!()
    };
    let inner_type = fields[0].value_type();
    let StructuredInfoTypeShape::Record {
        fields: inner_fields,
        ..
    } = inner_type.shape()
    else {
        panic!()
    };
    for number in [3u32, 10] {
        let leaf = super::primitive_into_structured(inner_fields[0].value_type().clone(), &number)
            .unwrap();
        let inner = StructuredInfoValue::record(
            inner_type.clone(),
            vec![StructuredFieldValue::new(inner_fields[0].name(), leaf).unwrap()],
        )
        .unwrap();
        let value = StructuredInfoValue::record(
            outer.value_type.clone(),
            vec![StructuredFieldValue::new(fields[0].name(), inner).unwrap()],
        )
        .unwrap();
        assert_eq!(
            prepared.validate(&value.canonical_bytes().unwrap()),
            validate_native_invariants(&value, &outer.invariants)
        );
    }
    let foreign = checked.native_types.iter().find(|ty| matches!(ty.value_type.shape(), StructuredInfoTypeShape::Record { fields, .. } if fields[0].name() == "different")).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = foreign.value_type.shape() else {
        panic!()
    };
    let value = StructuredInfoValue::record(
        foreign.value_type.clone(),
        vec![StructuredFieldValue::new(
            "different",
            super::primitive_into_structured(fields[0].value_type().clone(), &3u32).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    assert_eq!(
        prepared.validate(&value.canonical_bytes().unwrap()),
        validate_native_invariants(&value, &outer.invariants)
    );
    assert!(matches!(
        prepared.validate(&value.canonical_bytes().unwrap()),
        Err(NativeBindingRefusal::InvalidInvariant(
            crate::PortableExpressionEvaluationRefusal::InvalidInput
        ))
    ));
}
