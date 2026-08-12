//! Finite renderer-local mapping between canonical layout and canvas pixels.

use crate::gui_primitives::PixelRect;
use embedded_graphics::geometry::Point;

pub const MIN_ZOOM_PER_MILLE: i32 = 500;
pub const MAX_ZOOM_PER_MILLE: i32 = 2_000;
pub const ZOOM_STEP_PER_MILLE: i32 = 125;
pub const PAN_STEP_PIXELS: i32 = 48;
const MAX_OFFSET: i32 = 1_000_000;
const MAX_WORLD_COORDINATE: i32 = 1_000_000;
const FIT_PADDING: i32 = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewportError {
    CoordinateOutOfBounds,
    ArithmeticOverflow,
    ZoomOutOfBounds,
    EmptyCanvas,
    EmptyContent,
}

impl ViewportError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::CoordinateOutOfBounds => "Canvas coordinate is outside the finite viewport range",
            Self::ArithmeticOverflow => "Canvas viewport arithmetic overflowed",
            Self::ZoomOutOfBounds => "Canvas zoom reached its finite bound",
            Self::EmptyCanvas => "Canvas viewport has no drawable area",
            Self::EmptyContent => "The Form has no canvas content to frame",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldBounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WorldBounds {
    pub fn from_rect(rect: PixelRect) -> Result<Self, ViewportError> {
        let width = i32::try_from(rect.width).map_err(|_| ViewportError::CoordinateOutOfBounds)?;
        let height =
            i32::try_from(rect.height).map_err(|_| ViewportError::CoordinateOutOfBounds)?;
        Ok(Self {
            left: rect.x,
            top: rect.y,
            right: rect
                .x
                .checked_add(width)
                .ok_or(ViewportError::ArithmeticOverflow)?,
            bottom: rect
                .y
                .checked_add(height)
                .ok_or(ViewportError::ArithmeticOverflow)?,
        })
    }

    pub fn include(&mut self, other: Self) {
        self.left = self.left.min(other.left);
        self.top = self.top.min(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }

    pub fn center(self) -> Result<Point, ViewportError> {
        Ok(Point::new(
            midpoint(self.left, self.right)?,
            midpoint(self.top, self.bottom)?,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanvasViewport {
    zoom_per_mille: i32,
    offset: Point,
    canvas: PixelRect,
    initialized: bool,
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            zoom_per_mille: 1_000,
            offset: Point::zero(),
            canvas: PixelRect {
                x: 176,
                y: 52,
                width: 640,
                height: 626,
            },
            initialized: false,
        }
    }
}

impl CanvasViewport {
    pub const fn zoom_per_mille(&self) -> i32 {
        self.zoom_per_mille
    }

    pub const fn offset(&self) -> Point {
        self.offset
    }

    pub const fn canvas(&self) -> PixelRect {
        self.canvas
    }

    pub fn resize(&mut self, canvas: PixelRect) -> Result<(), ViewportError> {
        validate_canvas(canvas)?;
        if !self.initialized {
            self.canvas = canvas;
            self.initialized = true;
            return Ok(());
        }
        let old_center = rect_center(self.canvas)?;
        let new_center = rect_center(canvas)?;
        self.offset = checked_point_add(self.offset, new_center - old_center)?;
        self.canvas = canvas;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.zoom_per_mille = 1_000;
        self.offset = Point::zero();
    }

    pub fn pan(&mut self, delta: Point) -> Result<(), ViewportError> {
        let next = checked_point_add(self.offset, delta)?;
        if next.x.abs() > MAX_OFFSET || next.y.abs() > MAX_OFFSET {
            return Err(ViewportError::CoordinateOutOfBounds);
        }
        self.offset = next;
        Ok(())
    }

    pub fn zoom_by(&mut self, delta: i32, anchor: Point) -> Result<(), ViewportError> {
        let next = self
            .zoom_per_mille
            .checked_add(delta)
            .ok_or(ViewportError::ArithmeticOverflow)?;
        if !(MIN_ZOOM_PER_MILLE..=MAX_ZOOM_PER_MILLE).contains(&next) {
            return Err(ViewportError::ZoomOutOfBounds);
        }
        let world_anchor = self.screen_to_world(anchor)?;
        self.zoom_per_mille = next;
        let moved_anchor = self.world_to_screen(world_anchor)?;
        self.pan(anchor - moved_anchor)
    }

    pub fn center(&mut self, world: Point) -> Result<(), ViewportError> {
        let current = self.world_to_screen(world)?;
        self.pan(rect_center(self.canvas)? - current)
    }

    pub fn fit(&mut self, bounds: WorldBounds) -> Result<(), ViewportError> {
        let content_width = bounds
            .right
            .checked_sub(bounds.left)
            .ok_or(ViewportError::ArithmeticOverflow)?;
        let content_height = bounds
            .bottom
            .checked_sub(bounds.top)
            .ok_or(ViewportError::ArithmeticOverflow)?;
        if content_width <= 0 || content_height <= 0 {
            return Err(ViewportError::EmptyContent);
        }
        let canvas_width = i32::try_from(self.canvas.width)
            .map_err(|_| ViewportError::CoordinateOutOfBounds)?
            .checked_sub(FIT_PADDING * 2)
            .ok_or(ViewportError::ArithmeticOverflow)?;
        let canvas_height = i32::try_from(self.canvas.height)
            .map_err(|_| ViewportError::CoordinateOutOfBounds)?
            .checked_sub(FIT_PADDING * 2)
            .ok_or(ViewportError::ArithmeticOverflow)?;
        if canvas_width <= 0 || canvas_height <= 0 {
            return Err(ViewportError::EmptyCanvas);
        }
        let horizontal = checked_ratio(canvas_width, content_width)?;
        let vertical = checked_ratio(canvas_height, content_height)?;
        self.zoom_per_mille = horizontal
            .min(vertical)
            .clamp(MIN_ZOOM_PER_MILLE, MAX_ZOOM_PER_MILLE);
        self.offset = Point::zero();
        self.center(bounds.center()?)
    }

    pub fn world_to_screen(&self, world: Point) -> Result<Point, ViewportError> {
        validate_world_point(world)?;
        let anchor = Point::new(self.canvas.x, self.canvas.y);
        let relative = checked_point_sub(world, anchor)?;
        let scaled = Point::new(
            scale(relative.x, self.zoom_per_mille)?,
            scale(relative.y, self.zoom_per_mille)?,
        );
        checked_point_add(checked_point_add(anchor, self.offset)?, scaled)
    }

    pub fn screen_to_world(&self, screen: Point) -> Result<Point, ViewportError> {
        let anchor = Point::new(self.canvas.x, self.canvas.y);
        let relative = checked_point_sub(checked_point_sub(screen, anchor)?, self.offset)?;
        let unscaled = Point::new(
            unscale(relative.x, self.zoom_per_mille)?,
            unscale(relative.y, self.zoom_per_mille)?,
        );
        let world = checked_point_add(anchor, unscaled)?;
        validate_world_point(world)?;
        Ok(world)
    }

    pub fn world_rect_to_screen(&self, rect: PixelRect) -> Result<PixelRect, ViewportError> {
        let origin = self.world_to_screen(Point::new(rect.x, rect.y))?;
        let width = scale_unsigned(rect.width, self.zoom_per_mille)?;
        let height = scale_unsigned(rect.height, self.zoom_per_mille)?;
        Ok(PixelRect {
            x: origin.x,
            y: origin.y,
            width,
            height,
        })
    }
}

fn validate_canvas(canvas: PixelRect) -> Result<(), ViewportError> {
    if canvas.width == 0 || canvas.height == 0 {
        return Err(ViewportError::EmptyCanvas);
    }
    let _ = WorldBounds::from_rect(canvas)?;
    Ok(())
}

fn validate_world_point(point: Point) -> Result<(), ViewportError> {
    if point.x.unsigned_abs() > MAX_WORLD_COORDINATE as u32
        || point.y.unsigned_abs() > MAX_WORLD_COORDINATE as u32
    {
        return Err(ViewportError::CoordinateOutOfBounds);
    }
    Ok(())
}

fn rect_center(rect: PixelRect) -> Result<Point, ViewportError> {
    WorldBounds::from_rect(rect)?.center()
}

fn midpoint(start: i32, end: i32) -> Result<i32, ViewportError> {
    let sum = i64::from(start) + i64::from(end);
    i32::try_from(sum / 2).map_err(|_| ViewportError::ArithmeticOverflow)
}

fn checked_ratio(numerator: i32, denominator: i32) -> Result<i32, ViewportError> {
    let ratio = i64::from(numerator)
        .checked_mul(1_000)
        .ok_or(ViewportError::ArithmeticOverflow)?
        / i64::from(denominator);
    i32::try_from(ratio).map_err(|_| ViewportError::ArithmeticOverflow)
}

fn scale(value: i32, zoom: i32) -> Result<i32, ViewportError> {
    let product = i64::from(value)
        .checked_mul(i64::from(zoom))
        .ok_or(ViewportError::ArithmeticOverflow)?;
    let scaled = rounded_division(product, 1_000);
    i32::try_from(scaled).map_err(|_| ViewportError::CoordinateOutOfBounds)
}

fn unscale(value: i32, zoom: i32) -> Result<i32, ViewportError> {
    let product = i64::from(value)
        .checked_mul(1_000)
        .ok_or(ViewportError::ArithmeticOverflow)?;
    let unscaled = rounded_division(product, i64::from(zoom));
    i32::try_from(unscaled).map_err(|_| ViewportError::CoordinateOutOfBounds)
}

fn rounded_division(value: i64, divisor: i64) -> i64 {
    if value >= 0 {
        (value + divisor / 2) / divisor
    } else {
        (value - divisor / 2) / divisor
    }
}

fn scale_unsigned(value: u32, zoom: i32) -> Result<u32, ViewportError> {
    let scaled = u64::from(value)
        .checked_mul(u64::try_from(zoom).map_err(|_| ViewportError::ZoomOutOfBounds)?)
        .ok_or(ViewportError::ArithmeticOverflow)?
        / 1_000;
    u32::try_from(scaled.max(1)).map_err(|_| ViewportError::CoordinateOutOfBounds)
}

fn checked_point_add(left: Point, right: Point) -> Result<Point, ViewportError> {
    Ok(Point::new(
        left.x
            .checked_add(right.x)
            .ok_or(ViewportError::ArithmeticOverflow)?,
        left.y
            .checked_add(right.y)
            .ok_or(ViewportError::ArithmeticOverflow)?,
    ))
}

fn checked_point_sub(left: Point, right: Point) -> Result<Point, ViewportError> {
    Ok(Point::new(
        left.x
            .checked_sub(right.x)
            .ok_or(ViewportError::ArithmeticOverflow)?,
        left.y
            .checked_sub(right.y)
            .ok_or(ViewportError::ArithmeticOverflow)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> CanvasViewport {
        let mut viewport = CanvasViewport::default();
        viewport
            .resize(PixelRect {
                x: 176,
                y: 52,
                width: 640,
                height: 480,
            })
            .unwrap();
        viewport
    }

    #[test]
    fn default_mapping_is_identity_and_pan_zoom_round_trip_is_exact_on_grid() {
        let mut viewport = viewport();
        let point = Point::new(376, 252);
        assert_eq!(viewport.world_to_screen(point).unwrap(), point);
        viewport.pan(Point::new(48, -24)).unwrap();
        viewport
            .zoom_by(ZOOM_STEP_PER_MILLE, Point::new(496, 292))
            .unwrap();
        let grid_point = Point::new(576, 452);
        let screen = viewport.world_to_screen(grid_point).unwrap();
        assert_eq!(viewport.screen_to_world(screen).unwrap(), grid_point);
    }

    #[test]
    fn zoom_and_pan_fail_closed_at_each_finite_bound() {
        let mut viewport = viewport();
        viewport.zoom_per_mille = MAX_ZOOM_PER_MILLE;
        assert_eq!(
            viewport.zoom_by(1, Point::new(200, 100)),
            Err(ViewportError::ZoomOutOfBounds)
        );
        viewport.zoom_per_mille = MIN_ZOOM_PER_MILLE;
        assert_eq!(
            viewport.zoom_by(-1, Point::new(200, 100)),
            Err(ViewportError::ZoomOutOfBounds)
        );
        assert_eq!(
            viewport.pan(Point::new(MAX_OFFSET + 1, 0)),
            Err(ViewportError::CoordinateOutOfBounds)
        );
        assert_eq!(
            viewport.world_to_screen(Point::new(i32::MAX, i32::MAX)),
            Err(ViewportError::CoordinateOutOfBounds)
        );
    }

    #[test]
    fn fit_centers_finite_content_and_resize_preserves_world_center() {
        let mut viewport = viewport();
        let bounds = WorldBounds {
            left: 200,
            top: 100,
            right: 1_000,
            bottom: 700,
        };
        viewport.fit(bounds).unwrap();
        assert_eq!(viewport.zoom_per_mille(), 720);
        let canvas_center = rect_center(viewport.canvas()).unwrap();
        assert_eq!(
            viewport.world_to_screen(bounds.center().unwrap()).unwrap(),
            canvas_center
        );
        let world_center = viewport.screen_to_world(canvas_center).unwrap();
        viewport
            .resize(PixelRect {
                x: 176,
                y: 52,
                width: 800,
                height: 600,
            })
            .unwrap();
        let resized_center = rect_center(viewport.canvas()).unwrap();
        assert_eq!(
            viewport.screen_to_world(resized_center).unwrap(),
            world_center
        );
    }

    #[test]
    fn empty_canvas_and_content_are_distinct_failures() {
        let mut state = CanvasViewport::default();
        assert_eq!(
            state.resize(PixelRect {
                x: 0,
                y: 0,
                width: 0,
                height: 10,
            }),
            Err(ViewportError::EmptyCanvas)
        );
        state = viewport();
        assert_eq!(
            state.fit(WorldBounds {
                left: 4,
                top: 4,
                right: 4,
                bottom: 8,
            }),
            Err(ViewportError::EmptyContent)
        );
    }
}
