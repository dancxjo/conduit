use std::collections::{BTreeMap, BTreeSet};

use conduit_core::{KindId, StructuredFieldType, StructuredInfoType};
use conduit_form::{
    check_expression, parse_syntax_document, BackStatement, CheckedExpressionType, CordStage,
    ExpressionSyntax, ExpressionTypeContext,
};

fn expression(source: &str) -> ExpressionSyntax {
    let document = parse_syntax_document(&format!(
        "form typed (\n input: U8 >> output: U8\n) {{\n input >> ({source}) >> output\n}}\n"
    ));
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let BackStatement::Cord(cord) = &document.forms[0].back[0] else {
        panic!("fixture contains one cord")
    };
    let CordStage::PureExpression(expression) = &cord.stages[1] else {
        panic!("middle cord stage is a pure expression")
    };
    expression.syntax.clone()
}

fn context<'a>(
    input: &'a CheckedExpressionType,
    immutable_values: &'a BTreeMap<String, CheckedExpressionType>,
    structured_types: &'a BTreeMap<KindId, StructuredInfoType>,
    literal_types: &'a BTreeMap<String, CheckedExpressionType>,
    numeric_types: &'a BTreeSet<KindId>,
) -> ExpressionTypeContext<'a> {
    ExpressionTypeContext {
        input,
        immutable_values,
        structured_types,
        literal_types,
        numeric_types,
    }
}

#[test]
fn fixed_integer_operators_retain_width_and_comparisons_return_boolean() {
    let input = CheckedExpressionType::semantic("value/u8");
    let empty = BTreeMap::new();
    let no_structured = BTreeMap::new();
    let no_numeric = BTreeSet::new();
    let context = context(&input, &empty, &no_structured, &empty, &no_numeric);

    assert_eq!(
        check_expression(&expression("(. <<< 1) | 0x03"), &context)
            .unwrap()
            .value_type,
        input
    );
    assert_eq!(
        check_expression(&expression(". > 0xff"), &context)
            .unwrap()
            .value_type,
        CheckedExpressionType::semantic("value/bool")
    );
}

#[test]
fn ambiguous_and_out_of_range_integer_literals_refuse_during_checking() {
    let input = CheckedExpressionType::semantic("value/u8");
    let empty = BTreeMap::new();
    let no_structured = BTreeMap::new();
    let no_numeric = BTreeSet::new();
    let context = context(&input, &empty, &no_structured, &empty, &no_numeric);

    assert!(check_expression(&expression("255"), &context).is_err());
    let error = check_expression(&expression(". + 256"), &context).unwrap_err();
    assert!(error.message.contains("outside the exact U8 range"));
}

#[test]
fn conditional_is_boolean_and_unifies_one_exact_branch_type() {
    let input = CheckedExpressionType::semantic("value/bool");
    let empty = BTreeMap::new();
    let no_structured = BTreeMap::new();
    let no_numeric = BTreeSet::new();
    let context = context(&input, &empty, &no_structured, &empty, &no_numeric);

    assert_eq!(
        check_expression(&expression(". ? \"yes\" : \"no\""), &context)
            .unwrap()
            .value_type,
        CheckedExpressionType::semantic("value/text")
    );
    assert!(check_expression(&expression(". ? \"yes\" : false"), &context).is_err());
}

#[test]
fn nominal_structured_input_projects_into_anonymous_structures() {
    let text = StructuredInfoType::leaf(KindId::from("value/text")).unwrap();
    let count = StructuredInfoType::leaf(KindId::from("value/count")).unwrap();
    let reading = StructuredInfoType::record(
        KindId::from("weather/reading@1"),
        vec![
            StructuredFieldType::new("label", text).unwrap(),
            StructuredFieldType::new("samples", count).unwrap(),
        ],
    )
    .unwrap();
    let input = CheckedExpressionType::semantic("weather/reading@1");
    let empty = BTreeMap::new();
    let structured = BTreeMap::from([(KindId::from("weather/reading@1"), reading)]);
    let no_numeric = BTreeSet::new();
    let context = context(&input, &empty, &structured, &empty, &no_numeric);

    assert_eq!(
        check_expression(&expression("(.label, { n: .samples })"), &context)
            .unwrap()
            .value_type,
        CheckedExpressionType::Tuple(vec![
            CheckedExpressionType::semantic("value/text"),
            CheckedExpressionType::Record(vec![(
                "n".into(),
                CheckedExpressionType::semantic("value/count")
            )]),
        ])
    );
}

#[test]
fn scientific_literal_type_comes_from_the_semantic_literal_catalog() {
    let temperature = CheckedExpressionType::semantic("value/temperature");
    let input = temperature.clone();
    let empty = BTreeMap::new();
    let no_structured = BTreeMap::new();
    let literals = BTreeMap::from([("30°C".into(), temperature)]);
    let numeric = BTreeSet::from([KindId::from("value/temperature")]);
    let context = context(&input, &empty, &no_structured, &literals, &numeric);

    assert_eq!(
        check_expression(&expression(". > 30°C"), &context)
            .unwrap()
            .value_type,
        CheckedExpressionType::semantic("value/bool")
    );
}
