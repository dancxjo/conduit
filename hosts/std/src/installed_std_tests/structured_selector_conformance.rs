use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    ConnectionBase, KindContractRevision, KindId, PortDirection, PortTemporal, StructuredFieldType,
    StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document,
    structured_selector_definition, ConfigurationField, ConfigurationRule, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use std::collections::BTreeMap;

#[test]
fn music_and_llm_records_execute_the_same_planned_selector_infrastructure() {
    let count = StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap();
    let text = StructuredInfoType::leaf(KindId::from("value/text@1")).unwrap();
    let scalar = StructuredInfoType::leaf(KindId::from("value/scalar@1")).unwrap();

    let midi = StructuredInfoType::record(
        KindId::from("music/midi-note@1"),
        vec![
            StructuredFieldType::new("channel", count.clone()).unwrap(),
            StructuredFieldType::new("velocity", count.clone()).unwrap(),
        ],
    )
    .unwrap();
    let velocity = StructuredInfoValue::leaf(count.clone(), 96_u64.to_le_bytes().to_vec()).unwrap();
    let midi_value = StructuredInfoValue::record(
        midi.clone(),
        vec![
            StructuredFieldValue::new(
                "channel",
                StructuredInfoValue::leaf(count, 2_u64.to_le_bytes().to_vec()).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("velocity", velocity.clone()).unwrap(),
        ],
    )
    .unwrap();
    execute_case("MidiEvent", midi, midi_value, "velocity", velocity);

    let extraction = StructuredInfoType::record(
        KindId::from("llm/extraction@1"),
        vec![
            StructuredFieldType::new("confidence", scalar.clone()).unwrap(),
            StructuredFieldType::new("label", text.clone()).unwrap(),
        ],
    )
    .unwrap();
    let label = StructuredInfoValue::leaf(text, b"ready".to_vec()).unwrap();
    let extraction_value = StructuredInfoValue::record(
        extraction.clone(),
        vec![
            StructuredFieldValue::new(
                "confidence",
                StructuredInfoValue::leaf(scalar, 750_000_i64.to_le_bytes().to_vec()).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("label", label.clone()).unwrap(),
        ],
    )
    .unwrap();
    execute_case("Extraction", extraction, extraction_value, "label", label);
}

fn execute_case(
    type_name: &str,
    input_type: StructuredInfoType,
    input: StructuredInfoValue,
    field: &str,
    expected: StructuredInfoValue,
) {
    let temporal = PortTemporal::Flow { closes: true };
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type(type_name, input_type.clone())
        .unwrap();
    for (kind, value) in [
        (installed_std::test_structured_selector::SOURCE_KIND, &input),
        (
            installed_std::test_structured_selector::SINK_KIND,
            &expected,
        ),
    ] {
        let default = installed_std::test_structured_selector::configuration(value)
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
        "form pipeline {{\n source: {}\n sink: {}\n source > project({type_name}.{field}) > sink\n}}\n",
        installed_std::test_structured_selector::SOURCE_KIND,
        installed_std::test_structured_selector::SINK_KIND,
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).expect("structured pipeline checks");
    let conduit_form::CheckedCordStage::StructuredSelector { selector, .. } =
        &checked.forms[0].cords[0].stages[1]
    else {
        panic!("middle stage is the checked selector");
    };

    let source_offer =
        installed_std::test_structured_selector::offer(&input_type, PortDirection::Output);
    let sink_offer =
        installed_std::test_structured_selector::offer(expected.value_type(), PortDirection::Input);
    let selector_offer = conduit_std_catalog::structured_selector_std_offer(selector, temporal);
    let mut profile = ProfileCatalog::new();
    profile
        .insert(fixture_definition(&source_offer, &input))
        .unwrap();
    profile
        .insert(fixture_definition(&sink_offer, &expected))
        .unwrap();
    profile
        .insert(structured_selector_definition(selector, temporal))
        .unwrap();
    let expanded = expand_canonical_form(&checked, "pipeline", &profile)
        .expect("structured selector expands to ordinary gears");
    assert!(
        expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id == selector_offer.kind_id),
        "selector offer kind {:?} absent from expanded gears {:?}",
        selector_offer.kind_id,
        expanded
            .gears
            .iter()
            .map(|gear| gear.kind_id.as_str())
            .collect::<Vec<_>>()
    );

    let mut advertisement = host("structured-selector-host").advertisement().clone();
    advertisement
        .capabilities
        .extend([source_offer, selector_offer.clone(), sink_offer]);
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .expect("exact structured capabilities place");
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
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
    .expect("structured pipeline plans locally");
    let selector_placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id == selector_offer.kind_id)
        .expect("selector has one planned placement");
    assert_eq!(
        selector_placement.implementation_id.as_str(),
        conduit_std_catalog::STRUCTURED_SELECTOR_STD_IMPLEMENTATION
    );

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
        },
        &plan.fragments[0],
        0,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("structured pipeline executes through the production kernel");
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}

fn fixture_definition(
    offer: &conduit_core::CapabilityOffer,
    value: &StructuredInfoValue,
) -> KindDefinition {
    let entry = installed_std::test_structured_selector::configuration(value)
        .pop()
        .unwrap();
    KindDefinition {
        kind_id: offer.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(
            offer.kind_contract_revision.as_str().to_string(),
        ),
        inputs: offer.inputs.clone(),
        outputs: offer.outputs.clone(),
        configuration: vec![ConfigurationField {
            key: entry.key,
            default_value: entry.value,
            validation: ConfigurationRule::TextBytes {
                maximum: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
            },
        }],
    }
}
