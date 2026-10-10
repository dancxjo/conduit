use super::{host, installed_std, RecordingTimer};
use conduit_core::{BaseImplementationId, KindIdentity, Quantity, QuantityUnit};
use conduit_plot::{
    check_expression, check_syntax_document, expand_canonical_plot, parse_syntax_document,
    BackStatement, CheckedExpressionType, CordStage, ExpressionTypeContext, KindConfigurationField,
    KindConfigurationRule, KindProjection, KindSignature, PortableExpressionProgram,
    ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn fixed_integer_expression_plans_and_plays_through_the_std_host() {
    assert_expression_plans_and_plays(&[41], &[42], "value/u8", "value/u8", ". + 1");
}

#[test]
fn scientific_quantity_comparison_plans_and_plays_through_the_std_host() {
    assert_expression_plans_and_plays(
        &Quantity::new(31, QuantityUnit::Celsius).encode(),
        &conduit_core::InfoBool::TRUE.encode(),
        conduit_core::TEMPERATURE_INFO_ID,
        conduit_core::BOOL_INFO_ID,
        ". > 30°C",
    );
}

#[test]
fn ternary_selects_one_exact_branch_through_the_std_host() {
    assert_expression_plans_and_plays(
        &conduit_core::Scalar::from_raw_microunits(2_000_000).encode(),
        &conduit_core::Scalar::from_raw_microunits(1_000_000).encode(),
        conduit_core::SCALAR_INFO_ID,
        conduit_core::SCALAR_INFO_ID,
        ". > 0 ? 1 : (1 / 0)",
    );
}

#[test]
fn anonymous_record_and_tuple_plan_and_play_through_the_std_host() {
    let expression_source = "(., { doubled: . + . })";
    let syntax = parse_syntax_document(&format!(
        "plot typed (\n input: U8 >> output: U8\n) {{\n input >> ({expression_source}) >> output\n}}\n"
    ));
    let BackStatement::Cord(cord) = &syntax.plots[0].back[0] else {
        panic!("fixture contains one Cord")
    };
    let CordStage::PureExpression(expression) = &cord.stages[1] else {
        panic!("fixture contains one expression")
    };
    let input_type = CheckedExpressionType::semantic("value/u8");
    let empty_values = BTreeMap::new();
    let empty_types = BTreeMap::new();
    let empty_numeric = BTreeSet::new();
    let empty_kinds = BTreeMap::new();
    let checked = check_expression(
        &expression.syntax,
        &ExpressionTypeContext {
            glyph_values: None,
            input: &input_type,
            immutable_values: &empty_values,
            structured_types: &empty_types,
            literal_types: &empty_values,
            numeric_types: &empty_numeric,
            semantic_kinds: &empty_kinds,
        },
    )
    .unwrap();
    let program = PortableExpressionProgram::from_checked(&checked).unwrap();
    let expected = program.evaluate(&[3]).unwrap();
    let output_kind = program.output_type.profile().unwrap().value_kind().clone();
    assert_pipeline_plans_and_plays(
        &[3],
        &expected,
        "value/u8",
        output_kind.as_str(),
        &format!("({expression_source})"),
        false,
        false,
    );
}

#[test]
fn semantic_call_with_literal_argument_plans_and_plays_as_real_gears() {
    assert_pipeline_plans_and_plays(
        &conduit_core::Scalar::from_raw_microunits(2_000_000).encode(),
        &conduit_core::Scalar::from_raw_microunits(1_000_000).encode(),
        conduit_core::SCALAR_INFO_ID,
        conduit_core::SCALAR_INFO_ID,
        "(math/clamp(1))",
        false,
        true,
    );
}

#[test]
fn one_value_when_plans_and_plays_as_exact_some_or_none_through_the_std_host() {
    let scalar_type =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::SCALAR_INFO_ID))
            .unwrap();
    let optional_kind = conduit_core::optional_info_type(scalar_type.clone())
        .unwrap()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    for (input, selected) in [
        (conduit_core::Scalar::from_raw_microunits(2_000_000), true),
        (conduit_core::Scalar::from_raw_microunits(500_000), false),
    ] {
        let input = input.encode();
        let mut encoder =
            conduit_core::PreparedOptionalInfoEncoder::new(scalar_type.clone()).unwrap();
        let expected = encoder
            .encode(selected.then_some(input.as_slice()))
            .unwrap()
            .to_vec();
        assert_pipeline_plans_and_plays(
            &input,
            &expected,
            conduit_core::SCALAR_INFO_ID,
            optional_kind.as_str(),
            "when(. > 1)",
            true,
            false,
        );
    }
}

fn assert_expression_plans_and_plays(
    input: &[u8],
    expected: &[u8],
    input_kind: &str,
    output_kind: &str,
    expression_source: &str,
) {
    assert_pipeline_plans_and_plays(
        input,
        expected,
        input_kind,
        output_kind,
        &format!("({expression_source})"),
        false,
        false,
    );
}

fn assert_pipeline_plans_and_plays(
    input: &[u8],
    expected: &[u8],
    input_kind: &str,
    output_kind: &str,
    stage_source: &str,
    filter: bool,
    math_call: bool,
) {
    let mut startup = StartupCatalog::new();
    for (kind, value) in [
        (installed_std::test_structured_selector::SOURCE_KIND, input),
        (installed_std::test_structured_selector::SINK_KIND, expected),
    ] {
        let default = installed_std::test_structured_selector::raw_configuration(value)
            .pop()
            .and_then(|entry| match entry.value {
                conduit_core::ConfigurationValue::Text(value) => Some(value),
                _ => None,
            })
            .unwrap();
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![StartupParameterSignature {
                    name: "value".into(),
                    value_type: "Text".into(),
                    default: Some(format!("\"{default}\"")),
                }],
            })
            .unwrap();
    }
    if math_call {
        conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut ProfileCatalog::new())
            .unwrap();
    }
    let source = format!(
        "plot pipeline {{\n source: {}\n sink: {}\n source >> {stage_source} >> sink\n}}\n",
        installed_std::test_structured_selector::SOURCE_KIND,
        installed_std::test_structured_selector::SINK_KIND,
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).expect("expression pipeline checks");

    let mut source_offer = installed_std::test_structured_selector::raw_source_offer(
        installed_std::test_structured_selector::SOURCE_KIND,
        input_kind,
    );
    let mut sink_offer = installed_std::test_structured_selector::raw_sink_offer(
        installed_std::test_structured_selector::SINK_KIND,
        output_kind,
    );
    if filter || math_call {
        source_offer.outputs[0].temporal = conduit_core::PortTemporal::Value;
        sink_offer.inputs[0].temporal = conduit_core::PortTemporal::Value;
    }
    let mut profile = ProfileCatalog::new();
    let source_definition = fixture_definition(&source_offer, input);
    source_offer.semantic_contract.configuration = source_definition.configuration.clone();
    profile.insert(source_definition).unwrap();
    let sink_definition = fixture_definition(&sink_offer, expected);
    sink_offer.semantic_contract.configuration = sink_definition.configuration.clone();
    profile.insert(sink_definition).unwrap();
    if math_call {
        let mut unused_startup = StartupCatalog::new();
        conduit_semantic_catalog::install_math_catalogs(&mut unused_startup, &mut profile).unwrap();
    }
    let expanded = expand_canonical_plot(&checked, "pipeline", &profile)
        .expect("pure expression expands to an ordinary gear");
    let expression = expanded
        .gears
        .iter()
        .find(|gear| {
            gear.kind_contract_revision.as_str()
                == if filter {
                    conduit_plot::PURE_FILTER_REVISION
                } else {
                    conduit_plot::PURE_EXPRESSION_REVISION
                }
        })
        .expect("expanded expression gear exists");
    let conduit_core::ConfigurationValue::Text(program) = &expression.configuration[0].value else {
        panic!("expression has exact portable program configuration")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let expression_offer = if filter {
        conduit_std_offers::pure_filter_std_offer(&program, expression.inputs[0].temporal).unwrap()
    } else {
        conduit_std_offers::pure_expression_std_offer(&program, expression.inputs[0].temporal)
            .unwrap()
    };
    if math_call {
        let gear = expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::MATH_CLAMP_KIND)
            .unwrap();
        let offer = conduit_std_offers::math_clamp_offer();
        assert!(
            gear.accepts_realization(&offer),
            "called math Gear front {:?} differs from offer {:?}",
            gear.checked_front(),
            offer.checked_front()
        );
    }

    let mut advertisement = host("pure-expression-host").advertisement().clone();
    advertisement
        .capabilities
        .extend([source_offer, expression_offer.clone(), sink_offer]);
    if math_call {
        advertisement
            .capabilities
            .push(conduit_std_offers::math_clamp_offer());
    }
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .expect("exact expression capability places");
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: if math_call {
                conduit_core::SCALAR_ENCODED_LEN as u32
            } else {
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
            },
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("expression pipeline plans locally");
    assert!(plan.fragments[0].placements.iter().any(|placement| {
        placement.implementation_id.as_str()
            == if filter {
                conduit_std_offers::PURE_FILTER_STD_IMPLEMENTATION
            } else {
                conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION
            }
    }));

    let mut output = Vec::with_capacity(2_048);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let mut sign_sequence = 0;
    let report = installed_std::run_fragment(
        installed_std::InstalledRunHost {
            advertisement: &advertisement,
            playback: None,
            midi_input: None,
            midi_output: None,
            keyboard: None,
            local_model: None,
            vector_search: None,
            model_work: None,
            calendar: None,
            body_conversation_context: None,
        },
        &plan.fragments[0],
        0,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("expression executes through the production kernel");
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}

fn fixture_definition(offer: &conduit_core::CapabilityOffer, value: &[u8]) -> KindProjection {
    let entry = installed_std::test_structured_selector::raw_configuration(value)
        .pop()
        .unwrap();
    KindProjection {
        kind_id: offer.kind_id.clone(),
        kind_contract_revision: KindIdentity::from(offer.kind_contract_revision.as_str()),
        inputs: offer.inputs.clone(),
        outputs: offer.outputs.clone(),
        configuration: vec![KindConfigurationField {
            key: entry.key,
            default_value: entry.value,
            rule: KindConfigurationRule::TextBytes {
                maximum: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
            },
        }],
    }
}
