//! Native window/event-loop adapter for Patchbay.

use font8x8::UnicodeFonts;
use patchbay_model::{PatchbayModel, PatchbayTopology};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const HISTORY_CAPACITY: usize = 4;
const BACKGROUND: u32 = 0x0015_1820;
const FOREGROUND: u32 = 0x00e7_eaf0;
const ACCENT: u32 = 0x006d_d7c7;
const LEFT_MARGIN: usize = 16;
const TOP_MARGIN: usize = 16;
const GLYPH_ADVANCE: usize = 8;
const LINE_ADVANCE: usize = 11;

#[derive(Debug, Default, PartialEq, Eq)]
struct Arguments {
    exit_after_window: bool,
    snapshot_path: Option<PathBuf>,
}

struct PatchbayApplication {
    model: PatchbayModel,
    topology_lines: Vec<String>,
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
        Ok(Self {
            model,
            topology_lines,
            window: None,
            exit_after_window: arguments.exit_after_window,
            rendered_once: false,
            failure: None,
        })
    }

    fn title(&self) -> String {
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
        draw_document(
            &mut buffer,
            size.width as usize,
            size.height as usize,
            &self.topology_lines,
        );
        buffer.present().map_err(|error| error.to_string())?;
        println!(
            "patchbay topology-rendered lines={} width={} height={}",
            self.topology_lines.len(),
            size.width,
            size.height
        );
        self.rendered_once = true;
        Ok(())
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
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.exit_after_window && self.rendered_once {
            event_loop.exit();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Err(error) = emit_report("shutdown", &self.model.shutdown_snapshot()) {
            self.failure = Some(format!("Patchbay shutdown report is invalid: {error}"));
        }
    }
}

fn draw_document(buffer: &mut [u32], width: usize, height: usize, lines: &[String]) {
    for (line_index, line) in lines.iter().enumerate() {
        let y = TOP_MARGIN + line_index * LINE_ADVANCE;
        if y + 8 >= height {
            break;
        }
        let color = if line.starts_with("HOSTS")
            || line.starts_with("LINKS")
            || line.starts_with("OBSERVATIONS")
        {
            ACCENT
        } else {
            FOREGROUND
        };
        for (character_index, character) in line.chars().enumerate() {
            let x = LEFT_MARGIN + character_index * GLYPH_ADVANCE;
            if x + 8 >= width {
                break;
            }
            draw_character(buffer, width, x, y, character, color);
        }
    }
}

fn draw_character(
    buffer: &mut [u32],
    width: usize,
    x: usize,
    y: usize,
    character: char,
    color: u32,
) {
    let glyph = font8x8::BASIC_FONTS
        .get(character)
        .or_else(|| font8x8::BASIC_FONTS.get('?'))
        .unwrap_or([0; 8]);
    for (row, bits) in glyph.iter().enumerate() {
        for column in 0..8 {
            if bits & (1 << column) != 0 {
                buffer[(y + row) * width + x + column] = color;
            }
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
mod tests {
    use super::{draw_document, parse_arguments, Arguments, BACKGROUND};
    use std::path::PathBuf;

    #[test]
    fn arguments_are_explicit_and_fail_closed() {
        assert_eq!(
            parse_arguments(Vec::new().into_iter()).unwrap(),
            Arguments::default()
        );
        assert!(
            parse_arguments(vec!["--smoke-exit-after-window".into()].into_iter())
                .unwrap()
                .exit_after_window
        );
        assert_eq!(
            parse_arguments(
                vec!["--observatory-snapshot".into(), "report.json".into()].into_iter()
            )
            .unwrap()
            .snapshot_path,
            Some(PathBuf::from("report.json"))
        );
        assert!(parse_arguments(vec!["--unknown".into()].into_iter()).is_err());
        assert!(parse_arguments(vec!["--observatory-snapshot".into()].into_iter()).is_err());
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
}
