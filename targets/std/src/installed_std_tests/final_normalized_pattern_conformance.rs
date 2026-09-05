use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    BaseImplementationId, ConfigurationValue, PortDirection, PortTemporal, ResourceClassId,
    ResourceOffer, ResourcePoolId, StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

const SOURCE_KIND: &str = "conduit-test/calibration-pattern-revisions";
const SINK_KIND: &str = "conduit-test/final-calibration-pattern";

#[test]
fn final_pattern_adapter_is_reused_for_calibration_revisions() {
    let first = conduit_semantic_catalog::normalized_value(&[300_000, 1_000_000]).unwrap();
    let final_value = conduit_semantic_catalog::normalized_value(&[350_000, 1_000_000]).unwrap();
    let source_offer = sequence_source_offer(&first);
    let sink_offer = fixture_offer(&final_value, PortDirection::Input, SINK_KIND);
    let (startup, profile) = catalogs(&first, &final_value, &source_offer, &sink_offer);
    let values = [&first, &final_value]
        .iter()
        .map(|value| hex(&value.canonical_bytes().unwrap()))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        "form calibration-proof {{\n revisions: {SOURCE_KIND}(values = \"{values}\")\n final: sequence/final-normalized-pattern(maximum-values = 2)\n sink: {SINK_KIND}(value = \"{}\")\n revisions.output > final.patterns\n final.pattern > sink.input\n}}\n",
        hex(&final_value.canonical_bytes().unwrap()),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "calibration-proof", &profile).unwrap();

    let mut advertisement = host("calibration-pattern-host").advertisement().clone();
    advertisement.capabilities.extend([
        source_offer,
        conduit_std_offers::final_normalized_pattern_std_offer(),
        sink_offer,
    ]);
    if !advertisement
        .resources
        .iter()
        .any(|resource| resource.class_id.as_str() == conduit_core::TIMER_RESOURCE_CLASS)
    {
        advertisement.resources.push(ResourceOffer {
            pool_id: ResourcePoolId::from("pool/calibration-fixture-clock"),
            class_id: ResourceClassId::from(conduit_core::TIMER_RESOURCE_CLASS),
            capacity_units: 1,
            compute: None,
        });
    }
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
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
    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(2),
    };
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
    .expect("final pattern executes through the production kernel");
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}

fn catalogs(
    first: &StructuredInfoValue,
    final_value: &StructuredInfoValue,
    source_offer: &conduit_core::CapabilityOffer,
    sink_offer: &conduit_core::CapabilityOffer,
) -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_final_normalized_pattern_catalogs(&mut startup, &mut profile)
        .unwrap();
    for (kind, parameter, value, offer) in [
        (SOURCE_KIND, "values", first, source_offer),
        (SINK_KIND, "value", final_value, sink_offer),
    ] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![StartupParameterSignature {
                    name: parameter.into(),
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
                    key: parameter.into(),
                    default_value: ConfigurationValue::Text(hex(&value.canonical_bytes().unwrap())),
                    validation: ConfigurationRule::TextBytes {
                        maximum: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
                    },
                }],
            })
            .unwrap();
    }
    (startup, profile)
}

fn sequence_source_offer(value: &StructuredInfoValue) -> conduit_core::CapabilityOffer {
    let mut offer = fixture_offer(value, PortDirection::Output, SOURCE_KIND);
    offer.startup_parameters[0].name = "values".into();
    offer.host_operations = vec![conduit_core::wait_host_operation_requirement()];
    offer.resource_requirements = vec![conduit_core::resource_requirement(
        conduit_core::TIMER_RESOURCE_CLASS,
        1,
    )];
    offer
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
    offer.startup_parameters[0].has_default = false;
    let port = offer
        .outputs
        .first_mut()
        .or_else(|| offer.inputs.first_mut())
        .unwrap();
    port.temporal = if direction == PortDirection::Output {
        PortTemporal::Flow { closes: true }
    } else {
        PortTemporal::Value
    };
    offer
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
