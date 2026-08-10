use super::{
    arguments::parse_arguments, gui::GuiAction, presentation::portable_presentation_lines,
    render::draw_document, Arguments, PatchbayApplication, BACKGROUND,
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
    assert!(text.contains("RENDERER SIGN "));
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
    assert_eq!(
        parse_arguments(
            vec!["--linear-observatory-snapshot".into(), "report.json".into(),].into_iter(),
        )
        .unwrap()
        .linear_snapshot_path,
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

#[test]
fn native_build_mode_drives_explicit_birth_wake_plan_play_and_lull() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-build-birth-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../examples/hello.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    let build = application.presentation_lines().join("\n");
    assert!(build.contains("BUILD current=0 saved=0 checked=0 last-born=not-present"));
    assert!(build.contains("BODY not born — action: Birth Body"));
    assert!(build.contains("kind=text/upper"));
    assert!(build.contains("info=value/text@1"));

    application.birth_body().unwrap();
    let born_id = application.build_birth.body().unwrap().body_id.clone();
    let born = application.presentation_lines().join("\n");
    assert!(born.contains("BORN · LULLED — action: Wake Body"));
    assert!(!born.contains("WAKE "));
    application.wake_body().unwrap();
    application.plan_play().unwrap();
    application.play_plan().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while application.control.is_running() && std::time::Instant::now() < deadline {
        application.control.poll().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!application.control.is_running());
    application.lull_body().unwrap();
    let lulled = application.presentation_lines().join("\n");
    assert!(lulled.contains("BODY EVENT Born"));
    assert!(lulled.contains("WAKE EVENT Woke"));
    assert!(lulled.contains("WAKE EVENT PlanReady"));
    assert!(lulled.contains("WAKE EVENT PlayStarted"));
    assert!(lulled.contains("WAKE EVENT Lulled"));

    application.edit_source(|source| source.push('\n')).unwrap();
    assert_eq!(application.build_birth.body().unwrap().body_id, born_id);
    let revised = application.presentation_lines().join("\n");
    assert!(revised.contains("current=1 saved=0 checked=1 last-born=0"));

    application
        .edit_source(|source| {
            let closing = source.rfind('}').expect("example has a closing brace");
            source.remove(closing);
        })
        .unwrap();
    assert!(application.graphical_form.is_none());
    assert!(application
        .presentation_lines()
        .join("\n")
        .contains("DIAGNOSTIC"));

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn graphical_actions_open_a_checked_back_and_toggle_the_same_linear_projection() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-gui-actions-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("count.conduit");
    std::fs::write(&path, include_str!("../../../examples/count.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    assert!(application.graphical_form.is_none());
    application
        .handle_gui_action(GuiAction::OpenNextForm)
        .unwrap();
    assert_eq!(
        application.graphical_form.as_ref().unwrap().form_name,
        "count-demo"
    );
    application
        .handle_gui_action(GuiAction::ToggleLinearView)
        .unwrap();
    assert!(application.linear_view);

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn graphical_selection_rejects_stale_and_invented_hit_candidates() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-gui-selection-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../examples/hello.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    let graph = application.graphical_form.as_ref().unwrap();
    let identity = graph.subject_identities().nth(1).unwrap();
    let candidate = graph.subject_ref(identity).unwrap();

    let mut stale = candidate.clone();
    stale.expanded_form_id = conduit_core::ExpandedFormId::from("expanded/stale");
    assert!(application
        .handle_gui_action(GuiAction::SelectSubject(stale))
        .unwrap_err()
        .contains("stale expanded Form"));
    assert_eq!(application.graphical_selection, 0);

    let mut invented = candidate;
    invented.subject_identity = "renderer-invented/subject".into();
    assert!(application
        .handle_gui_action(GuiAction::SelectSubject(invented))
        .unwrap_err()
        .contains("not in the typed graph"));
    assert_eq!(application.graphical_selection, 0);

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
