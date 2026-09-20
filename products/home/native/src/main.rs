mod evidence;
mod render;

use std::{num::NonZeroU32, process::Command, rc::Rc};

use conduit_home_model::{HomeEvent, HomeView};
use conduit_home_native::{
    NativeHomeController, NativeHomeLayout, NativeHomeRequest, execute_installed_form,
    run_native_home_journey,
};
use render::{Canvas, render};
use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().map(std::ffi::OsString::as_os_str)
        == Some(std::ffi::OsStr::new("--journey"))
    {
        let receipt = run_native_home_journey()?;
        println!(
            "HOME JOURNEY COMPLETE front={} revision={} form_plan={} form_play={} patchbay_presentation={} steps={}",
            receipt.host_front,
            receipt.final_revision,
            receipt.form_plan_id,
            receipt.form_play_id,
            receipt.patchbay_presentation_id,
            receipt.step_ids.join(",")
        );
        return Ok(());
    }
    if arguments.first().map(std::ffi::OsString::as_os_str)
        == Some(std::ffi::OsStr::new("--journey-evidence"))
    {
        if arguments.len() != 2 {
            return Err("--journey-evidence requires exactly one new output directory".into());
        }
        let receipt = run_native_home_journey()?;
        let path = evidence::retain(&receipt, std::path::Path::new(&arguments[1]))?;
        println!("HOME FRONT EVIDENCE COMPLETE: {}", path.display());
        return Ok(());
    }
    let event_loop = EventLoop::new()?;
    let mut application = NativeHome::new()?;
    event_loop.run_app(&mut application)?;
    if let Some(failure) = application.failure {
        return Err(failure.into());
    }
    Ok(())
}

struct NativeHome {
    controller: NativeHomeController,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    failure: Option<String>,
    cursor: Option<(f64, f64)>,
}

impl NativeHome {
    fn new() -> Result<Self, String> {
        let controller = NativeHomeController::new();
        controller
            .presentation()
            .map_err(|error| format!("native Home refused its initial presentation: {error:?}"))?;
        Ok(Self {
            controller,
            window: None,
            context: None,
            surface: None,
            failure: None,
            cursor: None,
        })
    }

    fn accept(&mut self, event: HomeEvent<'_>) {
        if let Some(request) = self.controller.accept(event) {
            self.realize(request);
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn realize(&mut self, request: NativeHomeRequest) {
        let outcome = match &request {
            NativeHomeRequest::OpenTour => spawn("conduit-tour", &[]),
            NativeHomeRequest::OpenPatchbay => spawn("patchbay-native", &["--front-door"]),
            NativeHomeRequest::OpenCreche => spawn("conduit", &["creche"]),
            NativeHomeRequest::OpenForm(_) => Ok(()),
            NativeHomeRequest::RunForm(index) => execute_installed_form(*index).map(|_| ()),
        };
        let reported = match &outcome {
            Ok(()) => Ok(()),
            Err(error) => Err(error.as_str()),
        };
        self.controller.report_request(&request, reported);
    }

    fn draw(&mut self) -> Result<(), String> {
        let window = self.window.as_ref().ok_or("native Home window is absent")?;
        let size = window.inner_size();
        let width = NonZeroU32::new(size.width.max(1)).ok_or("native Home width is zero")?;
        let height = NonZeroU32::new(size.height.max(1)).ok_or("native Home height is zero")?;
        let surface = self
            .surface
            .as_mut()
            .ok_or("native Home surface is absent")?;
        surface
            .resize(width, height)
            .map_err(|error| format!("cannot resize native Home surface: {error}"))?;
        let mut buffer = surface
            .buffer_mut()
            .map_err(|error| format!("cannot acquire native Home surface: {error}"))?;
        let presentation = self
            .controller
            .presentation()
            .map_err(|error| format!("native Home presentation refused: {error:?}"))?;
        let mut canvas = Canvas::new(&mut buffer, size.width as usize, size.height as usize);
        render(&mut canvas, &presentation, self.controller.notice());
        buffer
            .present()
            .map_err(|error| format!("cannot present native Home: {error}"))
    }
}

impl ApplicationHandler for NativeHome {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Conduit Home")
            .with_inner_size(winit::dpi::LogicalSize::new(1120.0, 720.0));
        match event_loop.create_window(attributes) {
            Ok(window) => {
                let window = Rc::new(window);
                match Context::new(window.clone()).and_then(|context| {
                    Surface::new(&context, window.clone()).map(|surface| (context, surface))
                }) {
                    Ok((context, surface)) => {
                        window.request_redraw();
                        self.window = Some(window);
                        self.context = Some(context);
                        self.surface = Some(surface);
                    }
                    Err(error) => {
                        self.failure = Some(format!("cannot create native Home surface: {error}"));
                        event_loop.exit();
                    }
                }
            }
            Err(error) => {
                self.failure = Some(format!("cannot create native Home window: {error}"));
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
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw() {
                    self.failure = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x, position.y));
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let hit = self.cursor.and_then(|(x, y)| {
                    self.window.as_ref().and_then(|window| {
                        NativeHomeLayout::hit_index(
                            self.controller.model().view(),
                            window.inner_size().width,
                            x,
                            y,
                        )
                    })
                });
                if let Some(index) = hit {
                    if let Some(request) = self.controller.activate_index(index) {
                        self.realize(request);
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key {
                    Key::Named(NamedKey::ArrowRight | NamedKey::ArrowDown | NamedKey::Tab) => {
                        self.accept(HomeEvent::Next)
                    }
                    Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp) => {
                        self.accept(HomeEvent::Previous)
                    }
                    Key::Named(NamedKey::Enter) => self.accept(HomeEvent::Activate),
                    Key::Named(NamedKey::Backspace) => self.accept(HomeEvent::Backspace),
                    Key::Named(NamedKey::Escape)
                        if self.controller.model().view() == HomeView::Launcher =>
                    {
                        event_loop.exit();
                    }
                    Key::Named(NamedKey::Escape) => self.accept(HomeEvent::Escape),
                    Key::Character(text) if !event.repeat => {
                        self.accept(HomeEvent::Text(text.as_ref()));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn spawn(program: &str, arguments: &[&str]) -> Result<(), String> {
    Command::new(program)
        .args(arguments)
        .spawn()
        .map(|_| ())
        .map_err(|_| "required native application is not installed beside Conduit".into())
}
