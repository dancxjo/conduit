use super::*;
use crate::portable_keyboard;
use patchbay_model::PatchbayModel;
use std::path::PathBuf;

fn editor(name: &str, source: &str) -> FormEditor {
    FormEditor::from_source(PathBuf::from(format!("{name}.conduit")), source.into()).unwrap()
}

fn wait_for_terminal(control: &mut NativeControl) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        control.poll().unwrap();
        if !control.is_running() {
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
    assert!(lines.contains("GEAR operation="));
    assert!(lines.contains("PLAY active="));
    assert!(lines.contains("terminal=Completed"));
    assert!(lines.contains("RUN request=patchbay/run/1 disposition=Accepted"));
}

#[test]
fn stale_plan_is_rejected_before_worker_play_start() {
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
    control.failure = Some("base terminal failure".into());
    let lines = control.lines().join("\n");
    assert!(lines.contains("PLAY terminal=Failed error=base terminal failure"));
    assert!(!lines.contains("Cancelled"));
    assert!(!lines.contains("disposition=Rejected"));
}

#[test]
fn ordinary_native_control_consumes_the_advertised_keyboard_through_the_kernel() {
    let composition = StdHostComposition::minimal().with_text().with_input();
    let model = PatchbayModel::with_identity_composition_and(
        HostId::from("patchbay-native/text-lab"),
        BootId::from("patchbay-native/text-lab-boot"),
        composition,
        portable_keyboard::append_offer,
    )
    .unwrap();
    let mut keyboard = portable_keyboard::NativeKeyboardInput::new();
    let mut control =
        NativeControl::for_advertisement(model.advertisement().clone(), keyboard.reader()).unwrap();
    let editor = editor(
        "conduitos-keyboard-upper",
        conduitos::keyboard_text_plan::FORM_SOURCE,
    );

    control.request_plan(&editor).unwrap();
    control.run(&editor).unwrap();
    keyboard
        .observe(
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyH),
            winit::event::ElementState::Pressed,
            false,
        )
        .unwrap();
    keyboard
        .observe(
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyH),
            winit::event::ElementState::Released,
            false,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while control.presented_text().as_deref() != Some("H") && std::time::Instant::now() < deadline {
        control.poll().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(control.presented_text().as_deref(), Some("H"));
    assert!(control.is_running());
    control.stop().unwrap();
    wait_for_terminal(&mut control);

    let output = std::str::from_utf8(control.presentation().unwrap()).unwrap();
    assert!(output.lines().any(|line| line == "H"), "{output}");
    assert!(control.lines().join("\n").contains("terminal=Cancelled"));
}

#[test]
fn presented_text_receipts_are_utf8_exact_and_fail_closed_when_malformed() {
    let mut control = NativeControl::new();
    control.presentation = Some(
        b"PRESENTATION-TEXT bytes=3 hex=48c3a9\nH\xc3\xa9\nPRESENTATION-TEXT bytes=1 hex=21\n!\n"
            .to_vec(),
    );
    assert_eq!(control.presented_text().as_deref(), Some("H\u{e9}!"));

    control.presentation = Some(b"PRESENTATION-TEXT bytes=2 hex=ff\n".to_vec());
    assert_eq!(control.presented_text(), None);
}
