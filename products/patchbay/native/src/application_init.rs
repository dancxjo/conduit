//! Native application composition and exclusive workspace initialization.

use super::*;

impl PatchbayApplication {
    pub(super) fn new(arguments: Arguments) -> Result<Self, String> {
        let pico_admission_port = arguments.pico_admission_port.clone();
        let native_file_base = probe_native_file_base();
        let mut composition = StdHostComposition::minimal()
            .with_signal()
            .with_time()
            .with_text()
            .with_input()
            .with_state();
        if native_file_base.is_some() {
            composition = composition.with_files();
        }
        let model = hosted_adapter::fresh_model(composition, |advertisement| {
            portable_keyboard::append_offer(advertisement)?;
            portable_keyboard::append_button_offers(advertisement)
        })?;
        emit_report("startup", &model.startup_snapshot())?;
        let mut topology =
            PatchbayTopology::new(HISTORY_CAPACITY).map_err(|error| error.to_string())?;
        let observed_environment_snapshot = if let Some(path) = arguments.snapshot_path {
            let encoded = std::fs::read(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let snapshot = serde_json::from_slice(&encoded)
                .map_err(|error| format!("cannot decode {}: {error}", path.display()))?;
            topology
                .ingest(&snapshot)
                .map_err(|error| error.to_string())?;
            Some(snapshot)
        } else {
            topology
                .ingest(&model.startup_snapshot())
                .map_err(|error| error.to_string())?;
            None
        };
        let topology_lines = topology
            .document(None)
            .map_err(|error| error.to_string())?
            .lines()
            .to_vec();
        let workspace = workspace_open::open_workspace(
            arguments.form_path.clone(),
            arguments.environment_path,
            arguments.prewake,
        )?;
        let semantic_history = workspace
            .form_editor
            .as_ref()
            .zip(workspace.graphical_form.as_ref())
            .map(|(editor, graph)| {
                semantic_history::SemanticHistory::new(
                    semantic_history::SemanticCheckpoint::from_editor(editor, graph)?,
                )
                .map_err(|error| format!("semantic history: {error:?}"))
            })
            .transpose()?;
        if arguments.prewake && (workspace.form_editor.is_none() || workspace.environment.is_none())
        {
            return Err("--prewake requires both --form and --environment".into());
        }
        let mut prewake = arguments.prewake.then(|| {
            patchbay_model::PrewakeController::new(std::sync::Arc::new(
                patchbay_hosted::HostedPatchbayAdapter,
            ))
        });
        if let Some(controller) = &mut prewake {
            controller.set_hold(arguments.prewake_hold);
            controller
                .enter(
                    workspace
                        .form_editor
                        .as_ref()
                        .expect("PREWAKE Form checked"),
                    workspace
                        .environment
                        .as_ref()
                        .expect("PREWAKE environment checked"),
                )
                .map_err(|error| error.to_string())?;
        }
        let source_host_id = model.projection().host_id().clone();
        let source_boot_id = model.projection().boot_id().clone();
        let native_keyboard = portable_keyboard::NativeKeyboardInput::new();
        let control = NativeControl::for_advertisement(
            model.advertisement().clone(),
            native_keyboard.reader(),
        )?;
        let file_task = NativeFileTask::for_host(
            native_file_base,
            source_host_id.clone(),
            source_boot_id.clone(),
            composition,
        );
        let route_demo = (arguments.distributed_route_demo || arguments.distributed_play)
            .then(|| DistributedRouteDemo::build_for_source(model.advertisement().clone()))
            .transpose()
            .map_err(|error| format!("distributed route demo: {error:?}"))?;
        let renderer_execution = (arguments.distributed_route_demo || arguments.distributed_play)
            .then(|| {
                RendererExecution::prepare(
                    patchbay_model::portable_demonstration_with_adapter(
                        &patchbay_hosted::HostedPatchbayAdapter,
                    )?,
                    RendererAdapterKind::NativeWayland,
                    RendererAdapterIdentity {
                        host_id: source_host_id.clone(),
                        boot_id: source_boot_id.clone(),
                        target_subject: "patchbay-native/window-0".into(),
                    },
                    SignId::from("patchbay-native/manifestation-prepared"),
                )
                .map_err(|error| error.to_string())
            })
            .transpose()?;
        let distributed_play = arguments
            .distributed_play
            .then(|| NativeDistributedPlay::start(source_host_id.clone(), source_boot_id.clone()))
            .transpose()?;
        let mut application = Self {
            model,
            topology_lines,
            form_editor: workspace.form_editor,
            semantic_history,
            environment: workspace.environment,
            environment_path: workspace.environment_path,
            selected_environment_part: None,
            environment_drag: None,
            pending_environment_link: None,
            environment_name_editing: false,
            observed_environment_snapshot,
            prewake,
            prewake_environment_view: arguments.prewake,
            form_selection: 0,
            navigator_selection: 0,
            navigator_scroll: 0,
            back_navigation: Vec::with_capacity(forms_navigation::MAX_BACK_NAVIGATION_DEPTH),
            pending_back_target: None,
            pending_back_selection: false,
            graphical_form: workspace.graphical_form,
            body_workbench: Default::default(),
            layout: workspace.layout,
            debugger: None,
            interaction: Some(PatchbayInteraction::new(source_host_id, source_boot_id)),
            entrance: None,
            zero_body_front_door: None,
            hit_targets: Vec::new(),
            cursor_position: (0.0, 0.0),
            canvas_viewport: Default::default(),
            canvas_pan_drag: None,
            linear_view: false,
            details_lens: Default::default(),
            details_scroll: 0,
            modifiers: winit::keyboard::ModifiersState::empty(),
            native_keyboard,
            palette: Default::default(),
            selected_follow: None,
            exact_identity_open: false,
            parts_open: false,
            selected_part: None,
            selected_candidate: None,
            pending_revoke: None,
            body_candidates: None,
            browser_parts: arguments
                .browser_page_url
                .zip(arguments.browser_chat_url)
                .map(|(page, chat)| browser_parts::BrowserPartsCoordinator::new(page, chat)),
            pico_parts: None,
            face_control_focus: 0,
            face_text_edit: None,
            palette_drag: None,
            cord_drag: None,
            cord_route_drag: None,
            gear_drag: None,
            last_gear_click: None,
            interaction_status: Default::default(),
            control,
            build_birth: BuildBirthController::new(),
            lifecycle_sequence: 0,
            file_task,
            route_demo,
            renderer_execution,
            distributed_play,
            window: None,
            surface_context: None,
            surface: None,
            exit_after_window: arguments.exit_after_window,
            rendered_once: false,
            failure: None,
        };
        if let Some(path) = arguments.body_evidence_path {
            let entrance = arguments
                .body_entrance
                .expect("validated Body evidence arguments include an entrance");
            let encoded = std::fs::read(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let graph = application
                .graphical_form
                .as_ref()
                .ok_or("Body workbench requires a checked graphical Form")?;
            application
                .body_workbench
                .replace(1, encoded, entrance, graph)
                .map_err(|error| format!("native Body workbench: {error}"))?;
        }
        if arguments.control_demo || arguments.control_demo_stop {
            application.birth_body()?;
            application.wake_body()?;
            application.plan_play()?;
            application.play_plan()?;
            if arguments.control_demo_stop {
                application.stop_play()?;
            }
        }
        if arguments.body_parts_demo {
            application.birth_body()?;
            application.parts_open = true;
            if let Some(path) = pico_admission_port {
                let body_id = application
                    .build_birth
                    .body()
                    .expect("Body membership demo completed Birth")
                    .body_id
                    .clone();
                application.pico_parts =
                    Some(pico_parts::PicoPartsCoordinator::start(body_id, path)?);
                application.publish_completed(
                    "Observing the configured Pico USB Line; membership remains unchanged",
                );
            }
            if let Some(coordinator) = &mut application.browser_parts {
                let body_id = application
                    .build_birth
                    .body()
                    .expect("Body membership demo completed Birth")
                    .body_id
                    .clone();
                let target = coordinator.start_ambient(&body_id)?;
                std::process::Command::new("xdg-open")
                    .arg(target)
                    .spawn()
                    .map_err(|error| format!("cannot open ambient browser candidate: {error}"))?;
                application.publish_completed(
                    "Ambient browser opened as an inert candidate; inspect and Admit explicitly",
                );
            }
        }
        if arguments.native_copy_demo {
            application.file_task.run_choice_demo()?;
        }
        if arguments.front_door {
            application.initialize_front_door()?;
        }
        Ok(application)
    }
}
