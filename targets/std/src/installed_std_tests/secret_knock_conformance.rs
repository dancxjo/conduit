use super::timing_form_catalogs::install_catalogs;
use super::{host, installed_std};
use crate::TimerAdapter;
use conduit_body::{Body, BodyFormPlan, BodyPlan, BodyPlayIdentity, BodyWorkset, ResidentForm};
use conduit_core::{
    BaseImplementationId, ConfigurationValue, ObservationKind, PortDirection, PortTemporal,
    ResourceClassId, ResourceOffer, ResourcePoolId, SignId, StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document,
    structured_selector_definition, ConfigurationField, ConfigurationRule, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use std::collections::BTreeMap;
use std::time::Duration;

const BUTTON_SOURCE: &str = "conduit-test/secret-knock-button";
const COMMAND_SOURCE: &str = "conduit-test/secret-knock-template-commands";

struct KnockTimer {
    micros: [u64; 4],
    next_micros: usize,
    now_ms: u64,
}

impl TimerAdapter for KnockTimer {
    fn wait(&mut self, duration: Duration) {
        self.now_ms = self
            .now_ms
            .saturating_add(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX));
    }

    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.now_ms)
    }

    fn monotonic_now_micros(&mut self) -> Option<u64> {
        let value = self.micros.get(self.next_micros).copied()?;
        self.next_micros += 1;
        Some(value)
    }

    fn wait_until_monotonic_ms(&mut self, deadline_ms: u64) -> bool {
        self.now_ms = deadline_ms;
        true
    }
}

#[test]
fn secret_knock_composes_input_timing_storage_comparison_and_result_in_one_play() {
    let transitions = [
        conduit_semantic_catalog::button_transition_value("button/primary", true, 1).unwrap(),
        conduit_semantic_catalog::button_transition_value("button/primary", false, 2).unwrap(),
        conduit_semantic_catalog::button_transition_value("button/primary", true, 3).unwrap(),
        conduit_semantic_catalog::button_transition_value("button/primary", true, 4).unwrap(),
    ];
    let template = conduit_semantic_catalog::normalized_value(&[333_333, 1_000_000]).unwrap();
    let commands = [
        conduit_semantic_catalog::put_template_command("front-desk", template.clone()).unwrap(),
        conduit_semantic_catalog::get_template_command("front-desk").unwrap(),
    ];
    let comparison = conduit_semantic_catalog::compare_normalized_patterns(
        &template,
        &template,
        conduit_semantic_catalog::MAXIMUM_ABSOLUTE_METRIC,
        1,
    )
    .unwrap();
    let button_offer = sequence_source_offer(&transitions[0], BUTTON_SOURCE, 4);
    let command_offer = sequence_source_offer(&commands[0], COMMAND_SOURCE, 2);
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_catalogs(&mut startup, &mut profile);
    install_fixture(
        &mut startup,
        &mut profile,
        BUTTON_SOURCE,
        "values",
        &transitions[0],
        &button_offer,
        8,
    );
    install_fixture(
        &mut startup,
        &mut profile,
        COMMAND_SOURCE,
        "values",
        &commands[0],
        &command_offer,
        4,
    );
    let transition_values = encoded_values(&transitions);
    let command_values = encoded_values(&commands);
    let source = format!(
        "{}\nform secret-knock-proof {{\n    buttons: {BUTTON_SOURCE}(values = \"{transition_values}\")\n    commands: {COMMAND_SOURCE}(values = \"{command_values}\")\n    knock: secret-knock\n    buttons.output > knock.transitions\n    commands.output > knock.template_commands\n}}\n",
        include_str!("../../../../forms/secret-knock/main.conduit"),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let selectors = checked
        .forms
        .iter()
        .flat_map(|form| &form.cords)
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
                PortTemporal::Flow { closes: true },
            ))
            .unwrap();
    }
    let expanded = expand_canonical_form(&checked, "secret-knock-proof", &profile).unwrap();
    for kind in [
        conduit_semantic_catalog::TIMED_BUTTON_ATTEMPT_KIND,
        conduit_semantic_catalog::ORDERED_EVENT_INTERVALS_KIND,
        conduit_semantic_catalog::NORMALIZE_SEQUENCE_KIND,
        conduit_semantic_catalog::TEMPLATE_STORAGE_KIND,
        conduit_semantic_catalog::FINAL_NORMALIZED_PATTERN_KIND,
        conduit_semantic_catalog::COMPARE_PATTERN_KIND,
    ] {
        assert!(expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == kind));
    }

    let mut advertisement = host("secret-knock-host").advertisement().clone();
    advertisement.capabilities.extend([
        button_offer,
        command_offer,
        conduit_std_offers::timed_button_attempt_std_offer(),
        conduit_std_offers::ordered_event_intervals_std_offer(),
        conduit_std_offers::normalize_sequence_std_offer(),
        conduit_std_offers::template_storage_std_offer(),
        conduit_std_offers::final_normalized_pattern_std_offer(),
        conduit_std_offers::compare_pattern_std_offer(),
        conduit_std_offers::structured_presentation_std_offer(
            conduit_semantic_catalog::PATTERN_COMPARISON_TYPE,
            &conduit_semantic_catalog::pattern_comparison_type(),
        ),
    ]);
    advertisement
        .capabilities
        .extend(selectors.iter().map(|selector| {
            conduit_std_offers::structured_selector_std_offer(
                selector,
                PortTemporal::Flow { closes: true },
            )
        }));
    for (pool, class) in [
        (
            "pool/secret-knock-clock",
            conduit_core::TIMER_RESOURCE_CLASS,
        ),
        (
            "pool/secret-knock-deadline",
            conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
        ),
        (
            "pool/secret-knock-templates",
            conduit_std_offers::TEMPLATE_STORAGE_RESOURCE_CLASS,
        ),
        (
            "pool/secret-knock-presentation",
            conduit_core::PRESENTATION_RESOURCE_CLASS,
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
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let secret_knock_form = ResidentForm::new(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
    );
    let unrelated = conduit_form::parse(
        "form unrelated_status {\n clock: time/tick(count = 2, period-ms = 5)\n observe: conduit-test/tick-observer\n clock.tick > observe.in\n}\n",
        &installed_std::test_catalog(),
    )
    .unwrap();
    let unrelated_placements = conduit_planner::default_placements(&unrelated, &hosts).unwrap();
    let unrelated_plan = conduit_planner::plan_with_options(
        &unrelated,
        &hosts,
        &unrelated_placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let unrelated_form = ResidentForm::new(
        unrelated_plan.source_document_id.clone(),
        unrelated_plan.checked_form_id.clone(),
    );
    let body = Body::born_with_forms(
        BodyWorkset::from_forms([secret_knock_form.clone(), unrelated_form.clone()]).unwrap(),
        1,
        SignId::from("sign/secret-knock-body-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/secret-knock-body-woke"))
        .unwrap()
        .1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![
            BodyFormPlan {
                form: secret_knock_form,
                plan: plan.clone(),
            },
            BodyFormPlan {
                form: unrelated_form,
                plan: unrelated_plan.clone(),
            },
        ],
    )
    .unwrap();
    let body_play = BodyPlayIdentity::bind(&body_plan, 1);
    super::secret_knock_body_admission::admit_combined_resources(
        &body,
        &body_plan,
        &advertisement,
        &plan,
        &unrelated_plan,
    );
    let playing = wake
        .body_plan_ready(&body_plan, SignId::from("sign/secret-knock-body-planned"))
        .unwrap()
        .body_play_started(
            &body_plan,
            &body_play,
            SignId::from("sign/secret-knock-body-playing"),
        )
        .unwrap();
    assert_eq!(body_plan.forms.len(), 2);
    assert_eq!(
        playing.plans[0].active_play_id.as_ref(),
        Some(&body_play.active_play_id)
    );
    let storage = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::TEMPLATE_STORAGE_KIND
        })
        .unwrap();
    assert_eq!(storage.resources.len(), 1);

    let mut output = Vec::with_capacity(4_096);
    let mut timer = KnockTimer {
        micros: [100, 120, 250, 700],
        next_micros: 0,
        now_ms: 0,
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
        body_play.play_sequence,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("Secret Knock executes as one production-kernel Play");
    let unrelated_report = installed_std::run_fragment(
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
        &unrelated_plan.fragments[0],
        body_play.play_sequence,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("unrelated Form executes under the same Body-wide Play");
    assert_eq!(
        unrelated_report.kernel.unwrap().post_play_start_allocations,
        0
    );
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert!(kernel.identity.lengths().0 >= 8);
    assert_eq!(kernel.presentation_ids.len(), 1);
    let presented = report
        .observations
        .iter()
        .find_map(|observation| match &observation.kind {
            ObservationKind::ValuePresented { value } => Some(value),
            _ => None,
        })
        .expect("comparison has one correlated manifestation");
    assert_eq!(presented.encoded, comparison.canonical_bytes().unwrap());
}

#[allow(clippy::too_many_arguments)]
fn install_fixture(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    kind: &str,
    parameter: &str,
    value: &StructuredInfoValue,
    offer: &conduit_core::CapabilityOffer,
    multiplier: u32,
) {
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
                    maximum: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                        * multiplier as usize) as u32,
                },
            }],
        })
        .unwrap();
}

fn sequence_source_offer(
    value: &StructuredInfoValue,
    kind: &str,
    maximum: u16,
) -> conduit_core::CapabilityOffer {
    let mut offer = fixture_offer(value, PortDirection::Output, kind);
    offer.startup_parameters[0].name = "values".into();
    offer.host_operations = vec![conduit_core::wait_host_operation_requirement()];
    offer.resource_requirements = vec![conduit_core::resource_requirement(
        conduit_core::TIMER_RESOURCE_CLASS,
        1,
    )];
    offer.limits.max_queue_items = maximum;
    offer.limits.max_queue_bytes =
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * u32::from(maximum);
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

fn encoded_values<const N: usize>(values: &[StructuredInfoValue; N]) -> String {
    values
        .iter()
        .map(|value| hex(&value.canonical_bytes().unwrap()))
        .collect::<Vec<_>>()
        .join(",")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
