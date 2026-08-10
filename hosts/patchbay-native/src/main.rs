//! Native window/event-loop adapter for Patchbay.

use conduit_core::SignId;
use patchbay_model::{
    BuildBirthController, DistributedRouteDemo, FormEditor, PatchbayInteraction, PatchbayModel,
    PatchbayTopology, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const HISTORY_CAPACITY: usize = 4;
mod arguments;
mod build_birth;
mod canvas;
mod canvas_input;
mod control;
mod distributed_play;
mod file_task;
mod font;
mod form_authoring;
mod form_interaction;
mod gui;
mod gui_face_controls;
mod gui_hit;
mod gui_inspector;
mod gui_primitives;
mod icon;
mod palette_icon;
mod palette_icon_data;
mod palette_input;
mod palette_view;
mod presentation;
mod render;
mod renderer_adapter;
mod resource;
use arguments::{parse_arguments, Arguments};
use conduit_std_host::StdHostComposition;
use control::NativeControl;
use distributed_play::{run_server as run_distributed_server, NativeDistributedPlay};
use file_task::{probe_native_file_base, NativeFileTask};
use render::{draw_document, BACKGROUND};
use resource::open_form_resource;

struct PatchbayApplication {
    model: PatchbayModel,
    topology_lines: Vec<String>,
    form_editor: Option<FormEditor>,
    form_selection: usize,
    graphical_form: Option<patchbay_model::PatchbayGraph>,
    layout: patchbay_model::PatchbayLayout,
    interaction: Option<PatchbayInteraction>,
    hit_targets: Vec<gui::HitTarget>,
    cursor_position: (f64, f64),
    linear_view: bool,
    modifiers: winit::keyboard::ModifiersState,
    palette_query: String,
    palette_search_active: bool,
    palette_drag: Option<String>,
    cord_drag: Option<patchbay_model::PatchbaySubjectRef>,
    gear_drag: Option<(patchbay_model::PatchbaySubjectRef, (f64, f64))>,
    control: NativeControl,
    build_birth: BuildBirthController,
    lifecycle_sequence: u64,
    file_task: NativeFileTask,
    route_demo: Option<DistributedRouteDemo>,
    renderer_execution: Option<RendererExecution>,
    distributed_play: Option<NativeDistributedPlay>,
    window: Option<Rc<Window>>,
    surface_context: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
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
        let graphical_form = form_editor
            .as_ref()
            .map(form_interaction::graphical_form_for_editor)
            .transpose()?
            .flatten();
        let mut layout = form_editor
            .as_ref()
            .map(resource::open_layout_resource)
            .transpose()?
            .unwrap_or_default();
        if let Some(graph) = &graphical_form {
            layout.reconcile(graph);
        }
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
            .then(|| NativeDistributedPlay::start(source_host_id.clone(), source_boot_id.clone()))
            .transpose()?;
        let mut application = Self {
            model,
            topology_lines,
            form_editor,
            form_selection: 0,
            graphical_form,
            layout,
            interaction: Some(PatchbayInteraction::new(source_host_id, source_boot_id)),
            hit_targets: Vec::new(),
            cursor_position: (0.0, 0.0),
            linear_view: false,
            modifiers: winit::keyboard::ModifiersState::empty(),
            palette_query: String::new(),
            palette_search_active: false,
            palette_drag: None,
            cord_drag: None,
            gear_drag: None,
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
                match softbuffer::Context::new(window.clone()).and_then(|context| {
                    softbuffer::Surface::new(&context, window.clone())
                        .map(|surface| (context, surface))
                }) {
                    Ok((context, surface)) => {
                        window.request_redraw();
                        self.surface_context = Some(context);
                        self.surface = Some(surface);
                        self.window = Some(window);
                    }
                    Err(error) => {
                        self.failure = Some(format!("cannot create native surface: {error}"));
                        event_loop.exit();
                    }
                }
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
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = (position.x, position.y);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } if !self.linear_view => {
                if let Err(error) = self.handle_canvas_press() {
                    self.failure = Some(format!("native canvas press failed: {error}"));
                    event_loop.exit();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } if !self.linear_view => {
                if let Err(error) = self.handle_canvas_release() {
                    self.failure = Some(format!("native canvas release failed: {error}"));
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if self.handle_palette_key(&event.logical_key) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else if let Err(error) = self.handle_form_key(&event.logical_key) {
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

fn render_linear_snapshot(path: &std::path::Path) -> Result<String, String> {
    let encoded =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let snapshot = serde_json::from_slice(&encoded)
        .map_err(|error| format!("cannot decode {}: {error}", path.display()))?;
    let mut topology = PatchbayTopology::new(1).map_err(|error| error.to_string())?;
    topology
        .ingest(&snapshot)
        .map_err(|error| error.to_string())?;
    Ok(topology
        .document(None)
        .map_err(|error| error.to_string())?
        .lines()
        .join("\n"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    if let Some(path) = arguments.linear_snapshot_path.as_deref() {
        println!("{}", render_linear_snapshot(path)?);
        return Ok(());
    }
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

#[cfg(test)]
#[path = "canvas_tests.rs"]
mod canvas_tests;

#[cfg(test)]
#[path = "font_tests.rs"]
mod font_tests;

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;

#[cfg(test)]
#[path = "gui_tests.rs"]
mod gui_tests;

#[cfg(test)]
#[path = "face_configuration_tests.rs"]
mod face_configuration_tests;
