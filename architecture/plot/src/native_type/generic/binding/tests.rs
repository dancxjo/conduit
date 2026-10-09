use crate::prelude::*;
use crate::{check_syntax_document, parse_syntax_document, StartupCatalog};

fn checked(source: &str) -> crate::CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap()
}

#[test]
fn typed_values_specialize_all_numeric_vector_extents() {
    let mut source = "type Vector<N: U16> = collection F32 finite = N\n".to_string();
    for size in [2, 3, 12, 20, 32, 64, 128, 192, 320] {
        source.push_str(&format!("type Vector{size} = Vector<{size}>\n"));
    }
    let parsed = parse_syntax_document(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.round_trip(), source);
    assert_eq!(parsed.types[0].parameters[0].name.text, "N");
    assert!(parsed.types[0].parameters[0].value_type.is_some());
    let output = checked(&source);
    assert_eq!(output.native_types.len(), 9);
    for value in output.native_types {
        assert!(
            !value.value_contracts.is_empty(),
            "element finiteness survives"
        );
        assert!(value.value_type.canonical_bytes().is_ok());
    }
}

#[test]
fn nested_mixed_families_derive_window_extent_without_runtime_parameters() {
    let source = "type Vector<T, n: U16> = collection T = n\ntype History<T, h: U16, d: U16> = collection Vector<T, d> = h\ntype Window<T, h: U16, d: U16> = {\n window: Vector<T, (h + 1) * d>\n next_history: History<T, h, d>\n}\ntype Value = Window<F32 finite, 2, 64>\n";
    let value = checked(source);
    assert!(value
        .native_types
        .iter()
        .any(|native| native.name == "Value"));
    assert!(value
        .native_types
        .iter()
        .all(|native| native.value_type.canonical_bytes().is_ok()));
    use conduit_core::StructuredInfoTypeShape as Shape;
    let native = value
        .native_types
        .iter()
        .find(|native| native.name == "Value")
        .unwrap();
    let Shape::Record { fields, .. } = representation(&native.value_type) else {
        panic!("record expected")
    };
    let window = fields
        .iter()
        .find(|field| field.name() == "window")
        .unwrap();
    assert!(matches!(
        representation(window.value_type()),
        Shape::Collection { length: 192, .. }
    ));
    let history = fields
        .iter()
        .find(|field| field.name() == "next_history")
        .unwrap();
    let Shape::Collection { length: 2, element } = representation(history.value_type()) else {
        panic!("two history entries expected")
    };
    assert!(matches!(
        representation(element),
        Shape::Collection { length: 64, .. }
    ));
}

#[test]
fn equivalent_value_arguments_share_checked_meaning_for_the_same_declaration() {
    let prefix = "type Vector<N: U16> = collection U8 = N\n";
    let first = checked(&(prefix.to_string() + "type Value = Vector<32 + 32>\n"));
    let second = checked(&(prefix.to_string() + "type Value = Vector<64>\n"));
    assert_eq!(first.native_types, second.native_types);
    let nested = checked(
        &(prefix.to_string() + "type Value = {\n left: Vector<32 + 32>\n right: Vector<64>\n}\n"),
    );
    assert_eq!(
        nested.native_types.len(),
        2,
        "one shared private specialization"
    );
    let different = checked(&(prefix.to_string() + "type Other = Vector<64>\n"));
    assert_ne!(
        first.native_types[0].identity,
        different.native_types[0].identity
    );
}

#[test]
fn parameter_refinement_uses_existing_exact_scalar_contracts() {
    let prefix = "type Vector<N: U16 in 1..=64> = collection U8 = N\n";
    checked(&(prefix.to_string() + "type Value = Vector<32 + 32>\n"));
    for argument in ["0", "65", "65535"] {
        let source = prefix.to_string() + &format!("type Value = Vector<{argument}>\n");
        let error = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
            .unwrap_err();
        assert!(
            error.message.contains("declared scalar contract"),
            "{}",
            error.message
        );
        assert_eq!(&source[error.span.start..error.span.end], argument);
    }
}

#[test]
fn variable_sequence_capacity_and_fixed_shape_remain_distinct() {
    let source = "type Tier<Capacity: U16> = sequence U8 <= Capacity\ntype Fixed<Length: U16> = collection U8 = Length\ntype Value = {\n lip: Tier<3>\n tongue: Tier<7>\n exact: Fixed<3>\n}\n";
    let output = checked(source);
    assert_eq!(output.native_types.len(), 4);
    let sequences = output
        .native_types
        .iter()
        .filter(|value| value.name.starts_with("TierInstantiation"))
        .count();
    assert_eq!(sequences, 2);
}

#[test]
fn wrong_kinds_arity_unknown_values_zero_and_overflow_refuse() {
    for source in [
        "type Vector<N: U16> = collection U8 = N\ntype Value = Vector<Text>\n",
        "type Vector<N: U16> = collection U8 = N\ntype Value = Vector<.runtime>\n",
        "type Vector<N: U16> = collection U8 = N\ntype Value = Vector<0>\n",
        "type Vector<N: U16> = collection U8 = N\ntype Value = Vector<1025>\n",
        "type Vector<N: U16> = collection U8 = N\ntype Value = Vector<(65535 + 1) * 0>\n",
        "type Vector<N: U16> = collection U8 = N\ntype Value = Vector<2, 3>\n",
        "type Pair<T> = collection T = 2\ntype Value = Pair<64>\n",
        "type Pair<T> = collection T = 2\ntype Wrapper<N: U16> = Pair<N>\ntype Value = Wrapper<64>\n",
        "type Vector<N: U16> = collection U8 = Unknown\ntype Value = Vector<2>\n",
        "type Loop<N: U16> = Loop<N + 1>\ntype Value = Loop<1>\n",
    ] {
        assert!(check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).is_err(), "{source}");
    }
}

fn representation(
    value: &conduit_core::StructuredInfoType,
) -> conduit_core::StructuredInfoTypeShape<'_> {
    match value.shape() {
        conduit_core::StructuredInfoTypeShape::Nominal {
            representation: inner,
            ..
        } => representation(inner),
        shape => shape,
    }
}

#[test]
fn value_family_contract_version_and_specialization_budgets_are_retained() {
    let narrow =
        checked("type Vector<N: U16 in 1..=64> = collection U8 = N\ntype Value = Vector<32>\n");
    let wide =
        checked("type Vector<N: U16 in 1..=128> = collection U8 = N\ntype Value = Vector<32>\n");
    assert_ne!(
        narrow.native_types[0].identity,
        wide.native_types[0].identity
    );
    let mut source = "type Vector<N: U16> = collection U8 = N\n".to_string();
    for index in 1..=129 {
        source.push_str(&format!(
            "type Record{index} = {{\n value: Vector<{index}>\n}}\n"
        ));
    }
    let error =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap_err();
    assert!(
        error.message.contains("generated-instance budget"),
        "{}",
        error.message
    );
}

#[test]
fn instantiated_record_laws_refuse_wrong_dimensions_before_and_after_transport() {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoTypeShape as Shape, StructuredInfoValue,
    };
    let source = "type Matrix<C: U16, R: U16> = {\n columns: U16\n rows: U16\n where .columns == C && .rows == R\n}\ntype Value = Matrix<192, 128>\n";
    let output = checked(source);
    let matrix = &output.native_types[0];
    assert_eq!(matrix.invariants.len(), 1);
    let Shape::Record { fields, .. } = matrix.value_type.shape() else {
        panic!("record expected")
    };
    for (columns, rows, accepted) in [
        (192_u16, 128_u16, true),
        (193, 128, false),
        (192, 127, false),
    ] {
        let values = fields
            .iter()
            .map(|field| {
                let scalar = if field.name() == "columns" {
                    columns
                } else {
                    rows
                };
                StructuredFieldValue::new(
                    field.name(),
                    crate::rust_binding::primitive_into_structured(
                        field.value_type().clone(),
                        &scalar,
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect();
        let value = StructuredInfoValue::record(matrix.value_type.clone(), values).unwrap();
        assert_eq!(
            crate::rust_binding::validate_native_invariants(&value, &matrix.invariants).is_ok(),
            accepted
        );
        let bytes = value.canonical_bytes().unwrap();
        let decoded = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(
            crate::rust_binding::validate_native_invariants(&decoded, &matrix.invariants).is_ok(),
            accepted
        );
    }
    let nested = checked(
        &(source.replace(
            "type Value = Matrix<192, 128>",
            "type Value = {\n matrix: Matrix<192, 128>\n}",
        )),
    );
    assert!(nested
        .native_types
        .iter()
        .any(
            |native| native.name.starts_with("MatrixInstantiation") && native.invariants.len() == 1
        ));
}

#[test]
fn laws_are_instantiated_canonically_while_source_spelling_is_retained() {
    let prefix = "type Length<N: U16> = {\n value: U16\n where .value == N\n}\n";
    let literal = prefix.to_string() + "type Value = Length<64>\n";
    let expression = prefix.to_string() + "type Value = Length<32 + 32>\n";
    assert_eq!(
        checked(&literal).native_types,
        checked(&expression).native_types
    );
    let parsed = parse_syntax_document(&expression);
    assert_eq!(parsed.types[0].invariants[0].text, ".value == N");
    assert_eq!(parsed.round_trip(), expression);
}
