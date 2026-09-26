use std::collections::{BTreeMap, BTreeSet};

use conduit_core::{
    kind_id, port_id, CapabilityLimits, ExternalEffectBehavior, Kind, KindId, KindIdentity,
    KindSemanticLaw, PortDescriptor, PortDirection, PortTemporal, ReplayBehavior,
    SemanticDependence, StructuredFieldType, StructuredInfoType, SuspensionBehavior,
    TemporalStateBehavior, VariabilityBehavior,
};
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
    semantic_kinds: &'a BTreeMap<String, Kind>,
) -> ExpressionTypeContext<'a> {
    ExpressionTypeContext {
        input,
        immutable_values,
        structured_types,
        literal_types,
        numeric_types,
        semantic_kinds,
    }
}

fn pure_kind(name: &str) -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: Some((port_id("value"), port_id("result"))),
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(format!("{name}@1")),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id("value/scalar"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: kind_id("value/scalar"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Suspension(SuspensionBehavior::Never),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(ReplayBehavior::Exact),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16,
        },
    }
}

#[test]
fn fixed_integer_operators_retain_width_and_comparisons_return_boolean() {
    let input = CheckedExpressionType::semantic("value/u8");
    let empty = BTreeMap::new();
    let no_structured = BTreeMap::new();
    let no_numeric = BTreeSet::new();
    let no_kinds = BTreeMap::new();
    let context = context(
        &input,
        &empty,
        &no_structured,
        &empty,
        &no_numeric,
        &no_kinds,
    );

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
    let no_kinds = BTreeMap::new();
    let context = context(
        &input,
        &empty,
        &no_structured,
        &empty,
        &no_numeric,
        &no_kinds,
    );

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
    let no_kinds = BTreeMap::new();
    let context = context(
        &input,
        &empty,
        &no_structured,
        &empty,
        &no_numeric,
        &no_kinds,
    );

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
    let no_kinds = BTreeMap::new();
    let context = context(&input, &empty, &structured, &empty, &no_numeric, &no_kinds);

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
    let no_kinds = BTreeMap::new();
    let context = context(
        &input,
        &empty,
        &no_structured,
        &literals,
        &numeric,
        &no_kinds,
    );

    assert_eq!(
        check_expression(&expression(". > 30°C"), &context)
            .unwrap()
            .value_type,
        CheckedExpressionType::semantic("value/bool")
    );
}

#[test]
fn reviewed_pure_semantic_kind_call_uses_its_exact_front() {
    let input = CheckedExpressionType::semantic("value/scalar");
    let empty = BTreeMap::new();
    let no_structured = BTreeMap::new();
    let no_numeric = BTreeSet::new();
    let kinds = BTreeMap::from([("math/sin".into(), pure_kind("math/sin"))]);
    let pure_context = context(&input, &empty, &no_structured, &empty, &no_numeric, &kinds);
    assert_eq!(
        check_expression(&expression("math/sin(.)"), &pure_context)
            .unwrap()
            .value_type,
        input
    );

    let mut effectful = pure_kind("math/sin");
    effectful.semantic_laws[0] =
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    let effectful = BTreeMap::from([("math/sin".into(), effectful)]);
    let context = context(
        &input,
        &empty,
        &no_structured,
        &empty,
        &no_numeric,
        &effectful,
    );
    assert!(check_expression(&expression("math/sin(.)"), &context).is_err());
}

#[test]
fn anonymous_expression_types_have_stable_exact_port_identities() {
    let first = CheckedExpressionType::Record(vec![
        (
            "label".into(),
            CheckedExpressionType::semantic("value/text"),
        ),
        (
            "point".into(),
            CheckedExpressionType::Tuple(vec![
                CheckedExpressionType::semantic("value/i16"),
                CheckedExpressionType::semantic("value/i16"),
            ]),
        ),
    ]);
    let reordered = CheckedExpressionType::Record(vec![
        (
            "point".into(),
            CheckedExpressionType::Tuple(vec![
                CheckedExpressionType::semantic("value/i16"),
                CheckedExpressionType::semantic("value/i16"),
            ]),
        ),
        (
            "label".into(),
            CheckedExpressionType::semantic("value/text"),
        ),
    ]);
    assert_eq!(first.exact_value_kind(), reordered.exact_value_kind());
    assert_eq!(
        first.structured_info_type().unwrap(),
        reordered.structured_info_type().unwrap()
    );

    let changed = CheckedExpressionType::Record(vec![
        (
            "label".into(),
            CheckedExpressionType::semantic("value/text"),
        ),
        (
            "point".into(),
            CheckedExpressionType::Tuple(vec![
                CheckedExpressionType::semantic("value/i16"),
                CheckedExpressionType::semantic("value/i32"),
            ]),
        ),
    ]);
    assert_ne!(first.exact_value_kind(), changed.exact_value_kind());
}
