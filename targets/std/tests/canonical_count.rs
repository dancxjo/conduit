use conduit_core::{ObservationKind, PortTemporal, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, TimerAdapter};
use std::time::Duration;

const PROGRAM: &str = include_str!("../../../forms/count/main.conduit");

#[derive(Default)]
struct RecordingTimer {
    waits: Vec<Duration>,
}

impl TimerAdapter for RecordingTimer {
    fn wait(&mut self, duration: Duration) {
        self.waits.push(duration);
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn canonical_program_four_runs_startup_flow_and_current_through_one_kernel() {
    let (startup, profile) = catalogs();
    let syntax = parse_syntax_document(PROGRAM);
    assert_eq!(syntax.round_trip(), PROGRAM);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "count-demo", &profile).unwrap();
    assert_eq!(expanded.gears.len(), 3);

    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let state = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::STATE_COUNT_KIND)
        .unwrap();
    assert_eq!(
        state.inputs[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    assert_eq!(state.outputs[0].temporal, PortTemporal::Current);

    let mut output = Vec::with_capacity(512);
    let mut timer = RecordingTimer::default();
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
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn temporal_mismatch_range_overflow_and_selected_identity_fail_before_output() {
    let (startup, profile) = catalogs();
    let open_flow = PROGRAM.replace("Tick...|", "Tick...");
    let checked = check_syntax_document(&parse_syntax_document(&open_flow), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "count-demo", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-045"
    );

    let overflow = PROGRAM.replace("count(2)", "count(18446744073709551615)");
    let checked = check_syntax_document(&parse_syntax_document(&overflow), &startup).unwrap();
    assert!(expand_canonical_form(&checked, "count-demo", &profile).is_err());

    let checked = check_syntax_document(&parse_syntax_document(PROGRAM), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "count-demo", &profile).unwrap();
    let mut host = StdHost::new();
    let mut plan = host.plan_expanded_local(&expanded).unwrap();
    let state = plan.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::STATE_COUNT_KIND)
        .unwrap();
    state.implementation_id = conduit_core::ImplementationId::from("wrong/state-count@1");
    let mut output = Vec::with_capacity(512);
    let mut timer = RecordingTimer::default();
    assert!(host
        .run_fragment_to(plan.fragments.remove(0), &mut output, &mut timer)
        .is_err());
    assert!(timer.waits.is_empty());
    assert!(!String::from_utf8_lossy(&output).contains("count value="));
}
