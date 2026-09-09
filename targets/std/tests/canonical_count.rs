use conduit_core::{ObservationKind, PortTemporal, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, TimerAdapter};
use std::time::Duration;

const PROGRAM: &str = include_str!("../../../forms/count/main.conduit");
const EVIDENCE_MARKER: &str = "CONDUIT_FORM_EVIDENCE=";

#[derive(Default)]
struct RecordingTimer {
    waits: Vec<Duration>,
    stop: Option<RunControl>,
}

impl TimerAdapter for RecordingTimer {
    fn wait(&mut self, duration: Duration) {
        self.waits.push(duration);
        if self.waits.len() >= 24 {
            if let Some(control) = self.stop.take() {
                control
                    .request_stop(RunControlRequestId::new("stop-canonical-count").unwrap())
                    .unwrap();
            }
        }
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
fn canonical_program_reacts_to_open_flow_until_explicit_stop() {
    let (startup, profile) = catalogs();
    let syntax = parse_syntax_document(PROGRAM);
    assert_eq!(syntax.round_trip(), PROGRAM);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "count-demo", &profile).unwrap();
    assert_eq!(expanded.gears.len(), 3);

    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let plan_id = plan.plan_id.clone();
    let state = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::STATE_COUNT_KIND)
        .unwrap();
    assert_eq!(
        state.inputs[0].temporal,
        PortTemporal::Flow { closes: false }
    );
    assert_eq!(state.outputs[0].temporal, PortTemporal::Current);

    let mut output = Vec::with_capacity(4_096);
    let control = RunControl::default();
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(24),
        stop: Some(control.clone()),
    };
    let report = host
        .run_fragment_controlled_to(plan.fragments[0].clone(), &mut output, &mut timer, &control)
        .unwrap();
    assert_eq!(timer.waits, vec![Duration::from_secs(1); 24]);
    let output = String::from_utf8(output).unwrap();
    let counts = output
        .lines()
        .filter_map(|line| line.strip_prefix("count value="))
        .map(|value| value.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    assert!(counts.len() > 5, "counts={counts:?}");
    assert_eq!(counts.first(), Some(&2));
    assert!(counts.windows(2).all(|pair| pair[1] == pair[0] + 1));
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    println!(
        "{EVIDENCE_MARKER}{{\"plan_id\":\"{}\",\"play_id\":\"{}\"}}",
        plan_id.as_str(),
        kernel.active_play_id.as_str()
    );
}

#[test]
fn reactive_face_range_overflow_and_selected_identity_are_exact() {
    let (startup, profile) = catalogs();
    let single_value = PROGRAM.replace("Tick...", "Tick");
    let checked = check_syntax_document(&parse_syntax_document(&single_value), &startup).unwrap();
    let reactively_lifted = expand_canonical_form(&checked, "count-demo", &profile).unwrap();
    let canonical = check_syntax_document(&parse_syntax_document(PROGRAM), &startup).unwrap();
    let canonical = expand_canonical_form(&canonical, "count-demo", &profile).unwrap();
    assert_eq!(
        reactively_lifted.expanded_form_id,
        canonical.expanded_form_id
    );

    let overflow = PROGRAM.replace("count(2)", "count(18446744073709551616)");
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
