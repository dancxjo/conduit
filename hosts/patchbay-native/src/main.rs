//! Native window/event-loop adapter for Patchbay.

use patchbay_model::{FormEditor, GraphItemKind, PatchbayModel, PatchbayTopology};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const HISTORY_CAPACITY: usize = 4;
const MAX_FORM_PRESENTATION_LINES: usize = 256;
mod control;
mod render;
mod resource;
use control::NativeControl;
use render::{draw_document, BACKGROUND};
use resource::{open_form_resource, save_form_resource};

#[derive(Debug, Default, PartialEq, Eq)]
struct Arguments {
    exit_after_window: bool,
    snapshot_path: Option<PathBuf>,
    form_path: Option<PathBuf>,
    control_demo: bool,
    control_demo_stop: bool,
}

struct PatchbayApplication {
    model: PatchbayModel,
    topology_lines: Vec<String>,
    form_editor: Option<FormEditor>,
    form_selection: usize,
    modifiers: winit::keyboard::ModifiersState,
    control: NativeControl,
    window: Option<Rc<Window>>,
    exit_after_window: bool,
    rendered_once: bool,
    failure: Option<String>,
}

impl PatchbayApplication {
    fn new(arguments: Arguments) -> Result<Self, String> {
        let model = PatchbayModel::fresh();
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
        let mut application = Self {
            model,
            topology_lines,
            form_editor,
            form_selection: 0,
            modifiers: winit::keyboard::ModifiersState::empty(),
            control: NativeControl::new(),
            window: None,
            exit_after_window: arguments.exit_after_window,
            rendered_once: false,
            failure: None,
        };
        if arguments.control_demo || arguments.control_demo_stop {
            let editor = application
                .form_editor
                .as_ref()
                .ok_or("control demo requires --form")?;
            application.control.request_plan(editor)?;
            application.control.run(editor)?;
            if arguments.control_demo_stop {
                application.control.stop()?;
            }
        }
        Ok(application)
    }

    fn title(&self) -> String {
        if let Some(editor) = &self.form_editor {
            let view = editor.view();
            return format!(
                "Conduit Patchbay — {} — canonical Form revision {}",
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

    fn render(&mut self) -> Result<(), String> {
        let window = self.window.as_ref().ok_or("native window is absent")?;
        let size = window.inner_size();
        let width = NonZeroU32::new(size.width).ok_or("native window width is zero")?;
        let height = NonZeroU32::new(size.height).ok_or("native window height is zero")?;
        let context =
            softbuffer::Context::new(window.clone()).map_err(|error| error.to_string())?;
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|error| error.to_string())?;
        surface
            .resize(width, height)
            .map_err(|error| error.to_string())?;
        let mut buffer = surface.buffer_mut().map_err(|error| error.to_string())?;
        buffer.fill(BACKGROUND);
        let lines = self.presentation_lines();
        draw_document(
            &mut buffer,
            size.width as usize,
            size.height as usize,
            &lines,
        );
        buffer.present().map_err(|error| error.to_string())?;
        println!(
            "patchbay topology-rendered lines={} width={} height={}",
            lines.len(),
            size.width,
            size.height
        );
        self.rendered_once = true;
        Ok(())
    }

    fn presentation_lines(&self) -> Vec<String> {
        let Some(editor) = &self.form_editor else {
            return self.topology_lines.clone();
        };
        let view = editor.view();
        let mut lines = vec![
            format!("SOURCE {} revision={}", view.path.display(), view.revision),
            "  edit=end  Backspace=delete  Ctrl-S=save  Tab=open-next-back  Up/Down=select  F5=Plan F6=Run Esc=Stop".into(),
        ];
        lines.extend(
            view.source
                .lines()
                .take(MAX_FORM_PRESENTATION_LINES.saturating_sub(4))
                .map(|line| format!("  {line}")),
        );
        if let Some(diagnostic) = view.checked.diagnostics.first() {
            lines.push(format!(
                "DIAGNOSTIC {} {}:{}-{}:{} bytes={}..{} {}",
                diagnostic.code,
                diagnostic.span.line,
                diagnostic.span.column,
                diagnostic.span.end_line,
                diagnostic.span.end_column,
                diagnostic.span.start,
                diagnostic.span.end,
                diagnostic.message
            ));
            lines.truncate(MAX_FORM_PRESENTATION_LINES);
            return lines;
        }
        lines.push(format!(
            "CHECKED source={} forms={} OPEN BACK {}",
            view.checked
                .source_document_id
                .as_ref()
                .map(|id| id.as_str())
                .unwrap_or("none"),
            view.checked.forms.len(),
            view.open_form
        ));
        if let Some(form) = view
            .checked
            .forms
            .iter()
            .find(|form| form.name == view.open_form)
        {
            for (index, item) in form.items.iter().enumerate() {
                let marker = if index == self.form_selection {
                    ">"
                } else {
                    " "
                };
                let kind = match item.kind {
                    GraphItemKind::FaceInput => "face-in",
                    GraphItemKind::FaceOutput => "face-out",
                    GraphItemKind::StartupValue => "startup",
                    GraphItemKind::Cell => "cell",
                    GraphItemKind::Cord => "cord",
                };
                lines.push(format!(
                    "{marker} {kind} {} [{}..{}] {}",
                    item.identity, item.source_span.start, item.source_span.end, item.label
                ));
            }
        }
        lines.extend(self.control.lines());
        lines.truncate(MAX_FORM_PRESENTATION_LINES);
        lines
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
        if let Some(window) = &self.window {
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
            Key::Named(NamedKey::F5) => {
                self.control.request_plan(
                    self.form_editor
                        .as_ref()
                        .expect("editor presence was checked"),
                )?;
            }
            Key::Named(NamedKey::F6) => {
                self.control.run(
                    self.form_editor
                        .as_ref()
                        .expect("editor presence was checked"),
                )?;
            }
            Key::Named(NamedKey::Escape) => self.control.stop()?,
            Key::Character(character)
                if self.modifiers.control_key() && character.eq_ignore_ascii_case("s") =>
            {
                save_form_resource(
                    self.form_editor
                        .as_ref()
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
        if let Some(window) = &self.window {
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
                        || line.trim_start().starts_with("KERNEL-EVIDENCE ")
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
        event_loop.set_control_flow(if self.control.is_running() {
            ControlFlow::Poll
        } else {
            ControlFlow::Wait
        });
        if self.exit_after_window && self.rendered_once && !self.control.is_running() {
            event_loop.exit();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
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
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut application = PatchbayApplication::new(arguments)?;
    event_loop.run_app(&mut application)?;
    if let Some(error) = application.failure {
        return Err(error.into());
    }
    Ok(())
}

fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--smoke-exit-after-window" if !parsed.exit_after_window => {
                parsed.exit_after_window = true;
            }
            "--observatory-snapshot" if parsed.snapshot_path.is_none() => {
                let path = arguments
                    .next()
                    .ok_or("--observatory-snapshot requires a path")?;
                parsed.snapshot_path = Some(path.into());
            }
            "--form" if parsed.form_path.is_none() => {
                let path = arguments.next().ok_or("--form requires a path")?;
                parsed.form_path = Some(path.into());
            }
            "--control-demo" if !parsed.control_demo && !parsed.control_demo_stop => {
                parsed.control_demo = true;
            }
            "--control-demo-stop" if !parsed.control_demo && !parsed.control_demo_stop => {
                parsed.control_demo_stop = true;
            }
            _ => {
                return Err(format!(
                    "unsupported or repeated Patchbay argument: {argument}"
                ))
            }
        }
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
