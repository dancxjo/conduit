use super::{parse_arguments, render::draw_document, Arguments, BACKGROUND};
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
    assert!(parse_arguments(vec!["--unknown".into()].into_iter()).is_err());
    assert!(parse_arguments(vec!["--observatory-snapshot".into()].into_iter()).is_err());
    assert!(parse_arguments(vec!["--form".into()].into_iter()).is_err());
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
