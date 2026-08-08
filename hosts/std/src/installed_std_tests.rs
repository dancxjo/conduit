use super::{installed_std, StdHost, StdHostConfig, TimerAdapter};
use conduit_core::{
    BootId, ConnectionProvider, HostId, ObservationKind, OfferGeneration, TerminalDisposition,
};
use conduit_form::parse;
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use std::collections::BTreeMap;
use std::time::Duration;

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
        "form 0\n\ntyped_tick {\n clock: time/tick\n observe: conduit.test/tick-observer\n clock.count = 3\n clock.period-ms = 7\n clock.tick -> observe.in\n}\n",
        &installed_std::test_catalog(),
    )
    .expect("typed tick fixture parses");
    let realm = [host.advertisement().clone()];
    let placements = default_placements(&form, &realm).expect("typed tick placements resolve");
    let provider_choices = BTreeMap::new();
    let plan = plan_with_options(
        &form,
        &realm,
        &placements,
        &[ConnectionProvider::Local],
        PlanningOptions {
            connection_providers: &provider_choices,
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            link_bindings: &[],
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
    assert_eq!(kernel.post_activation_allocations, 0);
}

#[test]
fn zero_count_tick_completes_without_wait_or_value_receipt() {
    let mut host = host("zero-tick-host");
    let form = parse(
        "form 0\n\nzero_tick {\n clock: time/tick\n observe: conduit.test/tick-observer\n clock.count = 0\n clock.period-ms = 99\n clock.tick -> observe.in\n}\n",
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
        "form 0\n\nmutated_tick {\n clock: time/tick\n observe: conduit.test/tick-observer\n clock.count = 1\n clock.period-ms = 7\n clock.tick -> observe.in\n}\n",
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
            "form 0\n\ntext_demo {{\n source: conduit.test/text-source\n show: presentation/text\n source.invalid = {invalid}\n source.text -> show.text\n}}\n"
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
    assert_eq!(kernel.post_activation_allocations, 0);
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
    let mutations: [fn(&mut conduit_core::PlannedOperation); 14] = [
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
fn canonical_text_pipeline_has_zero_successful_post_activation_allocations() {
    let source = r#"form hello {
    upper: text/upper
    show: presentation/text
    "Hello, world." > upper > show
}
"#;
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
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
    assert_eq!(report.kernel.unwrap().post_activation_allocations, 0);
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("HELLO, WORLD.\n"));
}

#[test]
fn canonical_greet_has_zero_successful_post_activation_allocations() {
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
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
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
    assert_eq!(report.kernel.unwrap().post_activation_allocations, 0);
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("WelcomeTravis\n"));
}

#[test]
fn canonical_clock_has_zero_successful_post_activation_allocations() {
    let source = "form clock-demo {\n    clock: time/every(1s)\n    clock > presentation/tick\n}\n";
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_time_pipeline_catalogs(&mut startup, &mut profile).unwrap();
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
    assert_eq!(report.kernel.unwrap().post_activation_allocations, 0);
    assert_eq!(timer.waits, vec![Duration::from_secs(1); 4]);
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("tick sequence=3\n"));
}

#[test]
fn canonical_state_count_executes_current_values_with_bounded_evidence() {
    let source = r#"form count (
    start: Count = 0
    bump: Tick...| > value: $Count
) {
    cell: state/count(start)
    bump > cell.bump
    cell.value > value
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
    conduit_std_catalog::install_time_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    conduit_std_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile).unwrap();
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
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::STATE_COUNT_KIND)
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
    assert_eq!(kernel.post_activation_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
