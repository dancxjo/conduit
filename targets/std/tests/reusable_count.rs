use conduit_core::{ObservationKind, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, TimerAdapter};
use std::time::Duration;

const COUNT_SOURCE: &str = include_str!("../../../forms/count/main.conduit");
const EVIDENCE_MARKER: &str = "CONDUIT_FORM_EVIDENCE=";

#[derive(Default)]
struct RecordingTimer {
    waits: Vec<Duration>,
    stop: Option<RunControl>,
}

impl TimerAdapter for RecordingTimer {
    fn wait(&mut self, duration: Duration) {
        self.waits.push(duration);
        if self.waits.len() >= 20 {
            if let Some(control) = self.stop.take() {
                control
                    .request_stop(RunControlRequestId::new("stop-reusable-count").unwrap())
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
fn reusable_count_runs_through_two_nested_levels_in_one_kernel_play() {
    let (startup, profile) = catalogs();
    let canonical = check_syntax_document(&parse_syntax_document(COUNT_SOURCE), &startup).unwrap();
    let canonical_count_id = canonical
        .forms
        .iter()
        .find(|form| form.name == "count")
        .unwrap()
        .checked_form_id
        .clone();
    let proof_source = format!(
        "{COUNT_SOURCE}\nform nested-count (\n    bump: Tick...| > value: $Count\n) {{\n    counter: count(7)\n    bump > counter.bump\n    counter.value > value\n}}\n\nform count-nesting-driver {{\n    clock: time/every(1s)\n    nested: nested-count\n    show: presentation/count\n    clock > nested > show\n}}\n"
    );
    let checked = check_syntax_document(&parse_syntax_document(&proof_source), &startup).unwrap();
    assert_eq!(
        checked
            .forms
            .iter()
            .find(|form| form.name == "count")
            .unwrap()
            .checked_form_id,
        canonical_count_id
    );

    let expanded = expand_canonical_form(&checked, "count-nesting-driver", &profile).unwrap();
    let count = expanded
        .provenance
        .iter()
        .find(|row| row.source_form == "count" && row.source_gear == "gear")
        .unwrap();
    assert_eq!(
        count.form_path,
        ["count-nesting-driver", "nested", "counter"]
    );

    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    assert_eq!(plan.fragments.len(), 1);
    let plan_id = plan.plan_id.clone();
    let mut output = Vec::with_capacity(512);
    let control = RunControl::default();
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(20),
        stop: Some(control.clone()),
    };
    let report = host
        .run_fragment_controlled_to(plan.fragments[0].clone(), &mut output, &mut timer, &control)
        .unwrap();
    assert_eq!(timer.waits, vec![Duration::from_secs(1); 20]);
    let output = String::from_utf8(output).unwrap();
    let counts = output
        .lines()
        .filter_map(|line| line.strip_prefix("count value="))
        .map(|value| value.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    assert!(counts.len() > 5, "counts={counts:?}");
    assert_eq!(counts.first(), Some(&7));
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
