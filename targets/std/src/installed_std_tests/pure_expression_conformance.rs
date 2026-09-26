use super::{host, installed_std, RecordingTimer};
use conduit_core::{BaseImplementationId, KindIdentity};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindConfigurationField,
    KindConfigurationRule, KindProjection, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

#[test]
fn fixed_integer_expression_plans_and_plays_through_the_std_host() {
    let input = vec![41];
    let expected = vec![42];
    let mut startup = StartupCatalog::new();
    for (kind, value) in [
        (installed_std::test_structured_selector::SOURCE_KIND, &input),
        (
            installed_std::test_structured_selector::SINK_KIND,
            &expected,
        ),
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
    let source = format!(
        "form pipeline {{\n source: {}\n sink: {}\n source >> (. + 1) >> sink\n}}\n",
        installed_std::test_structured_selector::SOURCE_KIND,
        installed_std::test_structured_selector::SINK_KIND,
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).expect("expression pipeline checks");

    let source_offer = installed_std::test_structured_selector::raw_source_offer(
        installed_std::test_structured_selector::SOURCE_KIND,
        "value/u8",
    );
    let sink_offer = installed_std::test_structured_selector::raw_sink_offer(
        installed_std::test_structured_selector::SINK_KIND,
        "value/u8",
    );
    let mut profile = ProfileCatalog::new();
    profile
        .insert(fixture_definition(&source_offer, &input))
        .unwrap();
    profile
        .insert(fixture_definition(&sink_offer, &expected))
        .unwrap();
    let expanded = expand_canonical_form(&checked, "pipeline", &profile)
        .expect("pure expression expands to an ordinary gear");
    let expression = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_contract_revision.as_str() == conduit_form::PURE_EXPRESSION_REVISION)
        .expect("expanded expression gear exists");
    let conduit_core::ConfigurationValue::Text(program) = &expression.configuration[0].value else {
        panic!("expression has exact portable program configuration")
    };
    let program = conduit_form::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let expression_offer =
        conduit_std_offers::pure_expression_std_offer(&program, expression.inputs[0].temporal)
            .unwrap();

    let mut advertisement = host("pure-expression-host").advertisement().clone();
    advertisement
        .capabilities
        .extend([source_offer, expression_offer.clone(), sink_offer]);
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
            connection_item_capacity: 4,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("expression pipeline plans locally");
    assert!(plan.fragments[0].placements.iter().any(|placement| {
        placement.implementation_id.as_str()
            == conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION
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
