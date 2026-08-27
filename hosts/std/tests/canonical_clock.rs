use conduit_core::{ObservationKind, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, TimerAdapter};
use std::time::Duration;

const POSITIONAL: &str = include_str!("../../../examples/clock.conduit");
const NAMED: &str =
    "form clock-demo {\n    clock: time/every(freq = 1s)\n    clock > presentation/tick\n}\n";
const LOCAL: &str = "form clock-demo {\n    freq = 1s\n    clock: time/every(freq)\n    clock > presentation/tick\n}\n";

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
    conduit_std_catalog::install_tick_presentation_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn checked(source: &str) -> conduit_form::CheckedSyntaxDocument {
    let (startup, _) = catalogs();
    let syntax = parse_syntax_document(source);
    assert_eq!(syntax.round_trip(), source);
    check_syntax_document(&syntax, &startup).expect("canonical clock checks")
}

#[test]
fn duration_spellings_have_one_semantic_identity_and_execute_four_bounded_ticks() {
    let positional = checked(POSITIONAL);
    let named = checked(NAMED);
    let local = checked(LOCAL);
    assert_eq!(
        positional.forms[0].checked_form_id,
        named.forms[0].checked_form_id
    );
    assert_eq!(
        named.forms[0].checked_form_id,
        local.forms[0].checked_form_id
    );
    assert_ne!(positional.source_document_id, named.source_document_id);

    let (_, profile) = catalogs();
    let positional = expand_canonical_form(&positional, "clock-demo", &profile).unwrap();
    let named = expand_canonical_form(&named, "clock-demo", &profile).unwrap();
    let local = expand_canonical_form(&local, "clock-demo", &profile).unwrap();
    assert_eq!(positional.expanded_form_id, named.expanded_form_id);
    assert_eq!(named.expanded_form_id, local.expanded_form_id);
    assert_eq!(positional.gears.len(), 2);

    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&positional).unwrap();
    let mut output = Vec::with_capacity(256);
    let mut timer = RecordingTimer::default();
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .unwrap();
    assert_eq!(timer.waits, vec![Duration::from_secs(1); 4]);
    let output = String::from_utf8(output).unwrap();
    for sequence in 0..4 {
        assert!(
            output.contains(&format!("tick sequence={sequence}\n")),
            "{output}"
        );
    }
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
fn duration_and_selected_wait_contract_fail_before_tick_presentation() {
    let (startup, profile) = catalogs();
    for invalid in ["1", "1m", "-1s", "18446744073709551615s"] {
        let source = format!(
            "form bad {{\n    clock: time/every({invalid})\n    clock > presentation/tick\n}}\n"
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        assert!(expand_canonical_form(&checked, "bad", &profile).is_err());
    }

    let checked = checked(POSITIONAL);
    let expanded = expand_canonical_form(&checked, "clock-demo", &profile).unwrap();
    let mut host = StdHost::new();
    let mut plan = host.plan_expanded_local(&expanded).unwrap();
    let every = plan.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == "time/every")
        .unwrap();
    every.host_operations[0].contract_id =
        conduit_core::HostOperationContractId::from("wrong/wait@1");
    let mut output = Vec::with_capacity(256);
    let mut timer = RecordingTimer::default();
    assert!(host
        .run_fragment_to(plan.fragments.remove(0), &mut output, &mut timer)
        .is_err());
    assert!(timer.waits.is_empty());
    assert!(!String::from_utf8_lossy(&output).contains("tick sequence="));
}
