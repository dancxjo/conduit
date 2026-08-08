use super::*;
use std::path::PathBuf;

fn editor(name: &str, source: &str) -> FormEditor {
    FormEditor::from_source(PathBuf::from(format!("{name}.conduit")), source.into()).unwrap()
}

fn wait_for_terminal(control: &mut NativeControl) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if control.poll().unwrap() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("Play did not publish terminal state within five seconds");
}

#[test]
fn canonical_form_plans_runs_and_keeps_exact_plan_and_play_visible() {
    let editor = editor("hello", include_str!("../../../examples/hello.conduit"));
    let mut control = NativeControl::new();
    control.request_plan(&editor).unwrap();
    control.run(&editor).unwrap();
    wait_for_terminal(&mut control);
    let lines = control.lines().join("\n");
    assert!(lines.contains("PLAN request=patchbay/plan/0 plan="));
    assert!(lines.contains("CELL operation="));
    assert!(lines.contains("PLAY active="));
    assert!(lines.contains("terminal=Completed"));
    assert!(lines.contains("RUN request=patchbay/run/1 disposition=Accepted"));
}

#[test]
fn stale_plan_is_rejected_before_worker_activation() {
    let mut editor = editor("hello", include_str!("../../../examples/hello.conduit"));
    let mut control = NativeControl::new();
    control.request_plan(&editor).unwrap();
    editor
        .replace_source(include_str!("../../../examples/greet.conduit").into())
        .unwrap();
    editor.recheck().unwrap();
    assert!(control.run(&editor).unwrap_err().contains("StalePlan"));
    assert!(!control.is_running());
    assert!(control.lines().join("\n").contains(
        "RUN request=patchbay/run/1 disposition=Rejected reason=Run rejected: StalePlan"
    ));
}

#[test]
fn rejected_plan_keeps_its_exact_request_identity() {
    let editor = editor("invalid", "not a conduit form");
    let mut control = NativeControl::new();
    control.request_plan(&editor).unwrap_err();
    assert!(control
        .lines()
        .join("\n")
        .contains("PLAN-ACTION request=patchbay/plan/0 disposition=Rejected"));
}

#[test]
fn stop_request_reaches_ordinary_play_and_renders_cancelled_terminal() {
    let editor = editor("clock", include_str!("../../../examples/clock.conduit"));
    let mut control = NativeControl::new();
    control.request_plan(&editor).unwrap();
    control.run(&editor).unwrap();
    control.stop().unwrap();
    wait_for_terminal(&mut control);
    let lines = control.lines().join("\n");
    assert!(lines.contains("terminal=Cancelled"));
    assert!(lines.contains("CONTROL request=patchbay/stop/2 disposition=Accepted"));
    assert!(lines.contains("STOP request=patchbay/stop/2 disposition=Accepted"));
}

#[test]
fn host_failure_is_rendered_separately_from_rejection_and_cancellation() {
    let mut control = NativeControl::new();
    control.failure = Some("provider terminal failure".into());
    let lines = control.lines().join("\n");
    assert!(lines.contains("PLAY terminal=Failed error=provider terminal failure"));
    assert!(!lines.contains("Cancelled"));
    assert!(!lines.contains("disposition=Rejected"));
}
