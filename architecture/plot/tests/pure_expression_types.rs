use std::collections::{BTreeMap, BTreeSet};

use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, ExternalEffectBehavior, Kind, KindId,
    KindIdentity, KindSemanticLaw, PortDescriptor, PortDirection, PortTemporal, Quantity,
    QuantityUnit, ReplayBehavior, SemanticDependence, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue, SuspensionBehavior, TemporalStateBehavior,
    VariabilityBehavior,
};
use conduit_plot::{
    check_expression, parse_syntax_document, pure_expression_definition, BackStatement,
    CheckedExpressionType, CordStage, ExpressionSyntax, ExpressionTypeContext,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator,
};

fn expression(source: &str) -> ExpressionSyntax {
    let document = parse_syntax_document(&format!(
        "plot typed (\n input: U8 >> output: U8\n) {{\n input >> ({source}) >> output\n}}\n"
    ));
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let BackStatement::Cord(cord) = &document.plots[0].back[0] else {
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
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: kind_id("value/scalar"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
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

    let checked = check_expression(&expression("(.label, { n: .samples })"), &context).unwrap();
    assert_eq!(
        checked.value_type,
        CheckedExpressionType::Tuple(vec![
            CheckedExpressionType::semantic("value/text"),
            CheckedExpressionType::Record(vec![(
                "n".into(),
                CheckedExpressionType::semantic("value/count")
            )]),
        ])
    );

    let reading_type = structured[&KindId::from("weather/reading@1")].clone();
    let reading = StructuredInfoValue::record(
        reading_type,
        vec![
            StructuredFieldValue::new(
                "label",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(KindId::from("value/text")).unwrap(),
                    b"outside".to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "samples",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(KindId::from("value/count")).unwrap(),
                    conduit_core::encode_count(4).to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let program = PortableExpressionProgram::from_checked(&checked).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = prepared.output_capacity();
    let output = prepared.evaluate(&reading).unwrap();
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(output)
            .unwrap()
            .value_type(),
        &program.output_type
    );
    assert_eq!(prepared.output_capacity(), capacity);

    let arithmetic = check_expression(&expression(".samples + 1"), &context).unwrap();
    let program = PortableExpressionProgram::from_checked(&arithmetic).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(
        conduit_core::decode_count(prepared.evaluate(&reading).unwrap()).unwrap(),
        5
    );
}

#[test]
fn scientific_literal_unit_supplies_its_exact_expression_type() {
    let temperature = CheckedExpressionType::semantic("value/temperature");
    let input = temperature.clone();
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

    let checked = check_expression(&expression(". > 30°C"), &context).unwrap();
    assert_eq!(
        checked.value_type,
        CheckedExpressionType::semantic("value/bool")
    );
    let program = PortableExpressionProgram::from_checked(&checked).unwrap();
    assert_eq!(
        program
            .evaluate(&Quantity::new(31, QuantityUnit::Celsius).encode())
            .unwrap(),
        conduit_core::InfoBool::TRUE.encode()
    );
    assert_eq!(
        program
            .evaluate(&Quantity::new(303_150, QuantityUnit::Millikelvin).encode())
            .unwrap(),
        conduit_core::InfoBool::FALSE.encode()
    );
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(
        prepared
            .evaluate(&Quantity::new(31, QuantityUnit::Celsius).encode())
            .unwrap(),
        conduit_core::InfoBool::TRUE.encode()
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

#[test]
fn checked_expression_retains_its_exact_input_and_output_contract() {
    let input = CheckedExpressionType::semantic("value/u16");
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
    let checked = check_expression(&expression(". + 1"), &context).unwrap();
    assert_eq!(checked.input_type, input);
    assert_eq!(
        checked.value_type,
        CheckedExpressionType::semantic("value/u16")
    );
    assert_eq!(checked.node_types.len(), 3);
    assert!(checked
        .node_types
        .iter()
        .all(|node| node.value_type == CheckedExpressionType::semantic("value/u16")));
    let definition = pure_expression_definition(&checked, PortTemporal::Value).unwrap();
    assert_eq!(definition.inputs[0].value_kind.as_str(), "value/u16");
    assert_eq!(definition.outputs[0].value_kind.as_str(), "value/u16");
    assert_eq!(
        definition.kind_contract_revision.as_str(),
        "conduitese/pure-expression-operation@1"
    );
    let ConfigurationValue::Text(program) = &definition.configuration[0].default_value else {
        panic!("expression program must be exact text configuration")
    };
    let portable = PortableExpressionProgram::from_checked(&checked).unwrap();
    assert_eq!(
        PortableExpressionProgram::from_canonical_hex(program),
        Ok(portable)
    );
    let mut malformed = program.clone();
    malformed.push('0');
    assert!(PortableExpressionProgram::from_canonical_hex(&malformed).is_err());

    let same = check_expression(&expression(".+1"), &context).unwrap();
    assert_eq!(
        definition.kind_id,
        pure_expression_definition(&same, PortTemporal::Value)
            .unwrap()
            .kind_id
    );
    assert_ne!(
        definition.kind_id,
        pure_expression_definition(&same, PortTemporal::Current)
            .unwrap()
            .kind_id
    );
}

#[test]
fn portable_fixed_integer_evaluation_is_checked_and_conditional_is_lazy() {
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

    let add = check_expression(&expression(". + 1"), &context).unwrap();
    assert_eq!(
        PortableExpressionProgram::from_checked(&add)
            .unwrap()
            .evaluate(&[41])
            .unwrap(),
        [42]
    );

    let lazy = check_expression(&expression(". == 0 ? . : (. / 0)"), &context).unwrap();
    assert_eq!(
        PortableExpressionProgram::from_checked(&lazy)
            .unwrap()
            .evaluate(&[0])
            .unwrap(),
        [0]
    );

    let shift = check_expression(&expression(". <<< 8"), &context).unwrap();
    assert!(PortableExpressionProgram::from_checked(&shift)
        .unwrap()
        .evaluate(&[1])
        .is_err());
}

#[test]
fn prepared_primitive_evaluation_matches_checked_meaning() {
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
    for (source, input, expected) in [
        (". + 1", 41_u8, 42_u8),
        (". == 0 ? (. + 7) : (. / 0)", 0, 7),
        ("(. <<< 1) | 1", 3, 7),
    ] {
        let checked = check_expression(&expression(source), &context).unwrap();
        let program = PortableExpressionProgram::from_checked(&checked).unwrap();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(prepared.evaluate(&[input]).unwrap(), [expected]);
    }
}

#[test]
fn portable_anonymous_structures_carry_their_exact_checked_profile() {
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
    let checked = check_expression(&expression("(., { doubled: . + . })"), &context).unwrap();
    let program = PortableExpressionProgram::from_checked(&checked).unwrap();
    let output = program.evaluate(&[3]).unwrap();
    let value = conduit_core::StructuredInfoValue::from_canonical_bytes(&output).unwrap();
    assert_eq!(value.value_type(), &program.output_type);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = prepared.output_capacity();
    assert_eq!(prepared.evaluate(&[3]).unwrap(), output);
    assert_eq!(prepared.output_capacity(), capacity);
}

#[test]
fn semantic_count_and_scalar_arithmetic_remain_exact_and_checked() {
    let empty = BTreeMap::new();
    let no_structured = BTreeMap::new();
    let no_numeric = BTreeSet::new();
    let no_kinds = BTreeMap::new();

    let count = CheckedExpressionType::semantic("value/count");
    let count_context = context(
        &count,
        &empty,
        &no_structured,
        &empty,
        &no_numeric,
        &no_kinds,
    );
    let checked = check_expression(&expression(". + 2"), &count_context).unwrap();
    assert_eq!(
        PortableExpressionProgram::from_checked(&checked)
            .unwrap()
            .evaluate(&conduit_core::encode_count(40))
            .unwrap(),
        conduit_core::encode_count(42)
    );

    let scalar = CheckedExpressionType::semantic("value/scalar");
    let scalar_context = context(
        &scalar,
        &empty,
        &no_structured,
        &empty,
        &no_numeric,
        &no_kinds,
    );
    let checked = check_expression(&expression(". / 2"), &scalar_context).unwrap();
    let three = conduit_core::Scalar::from_raw_microunits(3_000_000).encode();
    assert_eq!(
        PortableExpressionProgram::from_checked(&checked)
            .unwrap()
            .evaluate(&three)
            .unwrap(),
        conduit_core::Scalar::from_raw_microunits(1_500_000).encode()
    );
}
