use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    BaseImplementationId, ConfigurationValue, PortDirection, PortTemporal, StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document,
    structured_selector_definition, ConfigurationField, ConfigurationRule, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use std::collections::BTreeMap;

const FOUND_KIND: &str = "conduit-test/found-template";
const CANDIDATE_KIND: &str = "conduit-test/candidate-pattern";
const SINK_KIND: &str = "conduit-test/template-comparison";

#[test]
fn found_storage_result_feeds_reusable_comparison_through_checked_selectors() {
    let pattern =
        conduit_semantic_catalog::normalized_value(&[400_000, 1_000_000, 700_000]).unwrap();
    let candidate =
        conduit_semantic_catalog::normalized_value(&[420_000, 1_000_000, 690_000]).unwrap();
    let found = conduit_semantic_catalog::found_template_result("service-cadence", pattern.clone())
        .unwrap();
    let comparison = conduit_semantic_catalog::compare_normalized_patterns(
        &candidate,
        &pattern,
        conduit_semantic_catalog::MAXIMUM_ABSOLUTE_METRIC,
        20_000,
    )
    .unwrap();
    let found_offer = fixture_offer(&found, PortDirection::Output, FOUND_KIND);
    let candidate_offer = fixture_offer(&candidate, PortDirection::Output, CANDIDATE_KIND);
    let sink_offer = fixture_offer(&comparison, PortDirection::Input, SINK_KIND);
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_pattern_comparison_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_template_storage_catalogs(&mut startup, &mut profile)
        .unwrap();
    for (kind, value, offer) in [
        (FOUND_KIND, &found, &found_offer),
        (CANDIDATE_KIND, &candidate, &candidate_offer),
        (SINK_KIND, &comparison, &sink_offer),
    ] {
        install_fixture(&mut startup, &mut profile, kind, value, offer);
    }
    let source = format!(
        "form proof {{\n found: {FOUND_KIND}(value = \"{}\")\n candidate: {CANDIDATE_KIND}(value = \"{}\")\n compare: sequence/compare-normalized-pattern(metric = \"{}\", tolerance-millionths = 20000)\n sink: {SINK_KIND}(value = \"{}\")\n found > select(NamedPatternTemplateResult.found, unmatched=refuse) > project(NamedPatternTemplate.pattern) > compare.template\n candidate.output > compare.candidate\n compare.comparison > sink.input\n}}\n",
        hex(&found.canonical_bytes().unwrap()),
        hex(&candidate.canonical_bytes().unwrap()),
        conduit_semantic_catalog::MAXIMUM_ABSOLUTE_METRIC,
        hex(&comparison.canonical_bytes().unwrap()),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let selectors = checked.forms[0]
        .cords
        .iter()
        .flat_map(|cord| &cord.stages)
        .filter_map(|stage| match stage {
            conduit_form::CheckedCordStage::StructuredSelector { selector, .. } => Some(selector),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(selectors.len(), 2);
    for selector in &selectors {
        profile
            .insert(structured_selector_definition(
                selector,
                PortTemporal::Value,
            ))
            .unwrap();
    }
    let expanded = expand_canonical_form(&checked, "proof", &profile).unwrap();
    let selector_offers = selectors
        .iter()
        .map(|selector| {
            conduit_std_offers::structured_selector_std_offer(selector, PortTemporal::Value)
        })
        .collect::<Vec<_>>();

    let mut advertisement = host("template-selection-host").advertisement().clone();
    advertisement.capabilities.extend([
        found_offer,
        candidate_offer,
        sink_offer,
        conduit_std_offers::compare_pattern_std_offer(),
    ]);
    advertisement.capabilities.extend(selector_offers);
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    assert_eq!(plan.fragments[0].placements.len(), 6);
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
        },
        &plan.fragments[0],
        0,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("stored template selection and comparison execute through one kernel");
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}

fn install_fixture(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    kind: &str,
    value: &StructuredInfoValue,
    offer: &conduit_core::CapabilityOffer,
) {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Text".into(),
                default: None,
            }],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            inputs: offer.inputs.clone(),
            outputs: offer.outputs.clone(),
            configuration: vec![ConfigurationField {
                key: "value".into(),
                default_value: ConfigurationValue::Text(hex(&value.canonical_bytes().unwrap())),
                validation: ConfigurationRule::TextBytes {
                    maximum: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
                },
            }],
        })
        .unwrap();
}

fn fixture_offer(
    value: &StructuredInfoValue,
    direction: PortDirection,
    kind: &str,
) -> conduit_core::CapabilityOffer {
    let mut offer = installed_std::test_structured_selector::offer_named(
        value.value_type(),
        direction,
        kind,
        kind,
    );
    let port = offer
        .outputs
        .first_mut()
        .or_else(|| offer.inputs.first_mut())
        .unwrap();
    port.temporal = PortTemporal::Value;
    offer.startup_parameters[0].has_default = false;
    offer
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
