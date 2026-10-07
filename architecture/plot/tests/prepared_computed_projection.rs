//! A checked projection may select a member of a computed structured value.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

fn program() -> PortableExpressionProgram {
    let source = "type Cell = {\n number: U64\n}\ntype Request = {\n cells: sequence Cell <= 4\n index: U64\n}\nplot read (\n value: Request >> result: U64\n) = (sequence/at(.cells, .index).number)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "read", &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn request(p: &PortableExpressionProgram, index: u64) -> Vec<u8> {
    let cells_type = field(&p.input_type, "cells");
    let StructuredInfoTypeShape::Sequence { element, .. } = cells_type.shape() else {
        panic!()
    };
    let cell = StructuredInfoValue::record(
        element.clone(),
        vec![StructuredFieldValue::new(
            "number",
            StructuredInfoValue::leaf(field(element, "number"), 77u64.to_le_bytes().to_vec())
                .unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    let cells = StructuredInfoValue::sequence(cells_type, vec![cell]).unwrap();
    StructuredInfoValue::record(
        p.input_type.clone(),
        vec![
            StructuredFieldValue::new("cells", cells).unwrap(),
            StructuredFieldValue::new(
                "index",
                StructuredInfoValue::leaf(
                    field(&p.input_type, "index"),
                    index.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn computed_element_projection_preserves_refusals_without_runtime_allocation() {
    let p = program();
    let inputs = [
        request(&p, 0),
        request(&p, 1),
        request(&p, u64::MAX),
        vec![0],
    ];
    let expected = inputs
        .iter()
        .map(|input| p.evaluate(input))
        .collect::<Vec<_>>();
    let foreign = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap(),
        77u64.to_le_bytes().to_vec(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let foreign_expected = p.evaluate(&foreign);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = prepared.output_capacity();
    assert_eq!(
        allocation::allocations(|| {
            for _ in 0..1000 {
                for (input, expected) in inputs.iter().zip(&expected) {
                    assert_eq!(
                        prepared.evaluate(input),
                        expected.as_ref().map(Vec::as_slice).map_err(Clone::clone)
                    );
                }
                assert_eq!(
                    prepared.evaluate(&foreign),
                    foreign_expected
                        .as_ref()
                        .map(Vec::as_slice)
                        .map_err(Clone::clone)
                );
            }
        }),
        0
    );
    assert_eq!(prepared.output_capacity(), capacity);
}
