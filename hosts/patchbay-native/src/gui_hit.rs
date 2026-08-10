//! Finite renderer-local hit geometry and pre-admission action candidates.

use crate::gui_primitives::PixelRect;
use embedded_graphics::prelude::Point;
use patchbay_model::PatchbaySubjectRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitTarget {
    pub action: GuiAction,
    pub(super) shape: HitShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiAction {
    SelectSubject(PatchbaySubjectRef),
    OpenNextForm,
    SaveForm,
    ToggleLinearView,
    PlacePaletteKind(String),
    DuplicateGear(patchbay_model::PatchbaySubjectRef),
    RemoveGear(patchbay_model::PatchbaySubjectRef),
    ConnectPorts {
        source: patchbay_model::PatchbaySubjectRef,
        sink: patchbay_model::PatchbaySubjectRef,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HitShape {
    Rect(PixelRect),
    Cord {
        source: Point,
        middle_x: i32,
        sink: Point,
    },
}

impl HitTarget {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        match self.shape {
            HitShape::Rect(bounds) => bounds.contains(x, y),
            HitShape::Cord {
                source,
                middle_x,
                sink,
            } => {
                near_horizontal(x, y, source.x, middle_x, source.y)
                    || near_vertical(x, y, middle_x, source.y, sink.y)
                    || near_horizontal(x, y, middle_x, sink.x, sink.y)
            }
        }
    }
}

fn near_horizontal(x: f64, y: f64, x1: i32, x2: i32, line_y: i32) -> bool {
    const TOLERANCE: f64 = 5.0;
    x >= f64::from(x1.min(x2)) - TOLERANCE
        && x <= f64::from(x1.max(x2)) + TOLERANCE
        && (y - f64::from(line_y)).abs() <= TOLERANCE
}

fn near_vertical(x: f64, y: f64, line_x: i32, y1: i32, y2: i32) -> bool {
    const TOLERANCE: f64 = 5.0;
    y >= f64::from(y1.min(y2)) - TOLERANCE
        && y <= f64::from(y1.max(y2)) + TOLERANCE
        && (x - f64::from(line_x)).abs() <= TOLERANCE
}
