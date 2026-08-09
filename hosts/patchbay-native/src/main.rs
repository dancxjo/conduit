//! Native window/event-loop adapter for Patchbay.

use conduit_core::SignId;
use patchbay_model::{
    BuildBirthController, DistributedRouteDemo, FormEditor, PatchbayModel, PatchbayTopology,
    RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const HISTORY_CAPACITY: usize = 4;
mod arguments;
mod build_birth;
mod control;
mod distributed_play;
mod file_task;
mod presentation;
mod render;
mod renderer_adapter;
mod resource;
use arguments::{parse_arguments, Arguments};
use conduit_std_host::StdHostComposition;
use control::NativeControl;
use distributed_play::{run_server as run_distributed_server, NativeDistributedPlay};
use file_task::{probe_native_file_base, DestinationPolicy, NativeFileTask};
use render::{draw_document, BACKGROUND};
use resource::{open_form_resource, save_form_resource};

struct PatchbayApplication {
    model: PatchbayModel,
    topology_lines: Vec<String>,
    form_editor: Option<FormEditor>,
    form_selection: usize,
    modifiers: winit::keyboard::ModifiersState,
    control: NativeControl,
    build_birth: BuildBirthController,
    lifecycle_sequence: u64,
    file_task: NativeFileTask,
    route_demo: Option<DistributedRouteDemo>,
    renderer_execution: Option<RendererExecution>,
    distributed_play: Option<NativeDistributedPlay>,
    window: Option<Rc<Window>>,
    exit_after_window: bool,
    rendered_once: bool,
    failure: Option<String>,
}

impl PatchbayApplication {
    fn new(arguments: Arguments) -> Result<Self, String> {
        let native_file_base = probe_native_file_base();
        let mut composition = StdHostComposition::minimal()
            .with_signal()
            .with_time()
            .with_text()
            .with_state();
        if native_file_base.is_some() {
            composition = composition.with_files();
        }
        let model = PatchbayModel::fresh_with_composition(composition);
        emit_report("startup", &model.startup_snapshot())?;
        let mut topology =
            PatchbayTopology::new(HISTORY_CAPACITY).map_err(|error| error.to_string())?;
        if let Some(path) = arguments.snapshot_path {
            let encoded = std::fs::read(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let snapshot = serde_json::from_slice(&encoded)
                .map_err(|error| format!("cannot decode {}: {error}", path.display()))?;
            topology
                .ingest(&snapshot)
                .map_err(|error| error.to_string())?;
        } else {
            topology
                .ingest(&model.startup_snapshot())
                .map_err(|error| error.to_string())?;
        }
        let topology_lines = topology
            .document(None)
            .map_err(|error| error.to_string())?
            .lines()
            .to_vec();
        let form_editor = arguments
            .form_path
            .map(open_form_resource)
            .transpose()
            .map_err(|error| error.to_string())?;
        let source_host_id = model.projection().host_id().clone();
        let source_boot_id = model.projection().boot_id().clone();
        let control =
            NativeControl::for_host(source_host_id.clone(), source_boot_id.clone(), composition);
        let file_task = NativeFileTask::for_host(
            native_file_base,
            source_host_id.clone(),
            source_boot_id.clone(),
            composition,
        );
        let route_demo = (arguments.distributed_route_demo || arguments.distributed_play)
            .then(|| {
                DistributedRouteDemo::build_for_source(
                    source_host_id.clone(),
                    source_boot_id.clone(),
                )
            })
            .transpose()
            .map_err(|error| format!("distributed route demo: {error:?}"))?;
        let renderer_execution = (arguments.distributed_route_demo || arguments.distributed_play)
            .then(|| {
                RendererExecution::prepare(
                    patchbay_model::portable_demonstration()?,
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
            .then(|| NativeDistributedPlay::start(source_host_id, source_boot_id))
            .transpose()?;
        let mut application = Self {
            model,
            topology_lines,
            form_editor,
            form_selection: 0,
            modifiers: winit::keyboard::ModifiersState::empty(),
            control,
            build_birth: BuildBirthController::new(),
            lifecycle_sequence: 0,
            file_task,
            route_demo,
            renderer_execution,
            distributed_play,
            window: None,
            exit_after_window: arguments.exit_after_window,
            rendered_once: false,
            failure: None,
        };
        if arguments.control_demo || arguments.control_demo_stop {
            application.birth_body()?;
            application.wake_body()?;
            application.plan_play()?;
            application.play_plan()?;
            if arguments.control_demo_stop {
                application.control.stop()?;
            }
        }
        if arguments.native_copy_demo {
            application.file_task.run_choice_demo()?;
        }
        Ok(application)
    }

    fn title(&self) -> String {
        if let Some(editor) = &self.form_editor {
            let view = editor.view();
            let mode = self
                .build_birth
                .document(editor)
                .map(|document| format!("{:?}", document.mode))
                .unwrap_or_else(|_| "BuildInvalid".into());
            return format!(
                "Conduit Patchbay — {mode} — {} — canonical Form revision {}",
                view.path.display(),
                view.revision
            );
        }
        format!(
            "Conduit Patchbay — host {} — boot {} — topology lines {}",
            self.model.projection().host_id().as_str(),
            self.model.projection().boot_id().as_str(),
            self.topology_lines.len(),
        )
    }

    fn edit_source(&mut self, update: impl FnOnce(&mut String)) -> Result<(), String> {
        let editor = self
            .form_editor
            .as_mut()
            .ok_or("canonical Form editor is absent")?;
        let mut source = editor.view().source;
        update(&mut source);
        editor
            .replace_source(source)
            .map_err(|error| error.to_string())?;
        editor.recheck().map_err(|error| error.to_string())?;
        self.form_selection = 0;
        let title = self.title();
        if let Some(window) = &self.window {
            window.set_title(&title);
            window.request_redraw();
        }
        Ok(())
    }

    fn handle_form_key(&mut self, key: &Key) -> Result<bool, String> {
        if self.form_editor.is_none() {
            return Ok(false);
        }
        match key {
            Key::Named(NamedKey::Backspace) => self.edit_source(|source| {
                source.pop();
            })?,
            Key::Named(NamedKey::Enter) => self.edit_source(|source| source.push('\n'))?,
            Key::Named(NamedKey::Tab) => {
                let editor = self
                    .form_editor
                    .as_mut()
                    .expect("editor presence was checked");
                let view = editor.view();
                if !view.checked.forms.is_empty() {
                    let current = view
                        .checked
                        .forms
                        .iter()
                        .position(|form| form.name == view.open_form)
                        .unwrap_or(0);
                    let next = &view.checked.forms[(current + 1) % view.checked.forms.len()].name;
                    editor.open_back(next).map_err(|error| error.to_string())?;
                    self.form_selection = 0;
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                let editor = self
                    .form_editor
                    .as_ref()
                    .expect("editor presence was checked");
                let view = editor.view();
                let count = editor
                    .view()
                    .checked
                    .forms
                    .iter()
                    .find(|form| form.name == view.open_form)
                    .map(|form| form.items.len())
                    .unwrap_or(0);
                if count > 0 {
                    self.form_selection = (self.form_selection + 1) % count;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.form_selection = self.form_selection.saturating_sub(1)
            }
            Key::Named(NamedKey::F4) => self.birth_body()?,
            Key::Named(NamedKey::F5) => self.wake_body()?,
            Key::Named(NamedKey::F6) => self.plan_play()?,
            Key::Named(NamedKey::F7) if !self.modifiers.alt_key() => self.play_plan()?,
            Key::Named(NamedKey::F8) if !self.modifiers.alt_key() => self.mark_unsatisfied()?,
            Key::Named(NamedKey::F9) if !self.modifiers.alt_key() => self.lull_body()?,
            Key::Named(NamedKey::Escape) => self.control.stop()?,
            Key::Named(NamedKey::F7) => {
                self.file_task.choose_source()?;
            }
            Key::Named(NamedKey::F8) => {
                let policy = if self.modifiers.shift_key() {
                    DestinationPolicy::Replace
                } else {
                    DestinationPolicy::Create
                };
                self.file_task.choose_destination(policy)?;
            }
            Key::Named(NamedKey::F9) => self.file_task.plan()?,
            Key::Named(NamedKey::F10) => self.file_task.run()?,
            Key::Named(NamedKey::F11) => self.file_task.stop()?,
            Key::Character(character)
                if self.modifiers.control_key() && character.eq_ignore_ascii_case("s") =>
            {
                save_form_resource(
                    self.form_editor
                        .as_mut()
                        .expect("editor presence was checked"),
                )?;
            }
            Key::Character(character)
                if !self.modifiers.control_key() && !self.modifiers.super_key() =>
            {
                let characters = character.clone();
                self.edit_source(|source| source.push_str(&characters))?;
            }
            _ => return Ok(false),
        }
        let editor = self
            .form_editor
            .as_mut()
            .expect("editor presence was checked");
        let view = editor.view();
        if let Some(identity) = view
            .checked
            .forms
            .iter()
            .find(|form| form.name == view.open_form)
            .and_then(|form| form.items.get(self.form_selection))
            .map(|item| item.identity.clone())
        {
            editor.select_graph_item(&identity);
        }
        let title = self.title();
        if let Some(window) = &self.window {
            window.set_title(&title);
            window.request_redraw();
        }
        Ok(true)
    }
}

impl ApplicationHandler for PatchbayApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.title())
            .with_inner_size(winit::dpi::LogicalSize::new(1100.0, 720.0));
        match event_loop.create_window(attributes) {
            Ok(window) => {
                let window = Rc::new(window);
                window.request_redraw();
                self.window = Some(window);
            }
            Err(error) => {
                self.failure = Some(format!("cannot create native window: {error}"));
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(|window| window.id()) != Some(window_id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.render() {
                    self.failure = Some(format!("cannot render native topology view: {error}"));
                    event_loop.exit();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if let Err(error) = self.handle_form_key(&event.logical_key) {
                    self.failure = Some(format!("canonical Form edit failed: {error}"));
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.file_task.poll() {
            Ok(true) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                self.rendered_once = false;
            }
            Ok(false) => {}
            Err(error) => {
                self.failure = Some(format!("Native file task failed: {error}"));
                event_loop.exit();
                return;
            }
        }
        match self.control.poll() {
            Ok(true) => {
                for line in self.control.lines().iter().filter(|line| {
                    line.starts_with("PLAN ")
                        || line.starts_with("PLAY ")
                        || line.starts_with("PLAN-ACTION ")
                        || line.starts_with("RUN ")
                        || line.starts_with("STOP ")
                        || line.starts_with("RUN-TERMINAL ")
                        || line.trim_start().starts_with("CONTROL ")
                        || line.trim_start().starts_with("KERNEL-SIGN ")
                }) {
                    println!("patchbay control {line}");
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                self.rendered_once = false;
            }
            Ok(false) => {}
            Err(error) => {
                self.failure = Some(format!("Play control failed: {error}"));
                event_loop.exit();
                return;
            }
        }
        if let Some(distributed) = &mut self.distributed_play {
            match distributed.poll() {
                Ok(true) => {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    self.rendered_once = false;
                }
                Ok(false) => {}
                Err(error) => {
                    self.failure = Some(format!("Distributed Play failed: {error}"));
                    event_loop.exit();
                    return;
                }
            }
        }
        event_loop.set_control_flow(
            if self.control.is_running()
                || self.file_task.is_running()
                || self
                    .distributed_play
                    .as_ref()
                    .is_some_and(NativeDistributedPlay::is_running)
            {
                ControlFlow::Poll
            } else {
                ControlFlow::Wait
            },
        );
        if self.exit_after_window
            && self.rendered_once
            && !self.control.is_running()
            && !self.file_task.is_running()
            && !self
                .distributed_play
                .as_ref()
                .is_some_and(NativeDistributedPlay::is_running)
        {
            event_loop.exit();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(execution) = &mut self.renderer_execution {
            if execution.manifestation.lifecycle
                == conduit_presentation::ManifestationLifecycle::Available
            {
                if let Err(error) =
                    execution.mark_closed(SignId::from("patchbay-native/window-closed"))
                {
                    self.failure = Some(format!("cannot close native Manifestation: {error}"));
                }
            }
        }
        if let Err(error) = emit_report("shutdown", &self.model.shutdown_snapshot()) {
            self.failure = Some(format!("Patchbay shutdown report is invalid: {error}"));
        }
    }
}

fn emit_report(
    phase: &str,
    snapshot: &conduit_observatory::ObservatorySnapshot,
) -> Result<(), String> {
    let report = conduit_observatory::build_report(snapshot)?;
    println!(
        "patchbay lifecycle={phase}\n{}",
        conduit_observatory::render_text_report(&report)
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    if arguments.distributed_play_server {
        run_distributed_server()?;
        return Ok(());
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut application = PatchbayApplication::new(arguments)?;
    event_loop.run_app(&mut application)?;
    if let Some(error) = application.failure {
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
