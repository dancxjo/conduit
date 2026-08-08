use super::{
    arguments::parse_arguments, render::draw_document, Arguments, PatchbayApplication, BACKGROUND,
};
use std::path::PathBuf;

#[test]
fn arguments_are_explicit_and_fail_closed() {
    assert_eq!(
        parse_arguments(Vec::new().into_iter()).unwrap(),
        Arguments::default()
    );
    assert_eq!(
        parse_arguments(vec!["--form".into(), "greet.conduit".into()].into_iter())
            .unwrap()
            .form_path,
        Some(PathBuf::from("greet.conduit"))
    );
    assert!(
        parse_arguments(vec!["--smoke-exit-after-window".into()].into_iter())
            .unwrap()
            .exit_after_window
    );
    assert_eq!(
        parse_arguments(vec!["--observatory-snapshot".into(), "report.json".into()].into_iter())
            .unwrap()
            .snapshot_path,
        Some(PathBuf::from("report.json"))
    );
    assert!(
        parse_arguments(vec!["--control-demo".into()].into_iter())
            .unwrap()
            .control_demo
    );
    assert!(
        parse_arguments(vec!["--control-demo-stop".into()].into_iter())
            .unwrap()
            .control_demo_stop
    );
    assert!(
        parse_arguments(vec!["--native-copy-demo".into()].into_iter())
            .unwrap()
            .native_copy_demo
    );
    assert!(
        parse_arguments(vec!["--distributed-route-demo".into()].into_iter())
            .unwrap()
            .distributed_route_demo
    );
    assert!(parse_arguments(vec!["--unknown".into()].into_iter()).is_err());
    assert!(parse_arguments(vec!["--observatory-snapshot".into()].into_iter()).is_err());
    assert!(parse_arguments(vec!["--form".into()].into_iter()).is_err());
}

#[test]
fn native_document_exposes_both_route_recovery_cases() {
    let application = PatchbayApplication::new(Arguments {
        distributed_route_demo: true,
        ..Arguments::default()
    })
    .expect("distributed route document");
    let text = application.presentation_lines().join("\n");
    assert!(text.contains("PLAN-A replan-required"));
    assert!(text.contains("OUTCOME replan=true"));
    assert!(text.contains("PLAN-B predeclared-fallback"));
    assert!(text.contains("OUTCOME replan=false"));
}

#[test]
fn topology_document_draws_pixels_inside_the_bounded_surface() {
    let mut buffer = vec![BACKGROUND; 320 * 100];
    draw_document(
        &mut buffer,
        320,
        100,
        &["HOSTS 1".into(), "  host=exact boot=boot-1".into()],
    );
    assert!(buffer.iter().any(|pixel| *pixel != BACKGROUND));
}
