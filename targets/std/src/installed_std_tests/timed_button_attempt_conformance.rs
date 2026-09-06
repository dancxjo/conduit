use super::{host, installed_std};
use crate::TimerAdapter;
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
use std::time::Duration;

const SOURCE_KIND: &str = "conduit-test/button-transition-sequence";
const SINK_KIND: &str = "conduit-test/timed-button-attempt";

struct AttemptTimer {
    micros: [u64; 4],
    next: usize,
}

impl TimerAdapter for AttemptTimer {
    fn wait(&mut self, _: Duration) {}

    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(0)
    }

    fn monotonic_now_micros(&mut self) -> Option<u64> {
        let value = self.micros.get(self.next).copied()?;
        self.next += 1;
        Some(value)
    }

    fn wait_until_monotonic_ms(&mut self, _: u64) -> bool {
        panic!("a reset deadline must not fire before the finite attempt completes")
    }
}

#[test]
fn portable_button_flow_becomes_one_timed_attempt_in_the_production_kernel() {
    let transitions = [
        conduit_semantic_catalog::button_transition_value("button/primary", true, 1).unwrap(),
        conduit_semantic_catalog::button_transition_value("button/primary", false, 2).unwrap(),
        conduit_semantic_catalog::button_transition_value("button/primary", true, 3).unwrap(),
        conduit_semantic_catalog::button_transition_value("button/primary", true, 4).unwrap(),
    ];
    let expected = conduit_semantic_catalog::timed_event_sequence_value(
        "host/boot-monotonic-microseconds@1",
        &[100, 250, 700],
    )
    .unwrap();
    let source_offer = sequence_source_offer(&transitions[0]);
    let sink_offer = fixture_offer(&expected, PortDirection::Input, SINK_KIND);
    let (startup, profile) = catalogs(&transitions[0], &expected, &source_offer, &sink_offer);
    let encoded = transitions
        .iter()
        .map(|value| hex(&value.canonical_bytes().unwrap()))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        "form proof {{\n transitions: {SOURCE_KIND}(values = \"{encoded}\")\n attempt: time/pressed-button-attempt(maximum-presses = 3, maximum-transitions = 4, timeout-ms = 1000ms)\n sink: {SINK_KIND}(value = \"{}\")\n transitions.output > attempt.transition\n attempt.events > sink.input\n}}\n",
        hex(&expected.canonical_bytes().unwrap()),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "proof", &profile).unwrap();

    let mut advertisement = host("timed-attempt-host").advertisement().clone();
    advertisement.capabilities.extend([
        source_offer,
        conduit_std_offers::timed_button_attempt_std_offer(),
        sink_offer,
    ]);
    for (pool, class) in [
        ("pool/attempt-clock", conduit_core::TIMER_RESOURCE_CLASS),
        (
            "pool/attempt-deadline",
            conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
        ),
    ] {
        if !advertisement
            .resources
            .iter()
            .any(|resource| resource.class_id.as_str() == class)
        {
            advertisement.resources.push(ResourceOffer {
                content: None,
                pool_id: ResourcePoolId::from(pool),
                class_id: ResourceClassId::from(class),
                capacity_units: 1,
                compute: None,
            });
        }
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
            connection_item_capacity: 4,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let attempt = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::TIMED_BUTTON_ATTEMPT_KIND
        })
        .unwrap();
    assert_eq!(attempt.resources.len(), 2);

    let mut output = Vec::with_capacity(2_048);
    let mut timer = AttemptTimer {
        micros: [100, 120, 250, 700],
        next: 0,
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
    .expect("timed button attempt executes through the production kernel");
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}

fn catalogs(
    transition: &StructuredInfoValue,
    expected: &StructuredInfoValue,
    source_offer: &conduit_core::CapabilityOffer,
    sink_offer: &conduit_core::CapabilityOffer,
) -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_generalized_input_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_timed_pattern_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_timed_button_attempt_catalogs(&mut startup, &mut profile)
        .unwrap();
    for (kind, parameter, value, offer) in [
        (SOURCE_KIND, "values", transition, source_offer),
        (SINK_KIND, "value", expected, sink_offer),
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
                        maximum: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
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
    offer.limits.max_queue_items = conduit_semantic_catalog::MAXIMUM_ATTEMPT_TRANSITIONS as u16;
    offer.limits.max_queue_bytes = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        * u32::from(offer.limits.max_queue_items);
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
