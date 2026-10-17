//! Compare the migration against exact #5208 source, not reconstructed examples.
use conduit_ai::fixed_numeric_catalog::{fixed_numeric_types, FIXED_NUMERIC_SOURCE};
use conduit_core::{kind_id, StructuredInfoType, StructuredInfoTypeShape as Shape};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

const BEFORE: &str = include_str!("fixtures/fixed_numeric_before_value_parameters.conduit");

#[test]
fn all_pinned_numeric_shapes_and_scalar_contracts_survive_parameterization() {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
        )
        .unwrap();
    let before = check_syntax_document(&parse_syntax_document(BEFORE), &catalog).unwrap();
    let after = fixed_numeric_types().unwrap();
    assert_eq!(before.native_types.len(), 23);
    for old in &before.native_types {
        let new = after
            .iter()
            .find(|candidate| candidate.name == old.name)
            .expect("every established named consumer remains available");
        assert_eq!(
            structure(&old.value_type),
            structure(&new.value_type),
            "{}",
            old.name
        );
        assert_eq!(old.value_contracts, new.value_contracts, "{}", old.name);
        if old.name.contains("MatrixRef")
            || old.name == "NumericEmbedding224x12"
            || old.name.contains("BiasRef")
        {
            assert_eq!(
                new.invariants.len(),
                1,
                "dimension law is an ordinary portable Type law"
            );
        }
    }
    let parsed = parse_syntax_document(FIXED_NUMERIC_SOURCE);
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.round_trip(), FIXED_NUMERIC_SOURCE);
    assert_eq!(
        parsed
            .types
            .iter()
            .filter(|item| !item.parameters.is_empty())
            .count(),
        5
    );
    assert_eq!(FIXED_NUMERIC_SOURCE.matches("columns: U16").count(), 1);
    assert_eq!(FIXED_NUMERIC_SOURCE.matches("length: U16").count(), 1);
}

#[test]
fn tensor_element_case_resource_reference_and_digest_are_preserved() {
    let types = fixed_numeric_types().unwrap();
    let element = &types
        .iter()
        .find(|item| item.name == "NumericTensorElement")
        .unwrap()
        .value_type;
    let Shape::Variant { cases, .. } = element.shape() else {
        panic!("closed semantic element case expected")
    };
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].tag(), "f32");
    let matrix = &types
        .iter()
        .find(|item| item.name == "NumericF32MatrixRef192x128")
        .unwrap()
        .value_type;
    let Shape::Record { fields, .. } = matrix.shape() else {
        panic!("record expected")
    };
    let resource = fields
        .iter()
        .find(|field| field.name() == "resource")
        .unwrap();
    assert!(
        matches!(resource.value_type().shape(), Shape::Leaf(kind) if kind.as_str() == conduit_core::RESOURCE_REFERENCE_INFO_ID)
    );
    let digest = fields
        .iter()
        .find(|field| field.name() == "content_digest")
        .unwrap();
    assert!(
        matches!(representation(digest.value_type()), Shape::Collection { length: 32, element } if matches!(element.shape(), Shape::Leaf(kind) if kind.as_str() == "value/u8"))
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field.name() == "element")
            .unwrap()
            .value_type(),
        element
    );
}

fn representation(value: &StructuredInfoType) -> Shape<'_> {
    match value.shape() {
        Shape::Nominal {
            representation: inner,
            ..
        } => representation(inner),
        shape => shape,
    }
}

// Compare admitted structure while keeping new family/nominal identities distinct.
fn structure(value: &StructuredInfoType) -> String {
    match value.shape() {
        Shape::Leaf(kind) => format!("leaf:{}", kind.as_str()),
        Shape::Nominal { representation, .. } => structure(representation),
        Shape::Collection { element, length } => {
            format!("collection:{length}:{}", structure(element))
        }
        Shape::Sequence {
            element,
            minimum_items,
            maximum_items,
        } => format!(
            "sequence:{minimum_items}:{maximum_items}:{}",
            structure(element)
        ),
        Shape::Record { fields, .. } => format!(
            "record:{:?}",
            fields
                .iter()
                .map(|field| (field.name(), structure(field.value_type())))
                .collect::<Vec<_>>()
        ),
        Shape::Variant { cases, .. } => format!(
            "variant:{:?}",
            cases
                .iter()
                .map(|case| (case.tag(), structure(case.payload_type())))
                .collect::<Vec<_>>()
        ),
    }
}
