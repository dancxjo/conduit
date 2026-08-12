//! Renderer-local keyboard and pointer control of the finite canvas viewport.

use crate::{
    canvas_viewport::{ViewportError, PAN_STEP_PIXELS, ZOOM_STEP_PER_MILLE},
    gui, PatchbayApplication,
};
use embedded_graphics::geometry::Point;
use winit::keyboard::{Key, NamedKey};

impl PatchbayApplication {
    pub(super) fn perform_viewport_action(&mut self, action: crate::gui_hit::ViewportAction) {
        use crate::gui_hit::ViewportAction;
        let result = match action {
            ViewportAction::ZoomIn => self.zoom_viewport(ZOOM_STEP_PER_MILLE),
            ViewportAction::ZoomOut => self.zoom_viewport(-ZOOM_STEP_PER_MILLE),
            ViewportAction::Fit => self.fit_viewport(),
            ViewportAction::CenterSelection => self.center_selected(),
            ViewportAction::Reset => {
                self.canvas_viewport.reset();
                Ok(())
            }
        };
        match result {
            Ok(()) => self.publish_completed(format!(
                "Canvas view {}% at pan {}, {}",
                self.canvas_viewport.zoom_per_mille() / 10,
                self.canvas_viewport.offset().x,
                self.canvas_viewport.offset().y
            )),
            Err(error) => self.publish_refusal(error.message()),
        }
    }

    pub(super) fn canvas_world_cursor(&self) -> Result<Point, ViewportError> {
        self.canvas_viewport
            .screen_to_world(cursor_point(self.cursor_position)?)
    }

    pub(super) fn handle_viewport_key(&mut self, key: &Key) -> bool {
        if self.graphical_form.is_none() || self.linear_view {
            return false;
        }
        let result = match key {
            Key::Character(character)
                if self.modifiers.control_key() && (character == "+" || character == "=") =>
            {
                Some(self.zoom_viewport(ZOOM_STEP_PER_MILLE))
            }
            Key::Character(character) if self.modifiers.control_key() && character == "-" => {
                Some(self.zoom_viewport(-ZOOM_STEP_PER_MILLE))
            }
            Key::Character(character) if self.modifiers.control_key() && character == "0" => {
                self.canvas_viewport.reset();
                Some(Ok(()))
            }
            Key::Character(character)
                if self.modifiers.control_key() && character.eq_ignore_ascii_case("f") =>
            {
                Some(self.fit_viewport())
            }
            Key::Character(character)
                if self.modifiers.control_key() && character.eq_ignore_ascii_case("c") =>
            {
                Some(self.center_selected())
            }
            Key::Named(NamedKey::ArrowLeft) if self.modifiers.shift_key() => {
                Some(self.canvas_viewport.pan(Point::new(PAN_STEP_PIXELS, 0)))
            }
            Key::Named(NamedKey::ArrowRight) if self.modifiers.shift_key() => {
                Some(self.canvas_viewport.pan(Point::new(-PAN_STEP_PIXELS, 0)))
            }
            Key::Named(NamedKey::ArrowUp) if self.modifiers.shift_key() => {
                Some(self.canvas_viewport.pan(Point::new(0, PAN_STEP_PIXELS)))
            }
            Key::Named(NamedKey::ArrowDown) if self.modifiers.shift_key() => {
                Some(self.canvas_viewport.pan(Point::new(0, -PAN_STEP_PIXELS)))
            }
            _ => None,
        };
        let Some(result) = result else {
            return false;
        };
        match result {
            Ok(()) => self.publish_completed(format!(
                "Canvas view {}% at pan {}, {}",
                self.canvas_viewport.zoom_per_mille() / 10,
                self.canvas_viewport.offset().x,
                self.canvas_viewport.offset().y
            )),
            Err(error) => self.publish_refusal(error.message()),
        }
        true
    }

    pub(super) fn pan_viewport(&mut self, delta: Point) {
        match self.canvas_viewport.pan(delta) {
            Ok(()) => self.request_viewport_redraw(),
            Err(error) => self.publish_refusal(error.message()),
        }
    }

    pub(super) fn scroll_viewport(&mut self, horizontal: f32, vertical: f32) {
        if !horizontal.is_finite() || !vertical.is_finite() {
            self.publish_refusal(ViewportError::CoordinateOutOfBounds.message());
            return;
        }
        if self.modifiers.control_key() {
            let delta = if vertical > 0.0 {
                ZOOM_STEP_PER_MILLE
            } else if vertical < 0.0 {
                -ZOOM_STEP_PER_MILLE
            } else {
                return;
            };
            match self.zoom_viewport(delta) {
                Ok(()) => self.request_viewport_redraw(),
                Err(error) => self.publish_refusal(error.message()),
            }
        } else {
            let delta = Point::new(bounded_scroll(horizontal), bounded_scroll(vertical));
            self.pan_viewport(delta);
        }
    }

    fn zoom_viewport(&mut self, delta: i32) -> Result<(), ViewportError> {
        self.canvas_viewport
            .zoom_by(delta, cursor_point(self.cursor_position)?)
    }

    fn fit_viewport(&mut self) -> Result<(), ViewportError> {
        let graph = self
            .graphical_form
            .as_ref()
            .ok_or(ViewportError::EmptyContent)?;
        let bounds = gui::canvas_world_bounds(graph, self.viewport_window_width(), &self.layout)
            .ok_or(ViewportError::EmptyContent)?;
        self.canvas_viewport.fit(bounds)
    }

    fn center_selected(&mut self) -> Result<(), ViewportError> {
        let graph = self
            .graphical_form
            .as_ref()
            .ok_or(ViewportError::EmptyContent)?;
        let identity = self
            .selected_graphical_identity()
            .ok_or(ViewportError::EmptyContent)?;
        let center =
            gui::subject_world_center(graph, self.viewport_window_width(), &self.layout, identity)
                .ok_or(ViewportError::EmptyContent)?;
        self.canvas_viewport.center(center)
    }

    fn viewport_window_width(&self) -> i32 {
        self.window
            .as_ref()
            .map(|window| i32::try_from(window.inner_size().width).unwrap_or(i32::MAX))
            .unwrap_or(1_100)
    }

    fn request_viewport_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn cursor_point(cursor: (f64, f64)) -> Result<Point, ViewportError> {
    if !cursor.0.is_finite()
        || !cursor.1.is_finite()
        || cursor.0 < f64::from(i32::MIN)
        || cursor.0 > f64::from(i32::MAX)
        || cursor.1 < f64::from(i32::MIN)
        || cursor.1 > f64::from(i32::MAX)
    {
        return Err(ViewportError::CoordinateOutOfBounds);
    }
    Ok(Point::new(cursor.0 as i32, cursor.1 as i32))
}

fn bounded_scroll(value: f32) -> i32 {
    value
        .round()
        .clamp(-(PAN_STEP_PIXELS as f32), PAN_STEP_PIXELS as f32) as i32
}
