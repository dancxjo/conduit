use super::{installed_std, StdHost, StdHostConfig, TimerAdapter};
use conduit_core::{
    BaseImplementationId, BootId, HostId, ObservationKind, OfferGeneration, TerminalDisposition,
};
use conduit_form::parse;
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use std::collections::BTreeMap;
use std::time::Duration;

mod audio_playback_conformance;
mod bool_presentation_conformance;
mod calendar_proposal_conformance;
mod calendar_provider_conformance;
mod gate_conformance;
mod graphics_conformance;
mod input_semantics_conformance;
mod instrument_conformance;
mod json_conformance;
mod layout_conformance;
mod logic_conformance;
mod math_conformance;
mod midi_input_conformance;
mod midi_output_conformance;
mod pattern_comparison_conformance;
mod presentation_composition;
mod recurrence_conformance;
mod rhythm_compare_conformance;
mod robotics_conformance;
mod sequence_normalization_conformance;
mod sound_replanning;
mod structured_selector_conformance;
mod structured_values_conformance;
mod template_pattern_selection_conformance;
mod template_storage_conformance;
mod timed_pattern_conformance;
mod timing_conformance;
mod vector_search_conformance;

struct RecordingTimer {
    waits: Vec<Duration>,
}

impl TimerAdapter for RecordingTimer {
    fn wait(&mut self, duration: Duration) {
        self.waits.push(duration);
    }
}

fn host(id: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(id),
        boot_id: BootId::from(format!("{id}-boot")),
        offer_generation: OfferGeneration(1),
    })
}

#[test]
fn typed_tick_plans_and_executes_through_the_installed_kernel_table() {
    let mut host = host("typed-tick-host");
    let form = parse(
        "form typed_tick {\n clock: time/tick(count = 3, period-ms = 7)\n observe: conduit-test/tick-observer\n clock.tick > observe.in\n}\n",
        &installed_std::test_catalog(),
    )
    .expect("typed tick fixture parses");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("typed tick placements resolve");
    let base_choices = BTreeMap::new();
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &base_choices,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("typed tick plans with capacity-one pressure");
    let tick = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == installed_std::contract::TICK_KIND)
        .expect("tick placement exists");
    assert_eq!(
        tick.kind_contract_revision.as_str(),
        installed_std::contract::TICK_CONTRACT_REVISION
    );
    assert_eq!(
        tick.outputs[0].value_kind.as_str(),
        installed_std::contract::TICK_VALUE_KIND
    );
    assert_eq!(plan.fragments[0].connections[0].item_capacity, 1);
    assert_eq!(plan.fragments[0].connections[0].byte_capacity, 8);

    let mut output = Vec::with_capacity(2_048);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(3),
    };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect("typed tick executes through the production installed path");
    assert_eq!(timer.waits, vec![Duration::from_millis(7); 3]);
    let output = String::from_utf8(output).expect("tick report is utf8");
    assert!(output.contains("receipt tick sequence=0"));
    assert!(output.contains("receipt tick sequence=1"));
    assert!(output.contains("receipt tick sequence=2"));
    assert!(matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(kernel.identity.lengths(), (6, 0, 1));
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn typed_latest_and_tee_plan_and_execute_with_capacity_one_pressure() {
    let mut host = host("typed-flow-state-host");
    let form = parse(
        "form typed_flow_state {\n source: conduit-test/scalar-source\n latest: state/latest\n split: flow/tee\n left: conduit-test/scalar-sink\n right: conduit-test/scalar-sink\n source.value > latest.in\n latest.out > split.in\n split.left > left.in\n split.right > right.in\n}\n",
        &installed_std::test_catalog(),
    )
    .expect("typed flow/state form parses");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("every typed placement resolves");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("typed latest/tee form plans with capacity-one cords");
    let fragment = &plan.fragments[0];
    let latest = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::LATEST_KIND)
        .expect("latest placement exists");
    let tee = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::TEE_KIND)
        .expect("tee placement exists");
    assert_eq!(
        latest.kind_contract_revision.as_str(),
        conduit_semantic_catalog::STATE_LATEST_SCALAR_CONTRACT_REVISION
    );
    assert_eq!(
        tee.kind_contract_revision.as_str(),
        conduit_semantic_catalog::FLOW_TEE_SCALAR_CONTRACT_REVISION
    );
    assert!(latest
        .inputs
        .iter()
        .chain(latest.outputs.iter())
        .chain(tee.inputs.iter())
        .chain(tee.outputs.iter())
        .all(|port| port.value_kind.as_str() == conduit_core::SCALAR_INFO_ID));
    assert!(fragment.connections.iter().all(|cord| {
        cord.item_capacity == 1 && cord.byte_capacity == conduit_core::SCALAR_ENCODED_LEN as u32
    }));

    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(3),
    };
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("typed latest/tee execute through the production kernel");
    let output = String::from_utf8(output).expect("flow/state report is utf8");
    assert!(output.contains("/latest kind=state/latest"));
    assert!(output.contains("/split kind=flow/tee"));
    assert!(output.contains(" complete\n"));
    assert_eq!(timer.waits, vec![Duration::ZERO; 3]);
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn mutated_typed_tee_identity_fails_before_play() {
    let baseline_host = host("mutated-flow-state-host");
    let form = parse(
        "form typed_flow_state {\n source: conduit-test/scalar-source\n latest: state/latest\n split: flow/tee\n left: conduit-test/scalar-sink\n right: conduit-test/scalar-sink\n source.value > latest.in\n latest.out > split.in\n split.left > left.in\n split.right > right.in\n}\n",
        &installed_std::test_catalog(),
    )
    .expect("typed flow/state form parses");
    let hosts = [baseline_host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("typed placements resolve");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("typed flow/state plan exists");
    let mut fragment = plan.fragments[0].clone();
    fragment
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::TEE_KIND)
        .expect("tee placement exists")
        .artifact_id = conduit_core::ArtifactId::from("mutated/tee-artifact");

    let mut host = baseline_host;
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    assert!(host
        .run_fragment_to(fragment, &mut output, &mut timer)
        .is_err());
    assert!(timer.waits.is_empty());
}

#[test]
fn zero_count_tick_completes_without_wait_or_value_receipt() {
    let mut host = host("zero-tick-host");
    let form = parse(
        "form zero_tick {\n clock: time/tick(count = 0, period-ms = 99)\n observe: conduit-test/tick-observer\n clock.tick > observe.in\n}\n",
        &installed_std::test_catalog(),
    )
    .expect("zero tick fixture parses");
    let plan = host.plan_local(&form, None).expect("zero tick plans");
    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect("zero tick completes through installed kernel path");
    assert!(timer.waits.is_empty());
    assert!(!String::from_utf8(output)
        .expect("tick report is utf8")
        .contains("receipt tick sequence="));
    assert_eq!(
        report
            .kernel
            .expect("kernel report exists")
            .identity
            .lengths(),
        (0, 0, 1)
    );
}

#[test]
fn mutated_tick_executable_identity_fails_before_any_wait() {
    let mut host = host("mutated-tick-host");
    let form = parse(
        "form mutated_tick {\n clock: time/tick(count = 1, period-ms = 7)\n observe: conduit-test/tick-observer\n clock.tick > observe.in\n}\n",
        &installed_std::test_catalog(),
    )
    .expect("typed tick fixture parses");
    let plan = host.plan_local(&form, None).expect("typed tick plans");
    let mut fragment = plan.fragments[0].clone();
    let tick = fragment
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == installed_std::contract::TICK_KIND)
        .expect("tick placement exists");
    tick.kind_contract_revision = conduit_core::KindContractRevision::from("wrong/tick@1");
    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let error = host
        .run_fragment_to(fragment, &mut output, &mut timer)
        .expect_err("mutated executable identity must fail closed");
    assert!(
        error.to_ascii_lowercase().contains("fragment") || error.contains("reservation"),
        "unexpected fail-closed reason: {error}"
    );
    assert!(timer.waits.is_empty());
}

fn text_plan(host: &StdHost, invalid: bool) -> conduit_core::Plan {
    let form = parse(
        &format!(
            "form text_demo {{\n source: conduit-test/text-source(invalid = {invalid})\n show: presentation/text\n source.text > show.text\n}}\n"
        ),
        &installed_std::test_catalog(),
    )
    .expect("typed text fixture parses");
    host.plan_local(&form, None)
        .expect("typed text presentation plans")
}

#[test]
fn typed_text_plans_presents_and_completes_through_the_installed_kernel() {
    let mut host = host("typed-text-host");
    let plan = text_plan(&host, false);
    let presentation = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == installed_std::contract::TEXT_PRESENTATION_KIND
        })
        .expect("text presentation placement exists");
    assert_eq!(
        presentation.kind_contract_revision.as_str(),
        installed_std::contract::TEXT_PRESENTATION_CONTRACT_REVISION
    );
    assert_eq!(
        presentation.implementation_id.as_str(),
        installed_std::contract::TEXT_PRESENTATION_IMPLEMENTATION
    );
    assert_eq!(
        presentation.inputs[0].value_kind.as_str(),
        installed_std::contract::TEXT_PRESENTATION_VALUE_KIND
    );

    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect("typed text executes through the production installed path");
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Hello\n"));
    assert!(timer.waits.is_empty());
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn invalid_utf8_fails_before_a_successful_text_presentation() {
    let mut host = host("invalid-text-host");
    let plan = text_plan(&host, true);
    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let error = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect_err("invalid UTF-8 must fail before presentation succeeds");
    assert!(
        error.contains("not valid UTF-8"),
        "unexpected error: {error}"
    );
    assert!(!String::from_utf8_lossy(&output).contains("Hello\n"));
    assert!(timer.waits.is_empty());
}

#[test]
fn planned_generate_text_uses_the_lowered_kernel_and_exact_fixture_base() {
    let mut catalog = installed_std::test_catalog();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_ai::install_generate_text_catalog(&mut startup, &mut catalog)
        .expect("generate-text catalog installs");
    let form = parse(
        "form generate_demo {\n source: conduit-test/text-source(invalid = false)\n generate: ai/generate-text\n show: presentation/text\n source.text > generate.prompt\n generate.text > show.text\n}\n",
        &catalog,
    )
    .expect("generate-text execution form parses");

    let mut advertisement = host("generate-text-host").advertisement().clone();
    let fixture = conduit_ai::generate_text_base_fixtures()
        .into_iter()
        .nth(1)
        .expect("large local fixture exists");
    advertisement
        .capabilities
        .push(fixture.advertisement.capabilities[0].clone());
    advertisement
        .resources
        .extend(fixture.advertisement.resources);
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts).expect("all operations are realizable");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("generate-text form plans through the ordinary planner");
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_ai::GENERATE_TEXT_KIND)
        .expect("generate-text placement exists");
    assert_eq!(
        placement.implementation_id.as_str(),
        conduit_ai::LARGE_LOCAL_IMPLEMENTATION
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
            calendar: None,
        },
        &plan.fragments[0],
        0,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("planned generate-text runs through lowering, kernel, and base");
    assert!(String::from_utf8(output)
        .expect("fixture output is utf8")
        .contains("fixture/large-local: Hello\n"));
    assert_eq!(
        report.kernel.expect("kernel report").identity.lengths(),
        (2, 0, 1)
    );
    assert!(timer.waits.is_empty());

    let mut substituted = plan.fragments[0].clone();
    let placement = substituted
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_ai::GENERATE_TEXT_KIND)
        .expect("generate-text placement exists");
    placement.implementation_id =
        conduit_core::ImplementationId::from(conduit_ai::REMOTE_FRONTIER_IMPLEMENTATION);
    let mut output = Vec::new();
    let error = installed_std::run_fragment(
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
        &substituted,
        1,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect_err("an implementation absent from the Plan cannot substitute at runtime");
    assert!(
        error.contains("InvalidFragment"),
        "unexpected rejection: {error}"
    );
}

#[test]
fn every_text_presentation_executable_identity_mutation_fails_before_output() {
    let baseline_host = host("mutated-text-host");
    let plan = text_plan(&baseline_host, false);
    let baseline = plan.fragments[0].clone();
    let presentation_index = baseline
        .placements
        .iter()
        .position(|placement| {
            placement.kind_id.as_str() == installed_std::contract::TEXT_PRESENTATION_KIND
        })
        .expect("text presentation placement exists");
    let mutations: [fn(&mut conduit_core::PlannedGear); 14] = [
        |placement| placement.kind_id = conduit_core::KindId::from("wrong/text"),
        |placement| {
            placement.kind_contract_revision =
                conduit_core::KindContractRevision::from("wrong/text@1")
        },
        |placement| {
            placement.execution_profile_id =
                conduit_core::ExecutionProfileId::from("wrong/profile@1")
        },
        |placement| placement.capability_id = conduit_core::CapabilityId::from("wrong-capability"),
        |placement| {
            placement.implementation_id = conduit_core::ImplementationId::from("wrong/impl@1")
        },
        |placement| placement.artifact_id = conduit_core::ArtifactId::from("wrong/artifact@1"),
        |placement| placement.inputs[0].value_kind = conduit_core::KindId::from("wrong/value@1"),
        |placement| placement.inputs[0].port_id = conduit_core::PortId::from("wrong-port"),
        |placement| placement.host_id = conduit_core::HostId::from("wrong-host"),
        |placement| placement.boot_id = conduit_core::BootId::from("wrong-boot"),
        |placement| placement.offer_generation = conduit_core::OfferGeneration(99),
        |placement| placement.configuration[0].value = conduit_core::ConfigurationValue::U64(5),
        |placement| {
            placement.host_operations[0].target_kind =
                Some(conduit_core::KindId::from("wrong/presentation"))
        },
        |placement| {
            placement.resources[0].pool_id = conduit_core::ResourcePoolId::from("wrong-pool")
        },
    ];
    for mutate in mutations {
        let mut fragment = baseline.clone();
        mutate(&mut fragment.placements[presentation_index]);
        let mut host = host("mutated-text-host");
        let mut output = Vec::with_capacity(1_024);
        let mut timer = RecordingTimer { waits: Vec::new() };
        host.run_fragment_to(fragment, &mut output, &mut timer)
            .expect_err("mutated executable identity must fail closed");
        assert!(!String::from_utf8_lossy(&output).contains("Hello\n"));
        assert!(timer.waits.is_empty());
    }
}

#[test]
fn canonical_text_pipeline_has_zero_successful_post_play_start_allocations() {
    let source = r#"form hello {
    upper: text/upper
    show: presentation/text
    "Hello, world." > upper > show
}
"#;
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = conduit_form::parse_syntax_document(source);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "hello", &profile).unwrap();
    let mut host = host("allocation-text-host");
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let mut output = Vec::with_capacity(4_096);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .unwrap();
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("HELLO, WORLD.\n"));
}

#[test]
fn canonical_greet_has_zero_successful_post_play_start_allocations() {
    let source = r#"form greet (
    greeting: Text = "Hello"
    name: Text > text: Text
) {
    join: text/join(greeting)
    name > join > text
}
form welcome {
    hello: greet("Welcome")
    "Travis" > hello > presentation/text
}
"#;
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = conduit_form::parse_syntax_document(source);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "welcome", &profile).unwrap();
    let mut host = host("allocation-greet-host");
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let mut output = Vec::with_capacity(4_096);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .unwrap();
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("WelcomeTravis\n"));
}

#[test]
fn canonical_clock_has_zero_successful_post_play_start_allocations() {
    let source = "form clock-demo {\n    clock: time/every(1s)\n    clock > presentation/tick\n}\n";
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    let syntax = conduit_form::parse_syntax_document(source);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "clock-demo", &profile).unwrap();
    let mut host = host("allocation-clock-host");
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let mut output = Vec::with_capacity(4_096);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(4),
    };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .unwrap();
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
    assert_eq!(timer.waits, vec![Duration::from_secs(1); 4]);
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("tick sequence=3\n"));
}

#[test]
fn canonical_state_count_executes_current_values_with_bounded_sign() {
    let source = r#"form count (
    start: Count = 0
    bump: Tick...| > value: $Count
) {
    gear: state/count(start)
    bump > gear.bump
    gear.value > value
}
form count-demo {
    clock: time/every(1s)
    count: count(2)
    show: presentation/count
    clock > count > show
}
"#;
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = conduit_form::parse_syntax_document(source);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "count-demo", &profile).unwrap();
    let mut host = host("allocation-count-host");
    let plan = host.plan_expanded_local(&expanded).unwrap();
    assert_eq!(plan.fragments[0].placements.len(), 3);
    assert_eq!(plan.fragments[0].connections.len(), 2);
    let state = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::STATE_COUNT_KIND)
        .unwrap();
    assert_eq!(
        state.inputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        state.outputs[0].temporal,
        conduit_core::PortTemporal::Current
    );

    let mut output = Vec::with_capacity(4_096);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(4),
    };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .unwrap();
    assert_eq!(timer.waits, vec![Duration::from_secs(1); 4]);
    let output = String::from_utf8(output).unwrap();
    let counts = output
        .lines()
        .filter_map(|line| line.strip_prefix("count value="))
        .map(|value| value.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(counts, vec![2, 3, 4, 5, 6]);
    assert!(matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
