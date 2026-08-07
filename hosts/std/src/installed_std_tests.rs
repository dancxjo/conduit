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
