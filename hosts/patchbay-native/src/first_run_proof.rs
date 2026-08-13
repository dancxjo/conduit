//! Finite native renderer/action-path acceptance for the first-run Patchbay journey.

use super::{gui::GuiAction, Arguments, PatchbayApplication, BACKGROUND};
use std::time::{Duration, Instant};
use winit::keyboard::{Key, NamedKey};

pub(super) const MAX_INTERACTIONS: u64 = 24;
pub(super) const MAX_WRONG_TURNS: u64 = 1;
pub(super) const MAX_ELAPSED_MILLIS: u128 = 30_000;

#[derive(Debug)]
pub(super) struct ShortTextEdit {
    pub(super) subject: patchbay_model::PatchbaySubjectRef,
    pub(super) key: String,
    pub(super) value: String,
    pub(super) maximum_bytes: usize,
}

pub(super) fn handle_short_text_key(
    application: &mut PatchbayApplication,
    key: &Key,
) -> Result<bool, String> {
    let Some(mut edit) = application.face_text_edit.take() else {
        return Ok(false);
    };
    match key {
        Key::Named(NamedKey::Escape) => application.interaction_status.publish(
            crate::interaction_status::InteractionStatusLevel::Information,
            crate::interaction_status::InteractionStatusCode::Cancelled,
            "Face text edit cancelled",
        ),
        Key::Named(NamedKey::Enter) => {
            application.dispatch_gear_configuration(
                &edit.subject,
                &edit.key,
                conduit_core::ConfigurationValue::Text(edit.value),
            )?;
        }
        Key::Named(NamedKey::Backspace) => {
            edit.value.pop();
            application.face_text_edit = Some(edit);
        }
        Key::Character(value)
            if !value.chars().any(char::is_control)
                && edit.value.len().saturating_add(value.len()) <= edit.maximum_bytes =>
        {
            edit.value.push_str(value);
            application.face_text_edit = Some(edit);
        }
        _ => application.face_text_edit = Some(edit),
    }
    Ok(true)
}

pub(super) fn run(mut arguments: Arguments) -> Result<(), String> {
    let started = Instant::now();
    let source_path = arguments
        .form_path
        .as_ref()
        .ok_or("first-run proof requires --form")?;
    let directory =
        std::env::temp_dir().join(format!("conduit-patchbay-first-run-{}", std::process::id()));
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create proof directory: {error}"))?;
    let proof_path = directory.join("greet.conduit");
    std::fs::copy(source_path, &proof_path)
        .map_err(|error| format!("cannot copy first-run Form: {error}"))?;
    arguments.form_path = Some(proof_path);
    arguments.first_run_proof = false;

    let result = execute(PatchbayApplication::new(arguments)?, started);
    let _ = std::fs::remove_dir_all(directory);
    result
}

fn execute(mut application: PatchbayApplication, started: Instant) -> Result<(), String> {
    let mut interactions = 0u64;
    let mut steps = Vec::with_capacity(16);
    observe(&mut application)?;
    require(
        application.back_breadcrumb() == "default-welcome",
        "default-welcome did not open",
    )?;
    steps.push("opened-default-welcome");

    let open = rendered_action(&application, |action| {
        matches!(action, GuiAction::OpenNavigatorComposition(_))
    })?;
    act(&mut application, open, &mut interactions)?;
    require(
        application.back_breadcrumb() == "default-welcome > hello : greet",
        "composition breadcrumb is not exact",
    )?;
    let graph = application
        .graphical_form
        .as_ref()
        .ok_or("greet graph absent")?;
    require(
        graph.face_inputs.len() == 1 && graph.face_outputs.len() == 1,
        "Face Ports absent",
    )?;
    steps.push("entered-hello-greet-with-face-ports");
    act(&mut application, GuiAction::OpenBack, &mut interactions)?;
    require(
        application.back_breadcrumb() == "default-welcome",
        "breadcrumb return failed",
    )?;
    steps.push("returned-through-breadcrumb");

    let literal = application
        .graphical_form
        .as_ref()
        .unwrap()
        .subject_ref("gear/default-welcome/literal")
        .map_err(|error| error.to_string())?;
    act(
        &mut application,
        GuiAction::SelectSubject(literal),
        &mut interactions,
    )?;
    let edit = rendered_action(
        &application,
        |action| matches!(action, GuiAction::BeginShortTextEdit { key, .. } if key == "value"),
    )?;
    act(&mut application, edit, &mut interactions)?;
    for _ in 0..5 {
        application.handle_form_key(&Key::Named(NamedKey::Backspace))?;
        interactions += 1;
    }
    application.handle_form_key(&Key::Character("Howdy".into()))?;
    interactions += 1;
    application.handle_form_key(&Key::Named(NamedKey::Enter))?;
    interactions += 1;
    let configured_source = application.form_editor.as_ref().unwrap().view().source;
    let configured_status = application
        .interaction_status
        .current()
        .map(|status| status.text.clone());
    require(
        configured_source.contains("Howdy"),
        &format!(
            "visible Face control did not configure Howdy; status={configured_status:?}; source={configured_source}"
        ),
    )?;
    steps.push("configured-howdy-through-visible-face-control");

    application.handle_palette_key(&Key::Character("/".into()));
    application.handle_palette_key(&Key::Character("uppercase".into()));
    application.handle_palette_key(&Key::Named(NamedKey::Enter));
    interactions += 3;
    require(
        application
            .graphical_form
            .as_ref()
            .unwrap()
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == "text/upper"),
        "Uppercase Gear was not added",
    )?;
    steps.push("found-and-added-uppercase");

    let (invalid_source, upper_input, old_cord) = {
        let graph = application.graphical_form.as_ref().unwrap();
        let literal = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == "default-welcome/literal")
            .unwrap();
        let upper = graph
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == "text/upper")
            .unwrap();
        let old = graph
            .cords
            .iter()
            .find(|cord| cord.source_port == literal.outputs[0].identity)
            .unwrap();
        (
            graph.subject_ref(&old.sink_port).unwrap(),
            graph.subject_ref(&upper.inputs[0].identity).unwrap(),
            graph.subject_ref(&old.identity).unwrap(),
        )
    };
    let before_refusal = application.form_editor.as_ref().unwrap().view().source;
    act(
        &mut application,
        GuiAction::ConnectPorts {
            source: invalid_source,
            sink: upper_input.clone(),
        },
        &mut interactions,
    )?;
    let refusal = application
        .interaction_status
        .current()
        .map(|status| status.text.clone());
    require(
        application.form_editor.as_ref().unwrap().view().source == before_refusal
            && refusal
                .as_deref()
                .is_some_and(|text| text.starts_with("Interaction refused:")),
        &format!("invalid connection did not refuse nonfatally: {refusal:?}"),
    )?;
    steps.push("recovered-one-invalid-connection");
    act(
        &mut application,
        GuiAction::RemoveCord(old_cord),
        &mut interactions,
    )?;
    let (literal_output, upper_input) = {
        let graph = application.graphical_form.as_ref().unwrap();
        let literal = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == "default-welcome/literal")
            .unwrap();
        let upper = graph
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == "text/upper")
            .unwrap();
        (
            graph.subject_ref(&literal.outputs[0].identity).unwrap(),
            graph.subject_ref(&upper.inputs[0].identity).unwrap(),
        )
    };
    act(
        &mut application,
        GuiAction::ConnectPorts {
            source: literal_output,
            sink: upper_input,
        },
        &mut interactions,
    )?;
    let (upper_output, show_input) = {
        let graph = application.graphical_form.as_ref().unwrap();
        let upper = graph
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == "text/upper")
            .unwrap();
        let show = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == "default-welcome/show")
            .unwrap();
        (
            graph.subject_ref(&upper.outputs[0].identity).unwrap(),
            graph.subject_ref(&show.inputs[0].identity).unwrap(),
        )
    };
    act(
        &mut application,
        GuiAction::ConnectPorts {
            source: upper_output,
            sink: show_input,
        },
        &mut interactions,
    )?;
    steps.push("wired-manifested-compatible-ports");

    let edited = application.form_editor.as_ref().unwrap().view().source;
    act(
        &mut application,
        GuiAction::UndoSemanticEdit,
        &mut interactions,
    )?;
    let undone = application.form_editor.as_ref().unwrap().view().source;
    require(undone != edited, "Undo did not move semantic history")?;
    act(
        &mut application,
        GuiAction::RedoSemanticEdit,
        &mut interactions,
    )?;
    require(
        application.form_editor.as_ref().unwrap().view().source == edited,
        "Redo was incoherent",
    )?;
    steps.push("undid-and-redid-semantic-edit");

    for lifecycle in [
        patchbay_model::PatchbayAction::BeBorn,
        patchbay_model::PatchbayAction::Wake,
        patchbay_model::PatchbayAction::Plan,
        patchbay_model::PatchbayAction::Play,
    ] {
        observe(&mut application)?;
        let action = rendered_action(
            &application,
            |candidate| matches!(candidate, GuiAction::Lifecycle(actual) if *actual == lifecycle),
        )?;
        act(&mut application, action, &mut interactions)?;
    }
    while application.control.is_running() {
        if started.elapsed().as_millis() > MAX_ELAPSED_MILLIS {
            return Err("first-run Play exceeded the pre-stated elapsed budget".into());
        }
        application.control.poll()?;
        std::thread::sleep(Duration::from_millis(1));
    }
    require(
        application.control.play_terminal() == Some(conduit_core::TerminalDisposition::Completed),
        &format!(
            "production-kernel Play did not complete: failure={:?} lines={:?}",
            application.control.play_failure(),
            application.control.lines()
        ),
    )?;
    let presentation = application
        .control
        .presentation()
        .ok_or("presentation absent")?;
    require(
        String::from_utf8_lossy(presentation).contains("HOWDY"),
        "resulting presentation omitted HOWDY",
    )?;
    let control_lines = application.control.lines();
    require(
        control_lines
            .iter()
            .any(|line| line.starts_with("RUN-TERMINAL "))
            && control_lines
                .iter()
                .any(|line| line.trim_start().starts_with("SIGN id="))
            && control_lines
                .iter()
                .any(|line| line.trim_start().starts_with("KERNEL-SIGN ")),
        "causal terminal or Sign evidence absent",
    )?;
    steps.push("birth-wake-plan-play-completed-with-causal-evidence");

    let elapsed = started.elapsed().as_millis();
    require(
        interactions <= MAX_INTERACTIONS,
        "interaction budget exceeded",
    )?;
    require(elapsed <= MAX_ELAPSED_MILLIS, "elapsed budget exceeded")?;
    println!(
        "{}",
        serde_json::json!({
            "schema": "conduit.patchbay.first-run-proof.v1",
            "completed": true,
            "steps": steps,
            "wrong_turns": MAX_WRONG_TURNS,
            "refusal_recovered": true,
            "interactions": interactions,
            "interaction_budget": MAX_INTERACTIONS,
            "elapsed_millis": elapsed,
            "elapsed_budget_millis": MAX_ELAPSED_MILLIS,
            "worker_count": 1,
            "retries": 0,
            "semantic_authority": "checked-form-identities",
            "execution_authority": "production-kernel",
            "sign_authority": "ordinary-play-document",
            "kernel_sign_observed": true,
            "terminal": "Completed"
        })
    );
    Ok(())
}

fn act(
    application: &mut PatchbayApplication,
    action: GuiAction,
    interactions: &mut u64,
) -> Result<(), String> {
    *interactions = interactions.saturating_add(1);
    application
        .handle_gui_action(action.clone())
        .map_err(|error| format!("{action:?}: {error}"))?;
    observe(application)
}

fn observe(application: &mut PatchbayApplication) -> Result<(), String> {
    let mut pixels = vec![BACKGROUND; 1_100 * 720];
    let graph = application
        .graphical_form
        .as_ref()
        .ok_or("native graph absent")?;
    let forms = application.form_navigator_entries();
    let selected = application.selected_graphical_identity().map(str::to_owned);
    let breadcrumb = application.back_breadcrumb();
    let status = application.interaction_status.current().cloned();
    let lifecycle = super::gui::LifecycleContext {
        flow: application.lifecycle_flow(),
        ..Default::default()
    };
    application.hit_targets = super::gui::draw_patchbay(
        &mut pixels,
        1_100,
        720,
        graph,
        super::gui::PatchbayViewContext {
            selected: selected.as_deref(),
            breadcrumb: &breadcrumb,
            lifecycle: &lifecycle,
            palette: &application.palette,
            forms: &forms,
            form_selection: application.navigator_selection,
            form_scroll: application.navigator_scroll,
            exact_identity_open: application.exact_identity_open,
            face_control_focus: application.face_control_focus,
            presentation_layout: &application.layout,
            realization_plan: None,
            realization_hosts: &[],
            status: status.as_ref(),
            gesture: Default::default(),
            viewport: &application.canvas_viewport,
        },
    );
    require(
        !application.hit_targets.is_empty(),
        "native renderer exposed no actionable state",
    )
}

fn rendered_action(
    application: &PatchbayApplication,
    predicate: impl Fn(&GuiAction) -> bool,
) -> Result<GuiAction, String> {
    application
        .hit_targets
        .iter()
        .find(|target| predicate(&target.action))
        .map(|target| target.action.clone())
        .ok_or_else(|| {
            format!(
                "required action was not exposed by the native renderer; actions={:?}",
                application
                    .hit_targets
                    .iter()
                    .map(|target| &target.action)
                    .collect::<Vec<_>>()
            )
        })
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}
