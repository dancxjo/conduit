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

const SOURCE_KIND: &str = "conduit-test/duration-sequence";
const SINK_KIND: &str = "conduit-test/normalized-sequence";

#[test]
fn reusable_normalization_form_executes_outside_secret_knock_without_play_allocations() {
    let events = conduit_semantic_catalog::timed_event_sequence_value(
        "fixture/protocol-microseconds",
        &[1_000, 1_200, 1_800, 2_200],
    )
    .unwrap();
    let intervals = conduit_semantic_catalog::derive_intervals(&events).unwrap();
    let normalized = conduit_semantic_catalog::normalize_relative_durations(&intervals).unwrap();
    let source_offer = fixture_offer(&intervals, PortDirection::Output);
    let sink_offer = fixture_offer(&normalized, PortDirection::Input);
    let (startup, profile) = catalogs(&intervals, &normalized, &source_offer, &sink_offer);
    let source = format!(
        "{}\nform protocol-cadence-proof {{\n    intervals: {SOURCE_KIND}(value = \"{}\")\n    normalize: normalize-durations\n    result: {SINK_KIND}(value = \"{}\")\n    intervals.output > normalize.intervals\n    normalize.normalized > result.input\n}}\n",
        include_str!("../../../../forms/secret-knock/main.conduit"),
        hex(&intervals.canonical_bytes().unwrap()),
        hex(&normalized.canonical_bytes().unwrap()),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let canonical = check_syntax_document(
        &parse_syntax_document(include_str!("../../../../forms/secret-knock/main.conduit")),
        &startup,
    )
    .unwrap();
    let canonical_normalize = canonical
        .forms
        .iter()
        .find(|form| form.name == "normalize-durations")
        .unwrap();
    let consumed_normalize = checked
        .forms
        .iter()
        .find(|form| form.name == "normalize-durations")
        .unwrap();
    assert_eq!(
        canonical_normalize.checked_form_id,
        consumed_normalize.checked_form_id
    );
    let consumer = checked
        .forms
        .iter()
        .find(|form| form.name == "protocol-cadence-proof")
        .unwrap();
    assert!(consumer
        .gears
        .iter()
        .any(|gear| gear.kind == "normalize-durations"));
    let expanded = expand_canonical_form(&checked, "protocol-cadence-proof", &profile).unwrap();
    assert!(expanded.gears.iter().any(|gear| {
        gear.kind_id.as_str() == conduit_semantic_catalog::NORMALIZE_SEQUENCE_KIND
    }));

    let mut advertisement = host("normalization-host").advertisement().clone();
    advertisement.capabilities.extend([
        source_offer,
        conduit_std_offers::normalize_sequence_std_offer(),
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
    assert_eq!(plan.fragments[0].placements.len(), 3);

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
    .expect("normalization executes through the production kernel");
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(kernel.identity.lengths(), (1, 0, 1));
}

fn catalogs(
    input: &StructuredInfoValue,
    output: &StructuredInfoValue,
    source_offer: &conduit_core::CapabilityOffer,
    sink_offer: &conduit_core::CapabilityOffer,
) -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    super::timing_form_catalogs::install_catalogs(&mut startup, &mut profile);
    for (kind, value, offer) in [
        (SOURCE_KIND, input, source_offer),
        (SINK_KIND, output, sink_offer),
    ] {
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
) -> conduit_core::CapabilityOffer {
    let mut offer = installed_std::test_structured_selector::offer_named(
        value.value_type(),
        direction,
        SOURCE_KIND,
        SINK_KIND,
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
