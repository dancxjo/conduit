use std::{num::NonZeroU32, rc::Rc};

use conduit_presentation::{ApplicationEvent, ApplicationNodeState, ApplicationView};
use conduit_tour_model::{
    NEXT_CHAPTER_ACTION_ID, NEXT_STAGE_ACTION_ID, OPEN_PATCHBAY_ACTION_ID,
    PREVIOUS_CHAPTER_ACTION_ID, PREVIOUS_STAGE_ACTION_ID, RUN_ACTION_ID, TourApplicationPort,
    TourWorkspaceRequest,
};
use conduit_tour_native::{DesktopPresentation, DesktopPresenter, HostedTourExecutor};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--journey")) {
        let report = conduit_tour_native::run_hosted_tour_journey()?;
        println!(
            "TOUR JOURNEY COMPLETE chapters={} exercises={} patchbays={}",
            report.chapters_visited, report.exercises_completed, report.patchbays_opened
        );
        return Ok(());
    }
    let event_loop = EventLoop::new()?;
    let mut application = NativeTour::new()?;
    event_loop.run_app(&mut application)?;
    if let Some(failure) = application.failure {
        return Err(failure.into());
    }
    Ok(())
}

struct NativeTour {
    port: TourApplicationPort,
    view: ApplicationView,
    presentation: DesktopPresentation,
    notice: String,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    failure: Option<String>,
}

impl NativeTour {
    fn new() -> Result<Self, String> {
        let mut port = TourApplicationPort::canonical();
        let bytes = port
            .apply(&[])
            .map_err(|error| format!("Tour did not emit its initial view: {error:?}"))?
            .view;
        let view = ApplicationView::decode(&bytes)
            .map_err(|error| format!("Tour emitted an invalid initial view: {error:?}"))?;
        let presentation = DesktopPresenter::project(&view)
            .map_err(|error| format!("desktop presentation refused the Tour: {error:?}"))?;
        Ok(Self {
            port,
            view,
            presentation,
            notice: "Left/Right chapter  |  Up/Down exercise  |  Enter run  |  P Patchbay".into(),
            window: None,
            context: None,
            surface: None,
            failure: None,
        })
    }

    fn activate(&mut self, action_id: &str) {
        let Some((action_index, action)) = self
            .view
            .actions
            .iter()
            .enumerate()
            .find(|(_, action)| action.id == action_id)
        else {
            self.notice = "That action is not available on this page.".into();
            return;
        };
        let action_index = u8::try_from(action_index).ok();
        let available =
            self.view.nodes.iter().any(|node| {
                node.action == action_index && node.state == ApplicationNodeState::Ready
            });
        if !available {
            self.notice = "That action is currently unavailable.".into();
            return;
        }
        let event = ApplicationEvent {
            revision: self.view.revision,
            action: action.id.clone(),
            kind: action.event,
            value: Vec::new(),
        };
        let encoded = match event.encode(&self.view) {
            Ok(encoded) => encoded,
            Err(error) => {
                self.notice = format!("Input refused: {error:?}");
                return;
            }
        };
        match self.port.apply(&encoded) {
            Ok(output) => {
                let run_requested =
                    matches!(output.request, Some(TourWorkspaceRequest::Run { .. }));
                self.notice = match output.request {
                    Some(TourWorkspaceRequest::Run { chapter, stage }) => {
                        match HostedTourExecutor::run(chapter, stage).and_then(|proof| {
                            self.port.complete_run(proof).map_err(|error| {
                                conduit_tour_native::HostedTourExecutorRefusal::Execution(format!(
                                    "shared Tour completion refused: {error:?}"
                                ))
                            })
                        }) {
                            Ok(()) => {
                                if let Ok(result) = self.port.apply(&[])
                                    && let Ok(view) = ApplicationView::decode(&result.view)
                                    && let Ok(presentation) = DesktopPresenter::project(&view)
                                {
                                    self.view = view;
                                    self.presentation = presentation;
                                }
                                format!(
                                    "Exercise {}.{} completed on this host.",
                                    chapter + 1,
                                    stage + 1
                                )
                            }
                            Err(error) => format!("Hosted exercise refused: {error:?}"),
                        }
                    }
                    Some(TourWorkspaceRequest::OpenPatchbay) => {
                        "Patchbay opened for the exact current form.".into()
                    }
                    None => "Shared Tour state updated.".into(),
                };
                if !run_requested && let Ok(view) = ApplicationView::decode(&output.view) {
                    match DesktopPresenter::project(&view) {
                        Ok(presentation) => {
                            self.view = view;
                            self.presentation = presentation;
                        }
                        Err(error) => self.notice = format!("Presentation refused: {error:?}"),
                    }
                }
            }
            Err(error) => self.notice = format!("Tour refused the action: {error:?}"),
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn draw(&mut self) -> Result<(), String> {
        let window = self.window.as_ref().ok_or("native window is absent")?;
        let size = window.inner_size();
        let width = NonZeroU32::new(size.width.max(1)).ok_or("window width is zero")?;
        let height = NonZeroU32::new(size.height.max(1)).ok_or("window height is zero")?;
        let surface = self.surface.as_mut().ok_or("native surface is absent")?;
        surface
            .resize(width, height)
            .map_err(|error| format!("cannot resize native surface: {error}"))?;
        let mut buffer = surface
            .buffer_mut()
            .map_err(|error| format!("cannot acquire native surface: {error}"))?;
        let mut canvas = Canvas::new(&mut buffer, size.width as usize, size.height as usize);
        render(&mut canvas, &self.presentation, &self.notice);
        buffer
            .present()
            .map_err(|error| format!("cannot present native Tour: {error}"))
    }
}

impl ApplicationHandler for NativeTour {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(format!("Conduit Tour - {}", self.presentation.title))
            .with_inner_size(winit::dpi::LogicalSize::new(1040.0, 720.0));
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
                        self.failure = Some(format!("cannot create native Tour surface: {error}"));
                        event_loop.exit();
                    }
                }
            }
            Err(error) => {
                self.failure = Some(format!("cannot create native Tour window: {error}"));
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
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let action = match event.logical_key {
                    Key::Named(NamedKey::ArrowLeft) => Some(PREVIOUS_CHAPTER_ACTION_ID),
                    Key::Named(NamedKey::ArrowRight) => Some(NEXT_CHAPTER_ACTION_ID),
                    Key::Named(NamedKey::ArrowUp) => Some(PREVIOUS_STAGE_ACTION_ID),
                    Key::Named(NamedKey::ArrowDown) => Some(NEXT_STAGE_ACTION_ID),
                    Key::Named(NamedKey::Enter) => Some(RUN_ACTION_ID),
                    Key::Character(ref text) if text.eq_ignore_ascii_case("p") => {
                        Some(OPEN_PATCHBAY_ACTION_ID)
                    }
                    Key::Named(NamedKey::Escape) => {
                        event_loop.exit();
                        None
                    }
                    _ => None,
                };
                if let Some(action) = action {
                    self.activate(action);
                }
            }
            _ => {}
        }
    }
}

fn render(canvas: &mut Canvas<'_>, presentation: &DesktopPresentation, notice: &str) {
    canvas.clear(Rgb888::new(246, 247, 251)).ok();
    Rectangle::new(Point::zero(), Size::new(canvas.width as u32, 58))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::new(24, 28, 45)))
        .draw(canvas)
        .ok();
    let heading = MonoTextStyle::new(&FONT_6X10, Rgb888::new(255, 255, 255));
    Text::new("CONDUIT  /  TOUR", Point::new(24, 34), heading)
        .draw(canvas)
        .ok();
    let body = MonoTextStyle::new(&FONT_6X10, Rgb888::new(40, 45, 63));
    let muted = MonoTextStyle::new(&FONT_6X10, Rgb888::new(92, 99, 120));
    let mut y = 88;
    for line in presentation.lines.iter().take(52) {
        let indent = 24 + i32::from(line.depth.min(5)) * 14;
        let style = if line.state == ApplicationNodeState::Unavailable {
            muted
        } else {
            body
        };
        for text in wrap(&line.text, 145).into_iter().take(3) {
            Text::new(&text, Point::new(indent, y), style)
                .draw(canvas)
                .ok();
            y += 14;
            if y > canvas.height as i32 - 70 {
                break;
            }
        }
        if y > canvas.height as i32 - 70 {
            break;
        }
        y += 3;
    }
    Rectangle::new(
        Point::new(0, canvas.height.saturating_sub(48) as i32),
        Size::new(canvas.width as u32, 48),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb888::new(231, 234, 243)))
    .draw(canvas)
    .ok();
    Text::new(
        &notice.chars().take(155).collect::<String>(),
        Point::new(24, canvas.height.saturating_sub(20) as i32),
        body,
    )
    .draw(canvas)
    .ok();
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    text.lines()
        .flat_map(|line| {
            let mut rows = Vec::new();
            let mut remaining = line;
            while remaining.chars().count() > width {
                let boundary = remaining
                    .char_indices()
                    .nth(width)
                    .map_or(remaining.len(), |(index, _)| index);
                let split = remaining[..boundary]
                    .rfind(char::is_whitespace)
                    .filter(|split| *split > 0)
                    .unwrap_or(boundary);
                rows.push(remaining[..split].trim().to_owned());
                remaining = remaining[split..].trim_start();
            }
            rows.push(remaining.to_owned());
            rows
        })
        .collect()
}

struct Canvas<'a> {
    pixels: &'a mut [u32],
    width: usize,
    height: usize,
}

impl<'a> Canvas<'a> {
    fn new(pixels: &'a mut [u32], width: usize, height: usize) -> Self {
        Self {
            pixels,
            width,
            height,
        }
    }
}

impl OriginDimensions for Canvas<'_> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl DrawTarget for Canvas<'_> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 {
                continue;
            }
            let (x, y) = (point.x as usize, point.y as usize);
            if x < self.width && y < self.height {
                self.pixels[y * self.width + x] =
                    u32::from(color.r()) << 16 | u32::from(color.g()) << 8 | u32::from(color.b());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn native_text_wrapping_preserves_unicode_at_exact_character_boundaries() {
        let rows = wrap("Birth, spores, and the Crèche are portable", 12);
        assert!(rows.len() > 1);
        assert_eq!(
            rows.concat().replace(' ', ""),
            "Birth,spores,andtheCrècheareportable".replace(' ', "")
        );
        assert!(rows.iter().all(|row| row.chars().count() <= 12));
    }
}
