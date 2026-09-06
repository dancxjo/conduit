use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    BaseImplementationId, ConfigurationValue, PortDirection, PortTemporal, StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

const CANDIDATE_KIND: &str = "conduit-test/gesture-candidate";
const TEMPLATE_KIND: &str = "conduit-test/gesture-template";
const SINK_KIND: &str = "conduit-test/comparison-result";

#[test]
fn reusable_pattern_comparison_executes_with_explicit_policy_through_one_play() {
    let candidate =
        conduit_semantic_catalog::normalized_value(&[500_000, 1_000_000, 760_000]).unwrap();
    let template =
        conduit_semantic_catalog::normalized_value(&[500_000, 1_000_000, 700_000]).unwrap();
    let comparison = conduit_semantic_catalog::compare_normalized_patterns(
        &candidate,
        &template,
        conduit_semantic_catalog::MAXIMUM_ABSOLUTE_METRIC,
        60_000,
    )
    .unwrap();
    let candidate_offer = fixture_offer(&candidate, PortDirection::Output, CANDIDATE_KIND);
    let template_offer = fixture_offer(&template, PortDirection::Output, TEMPLATE_KIND);
    let sink_offer = fixture_offer(&comparison, PortDirection::Input, SINK_KIND);
    let (startup, profile) = catalogs([
        (CANDIDATE_KIND, &candidate, &candidate_offer),
        (TEMPLATE_KIND, &template, &template_offer),
        (SINK_KIND, &comparison, &sink_offer),
    ]);
    let source = format!(
        "{}\nform gesture-cadence-proof {{\n    candidate: {CANDIDATE_KIND}(value = \"{}\")\n    template: {TEMPLATE_KIND}(value = \"{}\")\n    compare: compare-pattern(tolerance-millionths = 60000)\n    result: {SINK_KIND}(value = \"{}\")\n    candidate.output > compare.candidate\n    template.output > compare.template\n    compare.comparison > result.input\n}}\n",
        include_str!("../../../../forms/secret-knock/main.conduit"),
        hex(&candidate.canonical_bytes().unwrap()),
        hex(&template.canonical_bytes().unwrap()),
        hex(&comparison.canonical_bytes().unwrap()),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let canonical = check_syntax_document(
        &parse_syntax_document(include_str!("../../../../forms/secret-knock/main.conduit")),
        &startup,
    )
    .unwrap();
    assert_eq!(
        canonical
            .forms
            .iter()
            .find(|form| form.name == "compare-pattern")
            .unwrap()
            .checked_form_id,
        checked
            .forms
            .iter()
            .find(|form| form.name == "compare-pattern")
            .unwrap()
            .checked_form_id
    );
    let expanded = expand_canonical_form(&checked, "gesture-cadence-proof", &profile).unwrap();
    assert!(expanded.gears.iter().any(|gear| {
        gear.kind_id.as_str() == conduit_semantic_catalog::COMPARE_PATTERN_KIND
            && gear.configuration.iter().any(|entry| {
                entry.key == "tolerance-millionths"
                    && entry.value == ConfigurationValue::U64(60_000)
            })
    }));

    let mut advertisement = host("pattern-comparison-host").advertisement().clone();
    advertisement.capabilities.extend([
        candidate_offer,
        template_offer,
        conduit_std_offers::compare_pattern_std_offer(),
        sink_offer,
    ]);
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
    assert_eq!(plan.fragments[0].placements.len(), 4);
    let mut output = Vec::with_capacity(1_024);
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
    .expect("comparison executes through the production kernel");
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(kernel.identity.lengths(), (2, 0, 1));
}

fn catalogs<const N: usize>(
    fixtures: [(&str, &StructuredInfoValue, &conduit_core::CapabilityOffer); N],
) -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    super::timing_form_catalogs::install_catalogs(&mut startup, &mut profile);
    for (kind, value, offer) in fixtures {
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
    (startup, profile)
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
