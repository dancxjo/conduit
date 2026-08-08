use super::{
    arguments::parse_arguments, render::draw_document, Arguments, PatchbayApplication, BACKGROUND,
};
use std::path::PathBuf;

#[test]
fn application_adapters_share_the_fresh_advertised_host_identity() {
    let application = PatchbayApplication::new(Arguments::default()).unwrap();
    let advertised = application.model.advertisement();
    let (control_host, control_boot) = application.control.host_identity();
    let (file_host, file_boot) = application.file_task.host_identity();

    assert_eq!(control_host, &advertised.host_id);
    assert_eq!(control_boot, &advertised.boot_id);
    assert_eq!(file_host, &advertised.host_id);
    assert_eq!(file_boot, &advertised.boot_id);
    assert_eq!(
        advertised.capabilities.iter().any(|offer| {
            offer.capability_id.as_str() == conduit_std_catalog::COPY_FILE_CAPABILITY
        }),
        application.file_task.provider_available()
    );
}

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
    assert!(
        parse_arguments(vec!["--distributed-play".into()].into_iter())
            .unwrap()
            .distributed_play
    );
    assert!(
        parse_arguments(vec!["--distributed-play-server".into()].into_iter())
            .unwrap()
            .distributed_play_server
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
    let lines = application.presentation_lines();
    let text = lines.join("\n");
    assert!(text.contains("NEW-PLAN RECOVERY"));
    assert!(text.contains("Plan B  id="));
    assert!(text.contains("SAME-PLAN FALLBACK"));
    assert!(text.contains("Plan C unchanged"));
    assert!(text.contains("LINEAR NARRATION"));
    assert!(text.contains("Plan identity did not change"));
    assert!(text.contains("UNPLANNED ROUTE refused=ambient Wi-Fi"));
    assert!(text.contains("PLAN-A replan-required"));
    assert!(text.contains("OUTCOME replan=true"));
    assert!(text.contains("PLAN-B predeclared-fallback"));
    assert!(text.contains("OUTCOME replan=false"));
    let visual = lines
        .iter()
        .position(|line| line == "ROUTE RECOVERY — same facts, two outcomes")
        .unwrap();
    let detail = lines
        .iter()
        .position(|line| line == "ROUTE DETAIL — exact identities and evidence")
        .unwrap();
    assert!(
        visual < detail,
        "compact hierarchy must precede exact detail"
    );
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
