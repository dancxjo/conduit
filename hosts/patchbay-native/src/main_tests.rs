use super::{
    arguments::parse_arguments, presentation::portable_presentation_lines, render::draw_document,
    Arguments, PatchbayApplication, BACKGROUND,
};

#[test]
fn native_renderer_consumes_the_same_portable_value_as_html_transport() {
    let presentation = patchbay_model::portable_demonstration().unwrap();
    let lines = portable_presentation_lines(&presentation).unwrap();
    let rendered = lines.join("\n");
    assert!(rendered.contains(presentation.identity.as_str()));
    assert!(rendered.contains(presentation.basis.body_id.as_str()));
    assert!(rendered.contains(presentation.basis.wake_id.as_str()));
    assert!(rendered.contains("base=USB CDC"));
    assert!(rendered.contains("base=WebSocket"));
}

#[test]
fn native_renderer_inspects_its_exact_realization_without_local_state() {
    let application = PatchbayApplication::new(Arguments {
        distributed_route_demo: true,
        ..Arguments::default()
    })
    .unwrap();
    let text = application.presentation_lines().join("\n");
    assert!(text.contains("RENDERER FACE presentation/renderer inputs=1 outputs=1"));
    assert!(text.contains("RENDERER PLACEMENT "));
    assert!(text.contains("implementation=presentation/renderer-wayland@1"));
    assert!(text.contains("artifact=patchbay-native/wayland@1"));
    assert!(text.contains("RENDERER PORT presentation Input info=presentation/presentation@1"));
    assert!(text.contains("RENDERER PORT manifestation Output info=presentation/manifestation@1"));
    assert!(text.contains("RENDERER RESOURCE pool="));
    assert!(text.contains(
        "RENDERER BASE contract=conduit.host/present@1 target=presentation/base/wayland-surface@1"
    ));
    assert!(text.contains("RENDERER LIMITS active=1 queue-items=1"));
    assert!(text.contains("RENDERER CLUE "));
}
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
        application.file_task.base_available()
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
    assert!(text.contains("PRESENTATION "));
    assert!(text.contains("SEED "));
    assert!(text.contains("The Play became unsatisfied"));
    assert!(text.contains("Replacement Plan"));
    assert!(text.contains("Plan identity did not change"));
    assert!(text.contains("ambient route"));
    assert!(text.contains("base=USB CDC"));
    assert!(text.contains("base=WebSocket"));
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
