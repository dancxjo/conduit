use conduit_core::{ObservationKind, PlanCompletionPolicy, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, ThreadTimer};
use std::time::Duration;

fn plan(source: &str) -> (StdHost, conduit_core::PlanFragment) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "specimen", &profile).unwrap();
    let host = StdHost::new();
    let fragment = host
        .plan_expanded_local(&expanded)
        .unwrap()
        .fragments
        .remove(0);
    (host, fragment)
}

#[test]
fn live_drained_play_stays_attached_until_explicit_cancellation() {
    let source = "form specimen {\n value: text/literal(\"still here\")\n show: presentation/text\n value > show\n}\n";
    let (mut host, fragment) = plan(source);
    assert_eq!(fragment.completion_policy, PlanCompletionPolicy::Live);
    let plan_id = fragment.plan_id.clone();
    let control = RunControl::default();
    let runner_control = control.clone();
    let handle = std::thread::spawn(move || {
        let mut output = Vec::new();
        let report = host
            .run_fragment_attached_controlled_to(
                fragment,
                &mut output,
                &mut ThreadTimer,
                &runner_control,
            )
            .unwrap();
        (report, output)
    });

    assert!(control.wait_until_quiescent(Duration::from_secs(2)));
    assert!(!handle.is_finished(), "quiescence must not finish the Play");
    control
        .request_stop(RunControlRequestId::new("test/operator-stop").unwrap())
        .unwrap();
    let (report, output) = handle.join().unwrap();
    assert!(String::from_utf8(output).unwrap().contains("still here"));
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.identity.plan_id, plan_id);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn explicit_completion_policy_finishes_the_attached_product_runner() {
    let source = "form specimen {\n complete\n value: text/literal(\"done\")\n show: presentation/text\n value > show\n}\n";
    let (mut host, fragment) = plan(source);
    assert_eq!(
        fragment.completion_policy,
        PlanCompletionPolicy::SemanticCompletion
    );
    let report = host
        .run_fragment_attached_controlled_to(
            fragment,
            &mut Vec::new(),
            &mut ThreadTimer,
            &RunControl::default(),
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
}
