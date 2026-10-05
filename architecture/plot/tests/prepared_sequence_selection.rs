//! Finite runtime selection preserves actual count and exact element identity.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionEvaluationRefusal, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = "
type Request = {
 bytes: sequence U8 <= 4
 index: U64
}
type Result =
 octet U8
 | short
plot read (
 value: Request >> result: U8
) = (sequence/at(.bytes, .index))
plot guarded (
 value: Request >> result: Result
) = (.index < sequence/length(.bytes) ? octet(sequence/at(.bytes, .index)) : short(unit))
plot computed (
 value: Request >> result: U8
) = (sequence/at(.index == 0 ? .bytes : .bytes, .index))
type Cell = {
 number: U64
}
type Cells = {
 cells: sequence Cell <= 4
 index: U64
}
plot cell (
 value: Cells >> result: Cell
) = (sequence/at(.cells, .index))
";
fn program(source: &str, entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("pure program");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record");
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn request(
    program: &PortableExpressionProgram,
    name: &str,
    values: Vec<StructuredInfoValue>,
    index: u64,
) -> Vec<u8> {
    let ty = field_type(&program.input_type, name);
    let collection = match ty.shape() {
        StructuredInfoTypeShape::Collection { .. } => StructuredInfoValue::collection(ty, values),
        _ => StructuredInfoValue::sequence(ty, values),
    }
    .unwrap();
    let index = StructuredInfoValue::leaf(
        field_type(&program.input_type, "index"),
        index.to_le_bytes().to_vec(),
    )
    .unwrap();
    StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(name, collection).unwrap(),
            StructuredFieldValue::new("index", index).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn bytes(program: &PortableExpressionProgram, wire: &[u8], index: u64) -> Vec<u8> {
    let ty = field_type(&program.input_type, "bytes");
    let (StructuredInfoTypeShape::Sequence { element, .. }
    | StructuredInfoTypeShape::Collection { element, .. }) = ty.shape()
    else {
        panic!("collection or sequence");
    };
    request(
        program,
        "bytes",
        wire.iter()
            .map(|b| StructuredInfoValue::leaf(element.clone(), vec![*b]).unwrap())
            .collect(),
        index,
    )
}

#[test]
fn runtime_index_obeys_actual_count_and_never_truncates_large_indices() {
    for entry in ["read", "computed"] {
        let p = program(SOURCE, entry);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        for length in 0..=4 {
            for index in [0, 1, 2, 3, 4, 255, 65536, u64::MAX] {
                let wire = &[9, 8, 7, 6][..length];
                let input = bytes(&p, wire, index);
                if index < length as u64 {
                    assert_eq!(p.evaluate(&input).unwrap(), [wire[index as usize]]);
                    assert_eq!(prepared.evaluate(&input).unwrap(), [wire[index as usize]]);
                } else {
                    assert_eq!(
                        p.evaluate(&input),
                        Err(PortableExpressionEvaluationRefusal::InvalidInput)
                    );
                    assert_eq!(
                        prepared.evaluate(&input),
                        Err(PortableExpressionEvaluationRefusal::InvalidInput)
                    );
                }
            }
        }
    }
}
#[test]
fn guarded_selection_and_record_elements_are_allocation_free_after_preparation() {
    let p = program(SOURCE, "guarded");
    let inputs = [bytes(&p, &[], u64::MAX), bytes(&p, &[4, 3, 2, 1], 3)];
    let expected = inputs
        .iter()
        .map(|input| p.evaluate(input).unwrap())
        .collect::<Vec<_>>();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(
        allocation::allocations(|| {
            for _ in 0..10_000 {
                for (input, expected) in inputs.iter().zip(&expected) {
                    assert_eq!(prepared.evaluate(input).unwrap(), expected);
                }
            }
        }),
        0
    );
    let p = program(SOURCE, "cell");
    let cell = StructuredInfoValue::record(
        p.output_type.clone(),
        vec![StructuredFieldValue::new(
            "number",
            StructuredInfoValue::leaf(
                field_type(&p.output_type, "number"),
                77_u64.to_le_bytes().to_vec(),
            )
            .unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    let input = request(&p, "cells", vec![cell], 0);
    let expected = p.evaluate(&input).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(
        allocation::allocations(|| {
            for _ in 0..10_000 {
                assert_eq!(prepared.evaluate(&input).unwrap(), expected);
            }
        }),
        0
    );
}
#[test]
fn checking_and_preparation_reject_substituted_index_and_element_types() {
    let wrong = SOURCE.replace("index: U64", "index: U8");
    let syntax = parse_syntax_document(&wrong);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    assert!(expand_canonical_plot_for_authoring(&checked, "read", &ProfileCatalog::new()).is_err());
    let mut p = program(SOURCE, "read");
    p.root.value_type = StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    p.output_type = p.root.value_type.clone();
    assert!(PreparedPortableExpressionEvaluator::new(&p).is_err());
}

#[test]
fn fixed_collections_obey_the_same_exact_index_contract() {
    let source = SOURCE.replace("bytes: sequence U8 <= 4", "bytes: collection U8 = 4");
    let guarded = program(&source, "guarded");
    let mut guard = PreparedPortableExpressionEvaluator::new(&guarded).unwrap();
    let outside = bytes(&guarded, &[1, 2, 3, 4], 4);
    assert_eq!(
        guard.evaluate(&outside).unwrap(),
        guarded.evaluate(&outside).unwrap()
    );
    let p = program(&source, "read");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for index in [0, 3, 4, u64::MAX] {
        let input = bytes(&p, &[1, 2, 3, 4], index);
        let ordinary = p.evaluate(&input);
        let actual = prepared.evaluate(&input).map(|bytes| bytes.to_vec());
        assert_eq!(actual, ordinary);
        if index < 4 {
            assert_eq!(actual.unwrap(), [(index + 1) as u8]);
        } else {
            assert_eq!(
                actual,
                Err(PortableExpressionEvaluationRefusal::InvalidInput)
            );
        }
    }
}
