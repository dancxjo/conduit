//! Native window/event-loop adapter for Patchbay.

use patchbay_model::PatchbayModel;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct PatchbayApplication {
    model: PatchbayModel,
    window: Option<Window>,
    exit_after_window: bool,
}

impl PatchbayApplication {
    fn new(exit_after_window: bool) -> Result<Self, String> {
        let model = PatchbayModel::fresh();
        emit_report("startup", &model.startup_snapshot())?;
        Ok(Self {
            model,
            window: None,
            exit_after_window,
        })
    }

    fn title(&self) -> String {
        format!(
            "Conduit Patchbay — host {} — boot {} — operations {} — planners {}",
            self.model.projection().host_id().as_str(),
            self.model.projection().boot_id().as_str(),
            self.model.projection().capability_ids().len(),
            self.model.projection().planner_profile_count(),
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
            .with_inner_size(winit::dpi::LogicalSize::new(720.0, 240.0));
        match event_loop.create_window(attributes) {
            Ok(window) => self.window = Some(window),
            Err(error) => {
                eprintln!("Patchbay could not create its native window: {error}");
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
        if self.window.as_ref().map(Window::id) != Some(window_id) {
            return;
        }
        if event == WindowEvent::CloseRequested {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.exit_after_window && self.window.is_some() {
            event_loop.exit();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Err(error) = emit_report("shutdown", &self.model.shutdown_snapshot()) {
            eprintln!("Patchbay shutdown report is invalid: {error}");
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
    let exit_after_window = parse_arguments(std::env::args().skip(1))?;
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut application = PatchbayApplication::new(exit_after_window)?;
    event_loop.run_app(&mut application)?;
    Ok(())
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<bool, String> {
    let mut exit_after_window = false;
    for argument in arguments {
        if argument == "--smoke-exit-after-window" && !exit_after_window {
            exit_after_window = true;
        } else {
            return Err(format!("unsupported Patchbay argument: {argument}"));
        }
    }
    Ok(exit_after_window)
}

#[cfg(test)]
mod tests {
    use super::parse_arguments;

    #[test]
    fn smoke_exit_is_explicit_and_unknown_arguments_fail_closed() {
        assert!(!parse_arguments(Vec::new().into_iter()).unwrap());
        assert!(parse_arguments(vec!["--smoke-exit-after-window".into()].into_iter()).unwrap());
        assert!(parse_arguments(vec!["--unknown".into()].into_iter()).is_err());
        assert!(parse_arguments(
            vec![
                "--smoke-exit-after-window".into(),
                "--smoke-exit-after-window".into()
            ]
            .into_iter()
        )
        .is_err());
    }
}
