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
    RemoveCord(patchbay_model::PatchbaySubjectRef),
    ConnectPorts {
        source: patchbay_model::PatchbaySubjectRef,
        sink: patchbay_model::PatchbaySubjectRef,
    },
    RerouteCord {
        cord: PatchbaySubjectRef,
        sink: PatchbaySubjectRef,
    },
    ConfigureGear {
        subject: PatchbaySubjectRef,
        key: String,
        value: conduit_core::ConfigurationValue,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HitShape {
    Rect(PixelRect),
    Cord { points: [Point; 5] },
}

impl HitTarget {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        match self.shape {
            HitShape::Rect(bounds) => bounds.contains(x, y),
            HitShape::Cord { points } => points
                .windows(2)
                .any(|segment| near_segment(x, y, segment[0], segment[1])),
        }
    }
}

fn near_segment(x: f64, y: f64, first: Point, second: Point) -> bool {
    if first.y == second.y {
        near_horizontal(x, y, first.x, second.x, first.y)
    } else if first.x == second.x {
        near_vertical(x, y, first.x, first.y, second.y)
    } else {
        false
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
