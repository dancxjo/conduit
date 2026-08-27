use super::{host, installed_std, RecordingTimer};
use conduit_core::{BaseImplementationId, ConfigurationValue};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

const SINK: &str = "conduit-test/recurrence-sink";

#[test]
fn checked_civil_recurrence_executes_through_the_production_kernel() {
    let source = r#"form meeting {
  expand: time/expand-recurrence({ excluded_ordinals: [unused(""), unused(""), unused(""), unused("")], fold_policy: "earlier", gap_policy: "skip", identity: "recurrence/weekly-meeting", maximum_occurrences: 3, maximum_results: 2, resolutions: [unique({ instant: { basis: "utc", resolution_ticks: 1, scale: "seconds", ticks: 100 }, local_date: "2026-03-02", local_time: "09:00:00", ordinal: 0, rule_set: "tzdb/2026a", zone: "America/Los_Angeles" }), unique({ instant: { basis: "utc", resolution_ticks: 1, scale: "seconds", ticks: 300 }, local_date: "2026-03-16", local_time: "09:00:00", ordinal: 2, rule_set: "tzdb/2026a", zone: "America/Los_Angeles" }), unused(""), unused(""), unused(""), unused(""), unused(""), unused("")], rule: civil_weekdays({ excluded_dates: [exclude("2026-03-09"), unused(""), unused(""), unused("")], first_date: "2026-03-02", local_time: "09:00:00", rule_set: "tzdb/2026a", weekdays: 1, zone: "America/Los_Angeles" }), until: civil_date("2026-03-16"), window: wall({ end: { basis: "utc", resolution_ticks: 1, scale: "seconds", ticks: 300 }, start: { basis: "utc", resolution_ticks: 1, scale: "seconds", ticks: 100 } }) })
  sink: conduit-test/recurrence-sink(2)
  expand.occurrences > sink.occurrences
}
"#;
    let sink_offer = installed_std::test_recurrence_sink_offer();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_recurrence_catalogs(&mut startup, &mut profile).unwrap();
    install_sink(&mut startup, &mut profile, &sink_offer);
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "meeting", &profile).unwrap();

    let mut advertisement = host("recurrence-play-host").advertisement().clone();
    advertisement.capabilities.push(sink_offer);
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
            connection_item_capacity: 2,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let planned = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::RECURRENCE_KIND)
        .unwrap();
    assert!(matches!(
        planned.configuration[0].value,
        ConfigurationValue::Structured(_)
    ));

    let mut output = Vec::with_capacity(2_048);
    let mut timer = RecordingTimer { waits: vec![] };
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
    .expect("checked recurrence executes through production Play");
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
}

fn install_sink(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    offer: &conduit_core::CapabilityOffer,
) {
    startup
        .insert(KindSignature {
            kind: SINK.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "expected".into(),
                value_type: "Count".into(),
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
                key: "expected".into(),
                default_value: ConfigurationValue::U64(1),
                validation: ConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: u64::from(conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS),
                },
            }],
        })
        .unwrap();
}
