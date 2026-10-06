use conduit_core::{
    ConfigurationValue, ObservationKind, Quantity, QuantityUnit, TerminalDisposition,
    DURATION_INFO_ID,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, TimerAdapter};
use std::time::Duration;

const POSITIONAL: &str = include_str!("../../../plots/clock/main.conduit");
const NAMED: &str =
    "plot clock-demo {\n    clock: time/every(freq = 1s)\n    clock >> presentation/tick\n}\n";
const LOCAL: &str = "plot clock-demo {\n    freq = 1s\n    clock: time/every(freq)\n    clock >> presentation/tick\n}\n";

#[derive(Default)]
struct RecordingTimer {
    waits: Vec<Duration>,
    stop: Option<RunControl>,
    now_ms: u64,
}

impl TimerAdapter for RecordingTimer {
    fn wait(&mut self, duration: Duration) {
        self.waits.push(duration);
        self.now_ms += u64::try_from(duration.as_millis()).unwrap();
        if self.waits.len() >= 16 {
            if let Some(control) = self.stop.take() {
                control
                    .request_stop(RunControlRequestId::new("stop-canonical-clock").unwrap())
                    .unwrap();
            }
        }
    }

    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.now_ms)
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    (startup, profile)
}

fn checked(source: &str) -> conduit_plot::CheckedSyntaxDocument {
    let (startup, _) = catalogs();
    let syntax = parse_syntax_document(source);
    assert_eq!(syntax.round_trip(), source);
    check_syntax_document(&syntax, &startup).expect("canonical clock checks")
}

#[test]
fn duration_spellings_have_one_semantic_identity_and_execute_until_explicit_stop() {
    let positional = checked(POSITIONAL);
    let named = checked(NAMED);
    let local = checked(LOCAL);
    assert_eq!(
        positional.plots[0].checked_plot_id,
        named.plots[0].checked_plot_id
    );
    assert_eq!(
        named.plots[0].checked_plot_id,
        local.plots[0].checked_plot_id
    );
    assert_ne!(positional.source_document_id, named.source_document_id);

    let (_, profile) = catalogs();
    let positional = expand_canonical_plot(&positional, "clock-demo", &profile).unwrap();
    let named = expand_canonical_plot(&named, "clock-demo", &profile).unwrap();
    let local = expand_canonical_plot(&local, "clock-demo", &profile).unwrap();
    assert_eq!(positional.expanded_plot_id, named.expanded_plot_id);
    assert_eq!(named.expanded_plot_id, local.expanded_plot_id);
    assert_eq!(positional.gears.len(), 2);
    let every = positional
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "time/every")
        .unwrap();
    assert_eq!(
        every.checked_front().startup_parameters()[0]
            .value_type
            .as_str(),
        DURATION_INFO_ID
    );
    assert_eq!(
        every.configuration[0].value,
        ConfigurationValue::Quantity(Quantity::new(1, QuantityUnit::Second))
    );

    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&positional).unwrap();
    let mut output = Vec::with_capacity(4_096);
    let control = RunControl::default();
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(16),
        stop: Some(control.clone()),
        now_ms: 0,
    };
    let report = host
        .run_fragment_controlled_to(plan.fragments[0].clone(), &mut output, &mut timer, &control)
        .unwrap();
    assert_eq!(timer.waits, vec![Duration::from_secs(1); 16]);
    let output = String::from_utf8(output).unwrap();
    for sequence in 0..6 {
        assert!(
            output.contains(&format!("tick sequence={sequence}\n")),
            "{output}"
        );
    }
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
}

#[test]
fn duration_and_selected_wait_contract_fail_before_tick_presentation() {
    let (startup, profile) = catalogs();
    for invalid in ["1", "1m", "-1s", "18446744073709551615s"] {
        let source = format!(
            "plot bad {{\n    clock: time/every({invalid})\n    clock >> presentation/tick\n}}\n"
        );
        let refused = match check_syntax_document(&parse_syntax_document(&source), &startup) {
            Ok(checked) => expand_canonical_plot(&checked, "bad", &profile).is_err(),
            Err(_) => true,
        };
        assert!(refused, "{invalid} must not become an executable duration");
    }

    let checked = checked(POSITIONAL);
    let expanded = expand_canonical_plot(&checked, "clock-demo", &profile).unwrap();
    let mut host = StdHost::new();
    let mut plan = host.plan_expanded_local(&expanded).unwrap();
    let every = plan.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == "time/every")
        .unwrap();
    every.host_calls[0].contract_id = conduit_core::HostCallContractId::from("wrong/wait@1");
    let mut output = Vec::with_capacity(256);
    let mut timer = RecordingTimer::default();
    assert!(host
        .run_fragment_to(plan.fragments.remove(0), &mut output, &mut timer)
        .is_err());
    assert!(timer.waits.is_empty());
    assert!(!String::from_utf8_lossy(&output).contains("tick sequence="));
}
