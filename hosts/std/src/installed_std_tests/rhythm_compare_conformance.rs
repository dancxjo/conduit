use super::{host, installed_std, RecordingTimer};
use conduit_audio::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId};
use conduit_core::{
    BaseImplementationId, KindContractRevision, KindId, PortDirection, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

const PERFORMANCE_SOURCE: &str = "conduit-test/rhythm-performance-source";
const REFERENCE_SOURCE: &str = "conduit-test/rhythm-reference-source";
const FEEDBACK_SINK: &str = "conduit-test/rhythm-feedback-sink";

#[test]
fn portable_lesson_executes_with_generic_structured_sources() {
    let performance = note(1_020);
    let reference = record(
        conduit_std_catalog::beat_reference_type(),
        [("beat", 1), ("expected_time_micros", 1_000)],
    );
    let feedback =
        installed_std::rhythm_compare_host::expected_feedback(1, 1_000, Some(1_020), 0, 30_000);
    let reference_type = conduit_std_catalog::beat_reference_type();
    let feedback_type = conduit_std_catalog::timing_feedback_type();

    let performance_offer = installed_std::test_structured_selector::raw_source_offer(
        PERFORMANCE_SOURCE,
        conduit_audio::MUSIC_NOTE_INFO_ID,
    );
    let reference_offer = installed_std::test_structured_selector::offer_named(
        &reference_type,
        PortDirection::Output,
        REFERENCE_SOURCE,
        FEEDBACK_SINK,
    );
    let sink_offer = installed_std::test_structured_selector::offer_named(
        &feedback_type,
        PortDirection::Input,
        PERFORMANCE_SOURCE,
        FEEDBACK_SINK,
    );
    let compare_offer = conduit_std_offers::rhythm_compare_std_offer();

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_structured_music_form_catalogs(&mut startup, &mut profile)
        .unwrap();
    for (kind, value) in [(REFERENCE_SOURCE, &reference), (FEEDBACK_SINK, &feedback)] {
        install_fixture(
            &mut startup,
            &mut profile,
            kind,
            value,
            [&performance_offer, &reference_offer, &sink_offer]
                .into_iter()
                .find(|offer| offer.kind_id.as_str() == kind)
                .unwrap(),
        );
    }
    install_raw_fixture(
        &mut startup,
        &mut profile,
        PERFORMANCE_SOURCE,
        &performance,
        &performance_offer,
    );

    let source = format!(
        "form lesson {{\n performance: {PERFORMANCE_SOURCE}\n reference: {REFERENCE_SOURCE}\n feedback: {FEEDBACK_SINK}\n compare: music/rhythm-compare(target-offset-micros = 0, tolerance-micros = 30000)\n performance > compare.performance\n reference > compare.reference\n compare.feedback > feedback\n}}\n"
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "lesson", &profile).unwrap();

    let mut advertisement = host("rhythm-lesson-host").advertisement().clone();
    advertisement.capabilities.extend([
        performance_offer,
        reference_offer,
        sink_offer,
        compare_offer.clone(),
    ]);
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
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
            connection_item_capacity: 4,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let compare = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id == compare_offer.kind_id)
        .unwrap();
    assert_eq!(compare.host_operations.len(), 3);

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
    .expect("portable rhythm lesson executes through the production kernel");
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}

fn install_fixture(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    kind: &str,
    value: &StructuredInfoValue,
    offer: &conduit_core::CapabilityOffer,
) {
    let entry = installed_std::test_structured_selector::configuration(value)
        .pop()
        .unwrap();
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Text".into(),
                default: match &entry.value {
                    conduit_core::ConfigurationValue::Text(value) => Some(format!("\"{value}\"")),
                    _ => unreachable!(),
                },
            }],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: KindId::from(kind),
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
        })
        .unwrap();
}

fn install_raw_fixture(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    kind: &str,
    value: &[u8],
    offer: &conduit_core::CapabilityOffer,
) {
    let entry = installed_std::test_structured_selector::raw_configuration(value)
        .pop()
        .unwrap();
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Text".into(),
                default: match &entry.value {
                    conduit_core::ConfigurationValue::Text(value) => Some(format!("\"{value}\"")),
                    _ => unreachable!(),
                },
            }],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: KindId::from(kind),
            kind_contract_revision: KindContractRevision::from(
                offer.kind_contract_revision.as_str().to_string(),
            ),
            inputs: Vec::new(),
            outputs: offer.outputs.clone(),
            configuration: vec![ConfigurationField {
                key: entry.key,
                default_value: entry.value,
                validation: ConfigurationRule::TextBytes { maximum: 512 },
            }],
        })
        .unwrap();
}

fn note(time: u64) -> Vec<u8> {
    MusicalNoteEvent::new(
        NoteOccurrenceId(1),
        MusicalPitch::new(440_000, 440_000, 0).unwrap(),
        Gate::On,
        u16::MAX,
        time,
        0,
    )
    .unwrap()
    .encode()
    .to_vec()
}

fn record(value_type: StructuredInfoType, values: [(&str, u64); 2]) -> StructuredInfoValue {
    let count = StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap();
    StructuredInfoValue::record(
        value_type,
        values
            .into_iter()
            .map(|(name, value)| {
                StructuredFieldValue::new(
                    name,
                    StructuredInfoValue::leaf(count.clone(), value.to_string().into_bytes())
                        .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
