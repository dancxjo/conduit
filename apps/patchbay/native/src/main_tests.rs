use super::{
    arguments::parse_arguments, gui::GuiAction, presentation::portable_presentation_lines,
    render::draw_document, Arguments, PatchbayApplication, BACKGROUND,
};

fn assert_semantic_history_round_trip(
    application: &mut PatchbayApplication,
    before: &str,
    after: &str,
) {
    application
        .handle_gui_action(GuiAction::UndoSemanticEdit)
        .unwrap();
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().source,
        before
    );
    application
        .handle_gui_action(GuiAction::RedoSemanticEdit)
        .unwrap();
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().source,
        after
    );
}

#[test]
fn native_renderer_consumes_the_same_portable_value_as_html_transport() {
    let presentation = patchbay_model::portable_demonstration_with_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )
    .unwrap();
    let lines = portable_presentation_lines(&presentation).unwrap();
    let nonvisual = conduit_presentation::render_linear_presentation(&presentation).unwrap();
    assert_eq!(lines, nonvisual.lines);
    let rendered = lines.join("\n");
    assert!(rendered.contains(presentation.identity.as_str()));
    assert!(rendered.contains(presentation.basis.body_id.as_ref().unwrap().as_str()));
    assert!(rendered.contains(presentation.basis.wake_id.as_ref().unwrap().as_str()));
    assert!(rendered.contains("base=conduit.base/usb-cdc-acm@1"));
    assert!(rendered.contains("base=conduit.base/websocket-rfc6455@1"));
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
            offer.capability_id.as_str() == conduit_std_offers::COPY_FILE_CAPABILITY
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
        parse_arguments(vec!["--front-door".into()].into_iter())
            .unwrap()
            .front_door
    );
    assert!(
        parse_arguments(vec!["--help".into()].into_iter())
            .unwrap()
            .help
    );
    assert!(
        parse_arguments(vec!["--smoke-exit-after-window".into()].into_iter())
            .unwrap()
            .exit_after_window
    );
    assert!(
        parse_arguments(vec!["--prewake".into(), "--prewake-hold".into()].into_iter())
            .unwrap()
            .prewake_hold
    );
    assert_eq!(
        parse_arguments(vec!["--prewake-hold".into()].into_iter()),
        Err("--prewake-hold requires --prewake".into())
    );
    let external =
        parse_arguments(vec!["--body-biography".into(), "roseau.json".into()].into_iter()).unwrap();
    assert_eq!(
        external.body_biography_path,
        Some(PathBuf::from("roseau.json"))
    );
    assert!(external.hosted_patchbay_plan_id.is_none());
    let hosted = parse_arguments(
        vec![
            "--body-biography".into(),
            "roseau.json".into(),
            "--hosted-patchbay-plan".into(),
            "plan/17".into(),
            "--hosted-patchbay-implementation".into(),
            "patchbay/native@1".into(),
        ]
        .into_iter(),
    )
    .unwrap();
    assert_eq!(hosted.hosted_patchbay_plan_id.as_deref(), Some("plan/17"));
    assert_eq!(
        hosted.hosted_patchbay_implementation_id.as_deref(),
        Some("patchbay/native@1")
    );
    assert_eq!(
        parse_arguments(
            vec![
                "--body-biography".into(),
                "roseau.json".into(),
                "--hosted-patchbay-plan".into(),
                "plan/17".into(),
            ]
            .into_iter(),
        ),
        Err("hosted Patchbay Plan and implementation identities must be supplied together".into())
    );
    assert_eq!(
        parse_arguments(
            vec![
                "--hosted-patchbay-plan".into(),
                "plan/17".into(),
                "--hosted-patchbay-implementation".into(),
                "patchbay/native@1".into(),
            ]
            .into_iter(),
        ),
        Err("hosted Patchbay placement requires --body-biography".into())
    );
    assert_eq!(
        parse_arguments(
            vec![
                "--browser-page-url".into(),
                "http://127.0.0.1:8080/".into(),
                "--browser-chat-url".into(),
                "ws://127.0.0.1:9000/chat".into(),
            ]
            .into_iter()
        )
        .unwrap()
        .browser_chat_url
        .as_deref(),
        Some("ws://127.0.0.1:9000/chat")
    );
    assert_eq!(
        parse_arguments(
            vec!["--browser-page-url".into(), "http://127.0.0.1:8080/".into(),].into_iter()
        ),
        Err("browser page and chat URLs must be configured together".into())
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
    assert!(parse_arguments(vec!["--help".into(), "--help".into()].into_iter()).is_err());
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
    assert!(text.contains("base=conduit.base/usb-cdc-acm@1"));
    assert!(text.contains("base=conduit.base/websocket-rfc6455@1"));
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
    std::fs::write(&path, include_str!("../../../../examples/hello.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    let build = application.presentation_lines().join("\n");
    assert!(build.contains("SEED current=0 saved=0 checked=0 last-born=not-present"));
    assert!(build.contains("BODY not born — action: BIRTH"));
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
fn contextual_lifecycle_flow_exposes_only_exact_actions_and_refuses_invalid_accelerators() {
    use patchbay_model::PatchbayAction;

    let directory =
        std::env::temp_dir().join(format!("patchbay-lifecycle-flow-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("clock.conduit");
    std::fs::write(&path, include_str!("../../../../examples/clock.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    let assert_actions =
        |application: &PatchbayApplication, code: &str, expected: &[PatchbayAction]| {
            let flow = application.lifecycle_flow();
            assert_eq!(flow.state_code, code);
            assert_eq!(
                flow.actions
                    .iter()
                    .map(|candidate| candidate.action)
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(flow.actions.len() <= crate::lifecycle_flow::MAX_LIFECYCLE_ACTIONS);
        };

    assert_actions(&application, "FORM_CHECKED", &[PatchbayAction::Birth]);
    application
        .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Birth))
        .unwrap();
    assert_actions(&application, "BODY_LULLED", &[PatchbayAction::Wake]);

    // The invalid accelerator still executes one typed interaction and is a refusal, not exit.
    application
        .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Birth))
        .unwrap();
    let refusal = application.interaction_status.current().unwrap();
    assert_eq!(
        refusal.code,
        crate::interaction_status::InteractionStatusCode::Refused
    );
    assert!(refusal.text.contains("FORM_CHECKED") || refusal.text.contains("BODY_LULLED"));

    application
        .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Wake))
        .unwrap();
    assert_actions(
        &application,
        "WAKE_AWAITING_PLAN",
        &[PatchbayAction::Plan, PatchbayAction::Lull],
    );
    application
        .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Plan))
        .unwrap();
    assert_actions(
        &application,
        "PLAN_READY",
        &[PatchbayAction::Play, PatchbayAction::Lull],
    );
    application
        .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Play))
        .unwrap();
    assert_actions(&application, "PLAY_ACTIVE", &[PatchbayAction::Stop]);
    application
        .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Stop))
        .unwrap();
    application
        .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Stop))
        .unwrap();
    assert_eq!(
        application.interaction_status.current().unwrap().code,
        crate::interaction_status::InteractionStatusCode::Refused
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while application.control.is_running() && std::time::Instant::now() < deadline {
        application.control.poll().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_actions(&application, "PLAY_CANCELLED", &[PatchbayAction::Lull]);

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn contextual_lifecycle_flow_keeps_unsatisfied_and_failed_states_machine_readable() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-lifecycle-states-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../../examples/hello.conduit")).unwrap();
    let application = || {
        PatchbayApplication::new(Arguments {
            form_path: Some(path.clone()),
            ..Arguments::default()
        })
        .unwrap()
    };

    let mut unsatisfied = application();
    unsatisfied.birth_body().unwrap();
    unsatisfied.wake_body().unwrap();
    unsatisfied.plan_play().unwrap();
    unsatisfied.play_plan().unwrap();
    unsatisfied.mark_unsatisfied().unwrap();
    let active_unsatisfied = unsatisfied.lifecycle_flow();
    assert_eq!(active_unsatisfied.state_code, "PLAY_UNSATISFIED");
    assert_eq!(
        active_unsatisfied.actions[0].action,
        patchbay_model::PatchbayAction::Stop
    );
    assert!(active_unsatisfied.exact_basis.contains("plan="));
    unsatisfied.control.stop().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while unsatisfied.control.is_running() && std::time::Instant::now() < deadline {
        unsatisfied.control.poll().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let terminal_unsatisfied = unsatisfied.lifecycle_flow();
    assert_eq!(terminal_unsatisfied.state_code, "PLAY_UNSATISFIED");
    assert_eq!(
        terminal_unsatisfied
            .actions
            .iter()
            .map(|candidate| candidate.action)
            .collect::<Vec<_>>(),
        [
            patchbay_model::PatchbayAction::Plan,
            patchbay_model::PatchbayAction::Lull
        ]
    );

    let mut failed = application();
    failed.birth_body().unwrap();
    failed.wake_body().unwrap();
    let failed_sign = failed.lifecycle_sign("failed-test");
    let retained_sign = failed.lifecycle_sign("failed-retained-test");
    failed
        .build_birth
        .fail_wake(failed_sign, retained_sign)
        .unwrap();
    let failed_flow = failed.lifecycle_flow();
    assert_eq!(failed_flow.state_code, "WAKE_FAILED");
    assert_eq!(
        failed_flow.actions[0].action,
        patchbay_model::PatchbayAction::Wake
    );

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn graphical_actions_open_a_checked_back_and_toggle_the_same_linear_projection() {
    use winit::keyboard::{Key, NamedKey};

    let directory =
        std::env::temp_dir().join(format!("patchbay-gui-actions-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("greet.conduit");
    std::fs::write(&path, include_str!("../../../../examples/greet.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    assert_eq!(application.handle_gui_action(GuiAction::OpenBack), Ok(()));
    assert_eq!(
        application.interaction_status.current().unwrap().text,
        "Interaction refused: NavigationTargetMissing"
    );
    assert!(application.back_navigation.is_empty());
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().open_form,
        "default-welcome"
    );

    let composed = application
        .graphical_form
        .as_ref()
        .unwrap()
        .compositions
        .iter()
        .find(|gear| gear.back_name == "greet")
        .unwrap();
    let subject = application
        .graphical_form
        .as_ref()
        .unwrap()
        .subject_ref(&composed.identity)
        .unwrap();
    application
        .handle_gui_action(GuiAction::SelectSubject(subject))
        .unwrap();
    application.back_navigation = vec![
        super::forms_navigation::BackNavigationEntry {
            parent_form: "bounded-parent".into(),
            gear_name: "bounded-gear".into(),
            child_form: "bounded-child".into(),
        };
        super::forms_navigation::MAX_BACK_NAVIGATION_DEPTH
    ];
    assert_eq!(application.handle_gui_action(GuiAction::OpenBack), Ok(()));
    assert_eq!(
        application.interaction_status.current().unwrap().text,
        "Interaction refused: NavigationDepthExceeded"
    );
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().open_form,
        "default-welcome"
    );
    application.back_navigation.clear();
    assert!(application
        .handle_form_key(&Key::Named(NamedKey::Enter))
        .unwrap());
    assert_eq!(
        application.graphical_form.as_ref().unwrap().form_name,
        "greet"
    );
    assert_eq!(
        application
            .graphical_form
            .as_ref()
            .unwrap()
            .face_inputs
            .len(),
        1
    );
    assert_eq!(
        application
            .graphical_form
            .as_ref()
            .unwrap()
            .face_outputs
            .len(),
        1
    );
    assert_eq!(application.back_navigation[0].gear_name, "hello");
    assert_eq!(
        application.back_breadcrumb(),
        "default-welcome > hello : greet"
    );
    application.handle_gui_action(GuiAction::OpenBack).unwrap();
    assert_eq!(
        application.graphical_form.as_ref().unwrap().form_name,
        "default-welcome"
    );
    assert!(application.back_navigation.is_empty());
    assert_eq!(application.back_breadcrumb(), "default-welcome");
    let primitive = application
        .graphical_form
        .as_ref()
        .unwrap()
        .gears
        .iter()
        .find(|gear| gear.source_form == "default-welcome")
        .unwrap();
    let primitive = application
        .graphical_form
        .as_ref()
        .unwrap()
        .subject_ref(&primitive.identity)
        .unwrap();
    application
        .handle_gui_action(GuiAction::SelectSubject(primitive))
        .unwrap();
    assert_eq!(application.handle_gui_action(GuiAction::OpenBack), Ok(()));
    assert_eq!(
        application.interaction_status.current().unwrap().text,
        "Interaction refused: NavigationTargetUnavailable"
    );
    assert_eq!(
        application
            .interaction
            .as_ref()
            .unwrap()
            .history()
            .last()
            .unwrap()
            .disposition,
        patchbay_model::InteractionDisposition::Refused(
            patchbay_model::PatchbayRefusal::NavigationTargetUnavailable
        )
    );
    application
        .handle_gui_action(GuiAction::ToggleLinearView)
        .unwrap();
    assert!(application.linear_view);
    let actions = application
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .filter_map(|receipt| match &receipt.request {
            patchbay_model::PatchbayInteractionRequest::Invoke { invocation, .. } => {
                Some(invocation.action)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actions,
        [
            patchbay_model::PatchbayAction::OpenBack,
            patchbay_model::PatchbayAction::OpenBack,
            patchbay_model::PatchbayAction::OpenBack,
            patchbay_model::PatchbayAction::OpenBack,
            patchbay_model::PatchbayAction::OpenBack,
            patchbay_model::PatchbayAction::ToggleLinearView
        ]
    );

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn palette_placement_runs_an_ordinary_interaction_before_editing_canonical_source() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-palette-place-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("making.conduit");
    std::fs::write(&path, "form making {\n}\n").unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    application
        .handle_gui_action(GuiAction::PlacePaletteKind {
            kind: "text/upper".into(),
            target: (204, 80),
        })
        .unwrap();
    application.palette_drag = Some("text/upper".into());
    application.cursor_position = (553.0, 100.0);
    application.handle_canvas_release().unwrap();
    application.handle_gui_action(GuiAction::SaveForm).unwrap();

    let view = application.form_editor.as_ref().unwrap().view();
    assert!(view.source.contains("upper: text/upper"));
    assert!(view.source.contains("upper-2: text/upper"));
    assert_eq!(application.graphical_form.as_ref().unwrap().gears.len(), 2);
    let second = application
        .graphical_form
        .as_ref()
        .unwrap()
        .gears
        .iter()
        .find(|gear| application.layout.position(&gear.identity) == Some((458, 80)))
        .unwrap();
    assert_eq!(
        application.layout.position(&second.identity),
        Some((458, 80))
    );
    let receipts = application
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 3);
    assert_eq!(
        receipts
            .iter()
            .filter(|receipt| matches!(
                &receipt.request,
                patchbay_model::PatchbayInteractionRequest::Edit {
                    edit: patchbay_model::PatchbayEdit::PlaceGear { .. },
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(receipts.iter().all(|receipt| {
        receipt.disposition == patchbay_model::InteractionDisposition::Succeeded
    }));
    drop(application);
    let reopened = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    assert_eq!(reopened.graphical_form.as_ref().unwrap().gears.len(), 2);

    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(directory.join("making.conduit.patchbay.json")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn slash_query_enter_places_uppercase_at_the_explicit_deterministic_target() {
    let directory = std::env::temp_dir().join(format!(
        "patchbay-palette-keyboard-place-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("making.conduit");
    std::fs::write(&path, "form making {\n}\n").unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    assert!(application.handle_palette_key(&winit::keyboard::Key::Character("/".into())));
    assert!(application.handle_palette_key(&winit::keyboard::Key::Character("uppercase".into())));
    assert!(application.handle_palette_key(&winit::keyboard::Key::Named(
        winit::keyboard::NamedKey::Enter
    )));

    let graph = application.graphical_form.as_ref().unwrap();
    let gear = graph
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "text/upper")
        .unwrap();
    assert_eq!(application.layout.position(&gear.identity), Some((204, 80)));
    let receipt = application
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .last()
        .unwrap();
    assert!(matches!(
        &receipt.request,
        patchbay_model::PatchbayInteractionRequest::Edit {
            edit: patchbay_model::PatchbayEdit::PlaceGear { kind_id, .. },
            ..
        } if kind_id == "text/upper"
    ));
    let placed = application.form_editor.as_ref().unwrap().view().source;
    assert_semantic_history_round_trip(&mut application, "form making {\n}\n", &placed);

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn stale_palette_drop_is_refused_before_canonical_source_changes() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-stale-palette-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("making.conduit");
    std::fs::write(&path, "form making {\n}\n").unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    let view = application.form_editor.as_ref().unwrap().view();
    let basis = patchbay_model::PatchbayEditBasis::new(
        view.checked.source_document_id.unwrap(),
        view.revision.saturating_add(1),
        application
            .graphical_form
            .as_ref()
            .unwrap()
            .expanded_form_id
            .clone(),
    )
    .unwrap();
    let outcome = application.apply_authoring_edit(&patchbay_model::PatchbayEdit::PlaceGear {
        basis,
        kind_id: "text/upper".into(),
    });
    assert_eq!(
        outcome,
        patchbay_model::PatchbayInvocationOutcome::Refused(
            patchbay_model::PatchbayRefusal::StalePresentation
        )
    );
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().source,
        "form making {\n}\n"
    );
    assert_eq!(
        application
            .semantic_history
            .as_ref()
            .unwrap()
            .transaction_count(),
        0
    );

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn semantic_history_refuses_after_birth_without_mutating_source_or_lifecycle() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-history-lifecycle-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("making.conduit");
    std::fs::write(
        &path,
        "form making {\n    literal: text/literal(\"hello\")\n}\n",
    )
    .unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    let graph = application.graphical_form.as_ref().unwrap();
    let literal = graph.subject_ref("gear/making/literal").unwrap();
    application
        .handle_gui_action(GuiAction::DuplicateGear(literal))
        .unwrap();
    let edited = application.form_editor.as_ref().unwrap().view().source;
    application.birth_body().unwrap();
    let body = application.build_birth.body().unwrap().body_id.clone();

    application
        .handle_gui_action(GuiAction::UndoSemanticEdit)
        .unwrap();

    assert_eq!(
        application.form_editor.as_ref().unwrap().view().source,
        edited
    );
    assert_eq!(application.build_birth.body().unwrap().body_id, body);
    assert!(application
        .interaction_status
        .current()
        .unwrap()
        .text
        .contains("cannot rewind lifecycle or external state"));

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn typed_cord_duplicate_and_remove_use_ordinary_interactions_and_persist() {
    let directory = std::env::temp_dir().join(format!("patchbay-compose-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("making.conduit");
    std::fs::write(
        &path,
        "form making {\n    literal: text/literal(\"hello\")\n    upper: text/upper\n    show: presentation/text\n    count: state/count(0)\n}\n",
    )
    .unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    let graph = application.graphical_form.as_ref().unwrap();
    let source = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "making/literal")
        .unwrap()
        .outputs[0]
        .identity
        .clone();
    let sink = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "making/upper")
        .unwrap()
        .inputs[0]
        .identity
        .clone();
    let source = graph.subject_ref(&source).unwrap();
    let sink = graph.subject_ref(&sink).unwrap();
    let count_output = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "making/count")
        .unwrap()
        .outputs[0]
        .identity
        .clone();
    let incompatible_source = graph.subject_ref(&count_output).unwrap();
    let before_refusal = application.form_editor.as_ref().unwrap().view().source;
    application
        .handle_gui_action(GuiAction::ConnectPorts {
            source: incompatible_source,
            sink: sink.clone(),
        })
        .unwrap();
    assert!(application
        .interaction_status
        .current()
        .unwrap()
        .text
        .contains("incompatible exact Port"));
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().source,
        before_refusal
    );
    application
        .handle_gui_action(GuiAction::ConnectPorts { source, sink })
        .unwrap();
    let connected = application.form_editor.as_ref().unwrap().view().source;
    assert_semantic_history_round_trip(&mut application, &before_refusal, &connected);

    let graph = application.graphical_form.as_ref().unwrap();
    let cord = graph.subject_ref(&graph.cords[0].identity).unwrap();
    let show_input = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "making/show")
        .and_then(|gear| gear.inputs.first())
        .and_then(|port| graph.subject_ref(&port.identity).ok())
        .unwrap();
    application
        .handle_gui_action(GuiAction::RerouteCord {
            cord,
            endpoint: show_input,
        })
        .unwrap();
    let rerouted = application.form_editor.as_ref().unwrap().view().source;
    assert_semantic_history_round_trip(&mut application, &connected, &rerouted);
    application
        .handle_gui_action(GuiAction::UndoSemanticEdit)
        .unwrap();
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().source,
        connected
    );

    let graph = application.graphical_form.as_ref().unwrap();
    let duplicate_source = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "making/literal")
        .and_then(|gear| gear.outputs.first())
        .and_then(|port| graph.subject_ref(&port.identity).ok())
        .unwrap();
    let duplicate_sink = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "making/upper")
        .and_then(|gear| gear.inputs.first())
        .and_then(|port| graph.subject_ref(&port.identity).ok())
        .unwrap();
    application
        .handle_gui_action(GuiAction::ConnectPorts {
            source: duplicate_source,
            sink: duplicate_sink,
        })
        .unwrap();
    assert!(application
        .interaction_status
        .current()
        .unwrap()
        .text
        .contains("already have a Cord"));

    let graph = application.graphical_form.as_ref().unwrap();
    let cord = graph.subject_ref(&graph.cords[0].identity).unwrap();
    application
        .handle_gui_action(GuiAction::SelectSubject(cord))
        .unwrap();
    assert!(application
        .handle_form_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Delete,
        ))
        .unwrap());
    assert!(application
        .graphical_form
        .as_ref()
        .unwrap()
        .cords
        .is_empty());
    let removed_cord = application.form_editor.as_ref().unwrap().view().source;
    assert_semantic_history_round_trip(&mut application, &connected, &removed_cord);

    let graph = application.graphical_form.as_ref().unwrap();
    let source = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "making/literal")
        .and_then(|gear| gear.outputs.first())
        .and_then(|port| graph.subject_ref(&port.identity).ok())
        .unwrap();
    let sink = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "making/upper")
        .and_then(|gear| gear.inputs.first())
        .and_then(|port| graph.subject_ref(&port.identity).ok())
        .unwrap();
    application
        .handle_gui_action(GuiAction::ConnectPorts { source, sink })
        .unwrap();

    let graph = application.graphical_form.as_ref().unwrap();
    let literal = graph.subject_ref("gear/making/literal").unwrap();
    application
        .handle_gui_action(GuiAction::DuplicateGear(literal))
        .unwrap();
    let duplicated = application.form_editor.as_ref().unwrap().view().source;
    let before_duplicate = application
        .semantic_history
        .as_ref()
        .unwrap()
        .prepare(
            crate::semantic_history::SemanticHistoryDirection::Undo,
            &application.semantic_checkpoint().unwrap(),
        )
        .unwrap()
        .source;
    assert_semantic_history_round_trip(&mut application, &before_duplicate, &duplicated);
    let graph = application.graphical_form.as_ref().unwrap();
    let upper = graph.subject_ref("gear/making/upper").unwrap();
    application
        .handle_gui_action(GuiAction::RemoveGear(upper))
        .unwrap();
    let removed_gear = application.form_editor.as_ref().unwrap().view().source;
    assert_semantic_history_round_trip(&mut application, &duplicated, &removed_gear);
    let graph = application.graphical_form.as_ref().unwrap();
    let literal = graph.subject_ref("gear/making/literal").unwrap();
    application
        .handle_gui_action(GuiAction::ConfigureGear {
            subject: literal,
            key: "value".into(),
            value: conduit_core::ConfigurationValue::Text("Howdy".into()),
        })
        .unwrap();
    let configured = application.form_editor.as_ref().unwrap().view().source;
    assert_semantic_history_round_trip(&mut application, &removed_gear, &configured);
    let graph = application.graphical_form.as_ref().unwrap();
    let literal = graph.subject_ref("gear/making/literal").unwrap();
    let semantic_ids = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
    );
    application
        .layout
        .move_gear(graph, &literal, 410, 180)
        .unwrap();
    application
        .layout
        .group_gear(graph, &literal, Some("sources".into()))
        .unwrap();
    assert_eq!(
        semantic_ids,
        (
            graph.source_document_id.clone(),
            graph.checked_form_id.clone(),
            graph.expanded_form_id.clone(),
        )
    );
    application.handle_gui_action(GuiAction::SaveForm).unwrap();
    let saved = application.form_editor.as_ref().unwrap().view();
    assert_eq!(saved.saved_revision, saved.revision);
    application
        .handle_gui_action(GuiAction::UndoSemanticEdit)
        .unwrap();
    let dirty = application.form_editor.as_ref().unwrap().view();
    assert_ne!(dirty.saved_revision, dirty.revision);
    application
        .handle_gui_action(GuiAction::RedoSemanticEdit)
        .unwrap();
    let restored_saved = application.form_editor.as_ref().unwrap().view();
    assert_eq!(restored_saved.saved_revision, restored_saved.revision);

    let view = application.form_editor.as_ref().unwrap().view();
    assert!(view.source.contains("literal-2: text/literal(\"hello\")"));
    assert!(view.source.contains("literal: text/literal(\"Howdy\")"));
    assert!(!view.source.contains("upper:"));
    assert!(!view.source.contains("literal.text > upper.text"));
    let actions = application
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .filter_map(|receipt| match &receipt.request {
            patchbay_model::PatchbayInteractionRequest::Edit { edit, .. } => Some(edit.operation()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(actions.contains(&"connect-ports"));
    assert!(actions.contains(&"remove-cord"));
    assert!(actions.contains(&"duplicate-gear"));
    assert!(actions.contains(&"remove-gear"));
    drop(application);

    let reopened = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    assert_eq!(reopened.graphical_form.as_ref().unwrap().gears.len(), 4);
    assert!(reopened.graphical_form.as_ref().unwrap().cords.is_empty());
    assert_eq!(
        reopened.layout.position("gear/making/literal"),
        Some((410, 180))
    );
    assert_eq!(
        reopened
            .layout
            .gears
            .iter()
            .find(|placement| placement.gear_identity == "gear/making/literal")
            .and_then(|placement| placement.group.as_deref()),
        Some("sources")
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(directory.join("making.conduit.patchbay.json")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn pointer_and_keyboard_selection_share_the_typed_interaction_path() {
    use winit::keyboard::{Key, NamedKey};

    let directory =
        std::env::temp_dir().join(format!("patchbay-input-selection-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../../examples/hello.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    let graph = application.graphical_form.as_ref().unwrap();
    let pointer = graph
        .subject_ref(graph.subject_identities().nth(2).unwrap())
        .unwrap();
    application
        .handle_gui_action(GuiAction::SelectSubject(pointer.clone()))
        .unwrap();
    assert_eq!(
        application.selected_graphical_identity(),
        Some(pointer.subject_identity.as_str())
    );

    application
        .handle_form_key(&Key::Named(NamedKey::ArrowRight))
        .unwrap();
    let history = application
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .collect::<Vec<_>>();
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|receipt| matches!(
        receipt.request,
        patchbay_model::PatchbayInteractionRequest::Select { .. }
    )));
    assert!(history.iter().all(|receipt| {
        receipt.disposition == patchbay_model::InteractionDisposition::Succeeded
            && !receipt.signs.is_empty()
    }));

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn focus_cancellation_clears_every_transient_gesture_deterministically() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-gesture-cancel-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../../examples/hello.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    let graph = application.graphical_form.as_ref().unwrap();
    let gear = graph.subject_ref(&graph.gears[0].identity).unwrap();
    let source = graph
        .subject_ref(&graph.gears[0].outputs[0].identity)
        .unwrap();
    let cord = graph.subject_ref(&graph.cords[0].identity).unwrap();
    application.environment_drag = Some(("part-1".into(), (1.0, 2.0)));
    application.palette_drag = Some("text/upper".into());
    application.gear_drag = Some((gear, (1.0, 2.0)));
    application.cord_drag = Some(source);
    application.cord_route_drag = Some(cord);
    application.cancel_transient_gestures("test cancellation");
    assert!(application.environment_drag.is_none());
    assert!(application.palette_drag.is_none());
    assert!(application.gear_drag.is_none());
    assert!(application.cord_drag.is_none());
    assert!(application.cord_route_drag.is_none());
    let status = application.interaction_status.current().unwrap();
    assert_eq!(
        status.code,
        super::interaction_status::InteractionStatusCode::Cancelled
    );
    assert!(status.text.contains("test cancellation"));

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn graphical_selection_rejects_stale_and_invented_hit_candidates() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-gui-selection-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../../examples/hello.conduit")).unwrap();
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
    application
        .handle_gui_action(GuiAction::SelectSubject(stale))
        .unwrap();
    assert!(application
        .interaction_status
        .current()
        .unwrap()
        .text
        .contains("StalePresentation"));
    assert!(application.selected_graphical_identity().is_none());

    let mut invented = candidate;
    invented.subject_identity = "renderer-invented/subject".into();
    application
        .handle_gui_action(GuiAction::SelectSubject(invented))
        .unwrap();
    assert!(application
        .interaction_status
        .current()
        .unwrap()
        .text
        .contains("UnknownSubject"));
    assert!(application.selected_graphical_identity().is_none());

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn invalid_request_construction_restores_interaction_state() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-invalid-request-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../../examples/hello.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();

    let graph = application.graphical_form.as_ref().unwrap();
    let valid = graph
        .subject_ref(graph.subject_identities().next().unwrap())
        .unwrap();
    let mut oversized = valid.clone();
    oversized.subject_identity = "x".repeat(patchbay_model::MAX_INTERACTION_ID_BYTES + 1);
    assert!(application
        .handle_gui_action(GuiAction::SelectSubject(oversized))
        .unwrap_err()
        .contains("InvalidIdentity"));
    assert!(application.interaction.is_some());
    application
        .handle_gui_action(GuiAction::SelectSubject(valid))
        .unwrap();

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn native_invocation_adapter_distinguishes_stale_target_and_platform_failure() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-action-outcome-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("hello.conduit");
    std::fs::write(&path, include_str!("../../../../examples/hello.conduit")).unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    let target = application
        .graphical_form
        .as_ref()
        .unwrap()
        .expanded_form_id
        .as_str()
        .to_owned();

    assert_eq!(
        application.apply_invocation(&patchbay_model::PatchbayInvocation {
            presentation_id: "presentation/test".into(),
            presentation_revision: 1,
            action_id: "action/open/test".into(),
            action: patchbay_model::PatchbayAction::OpenBack,
            target_identity: "expanded/stale".into(),
        }),
        patchbay_model::PatchbayInvocationOutcome::Refused(
            patchbay_model::PatchbayRefusal::StalePresentation
        )
    );
    assert!(application.back_navigation.is_empty());
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().open_form,
        "hello"
    );

    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&directory).unwrap();
    assert_eq!(
        application.apply_invocation(&patchbay_model::PatchbayInvocation {
            presentation_id: "presentation/test".into(),
            presentation_revision: 1,
            action_id: "action/save/test".into(),
            action: patchbay_model::PatchbayAction::Save,
            target_identity: target,
        }),
        patchbay_model::PatchbayInvocationOutcome::Failed
    );
}
