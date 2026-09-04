//! Viewport-specific GUI geometry, transforms, and visible controls.

use crate::{
    canvas_viewport::{CanvasViewport, WorldBounds},
    gui::{
        layout_boundaries, BoundaryLayout, GearLayout, GuiAction, HitTarget, FOOTER_HEIGHT,
        HEADER_HEIGHT, INSPECTOR_WIDTH, MINIMUM_NODE_HEIGHT, NAV_WIDTH, NODE_WIDTH,
    },
    gui_composition::{layout_compositions, CompositionLayout},
    gui_gear_layout::{layout_gears, GearGeometry},
    gui_hit::{HitShape, ViewportAction},
    gui_primitives::{frame_rect, text, PixelRect},
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{ApplicationTheme, PatchbayGraph};

#[cfg(test)]
pub(super) fn canvas_rect(width: u32, height: u32) -> PixelRect {
    canvas_rect_for(width, height, true)
}

pub(super) fn canvas_rect_for(width: u32, height: u32, inspector_requested: bool) -> PixelRect {
    if let (Ok(width16), Ok(height16)) = (u16::try_from(width), u16::try_from(height)) {
        if let Ok(layout) = patchbay_model::ResponsivePatchbayLayout::allocate(
            width16,
            height16,
            100,
            inspector_requested,
        ) {
            if let Some(region) = layout.region(patchbay_model::PresentationRegionId::Canvas) {
                return PixelRect {
                    x: i32::from(region.bounds.x),
                    y: i32::from(region.bounds.y),
                    width: u32::from(region.bounds.width),
                    height: u32::from(region.bounds.height),
                };
            }
        }
    }
    let width = i32::try_from(width).unwrap_or(i32::MAX);
    let height = i32::try_from(height).unwrap_or(i32::MAX);
    let right = (width - INSPECTOR_WIDTH).max(NAV_WIDTH + 1);
    let bottom = (height - FOOTER_HEIGHT).max(HEADER_HEIGHT + 1);
    PixelRect {
        x: NAV_WIDTH,
        y: HEADER_HEIGHT,
        width: u32::try_from(right - NAV_WIDTH).unwrap_or(1),
        height: u32::try_from(bottom - HEADER_HEIGHT).unwrap_or(1),
    }
}

pub(super) fn canvas_world_bounds(
    graph: &PatchbayGraph,
    width: i32,
    presentation_layout: &patchbay_model::PatchbayLayout,
) -> Option<WorldBounds> {
    let gears = layout_gears(graph, width, presentation_layout, gear_geometry());
    let compositions = layout_compositions(graph, width);
    let boundaries = layout_boundaries(graph, width);
    gears
        .iter()
        .map(|layout| layout.bounds)
        .chain(compositions.iter().map(|layout| layout.bounds))
        .chain(boundaries.iter().map(|layout| layout.bounds))
        .filter_map(|rect| WorldBounds::from_rect(rect).ok())
        .reduce(|mut bounds, next| {
            bounds.include(next);
            bounds
        })
}

pub(super) fn subject_world_center(
    graph: &PatchbayGraph,
    width: i32,
    presentation_layout: &patchbay_model::PatchbayLayout,
    identity: &str,
) -> Option<Point> {
    let gears = layout_gears(graph, width, presentation_layout, gear_geometry());
    let compositions = layout_compositions(graph, width);
    let boundaries = layout_boundaries(graph, width);
    gears
        .iter()
        .find(|layout| layout.gear.identity == identity)
        .and_then(|layout| WorldBounds::from_rect(layout.bounds).ok())
        .and_then(|bounds| bounds.center().ok())
        .or_else(|| {
            compositions
                .iter()
                .find(|layout| layout.composition.identity == identity)
                .and_then(|layout| WorldBounds::from_rect(layout.bounds).ok())
                .and_then(|bounds| bounds.center().ok())
        })
        .or_else(|| canvas_port_point(&gears, &compositions, &boundaries, identity))
        .or_else(|| {
            boundaries
                .iter()
                .find(|boundary| boundary.identity == identity)
                .map(|boundary| boundary.point)
        })
        .or_else(|| {
            graph
                .cords
                .iter()
                .find(|cord| cord.identity == identity)
                .and_then(|cord| {
                    let source =
                        canvas_port_point(&gears, &compositions, &boundaries, &cord.source_port)?;
                    let sink =
                        canvas_port_point(&gears, &compositions, &boundaries, &cord.sink_port)?;
                    Some(Point::new(
                        i32::try_from((i64::from(source.x) + i64::from(sink.x)) / 2).ok()?,
                        i32::try_from((i64::from(source.y) + i64::from(sink.y)) / 2).ok()?,
                    ))
                })
        })
}

pub(super) fn transform_canvas_layout(
    viewport: &CanvasViewport,
    gears: &mut Vec<GearLayout<'_>>,
    compositions: &mut Vec<CompositionLayout<'_>>,
    boundaries: &mut Vec<BoundaryLayout>,
) {
    gears.retain_mut(|layout| {
        let Ok(bounds) = viewport.world_rect_to_screen(layout.bounds) else {
            return false;
        };
        let Some(inputs) = transform_ports(viewport, &layout.inputs) else {
            return false;
        };
        let Some(outputs) = transform_ports(viewport, &layout.outputs) else {
            return false;
        };
        layout.bounds = bounds;
        layout.inputs = inputs;
        layout.outputs = outputs;
        true
    });
    compositions.retain_mut(|layout| {
        let Ok(bounds) = viewport.world_rect_to_screen(layout.bounds) else {
            return false;
        };
        let Some(inputs) = transform_ports(viewport, &layout.inputs) else {
            return false;
        };
        let Some(outputs) = transform_ports(viewport, &layout.outputs) else {
            return false;
        };
        layout.bounds = bounds;
        layout.inputs = inputs;
        layout.outputs = outputs;
        true
    });
    boundaries.retain_mut(|boundary| {
        let Ok(bounds) = viewport.world_rect_to_screen(boundary.bounds) else {
            return false;
        };
        let Ok(point) = viewport.world_to_screen(boundary.point) else {
            return false;
        };
        boundary.bounds = bounds;
        boundary.point = point;
        true
    });
}

pub(super) fn draw_viewport_controls<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    viewport: &CanvasViewport,
    origin: Point,
    theme: &ApplicationTheme,
    targets: &mut Vec<HitTarget>,
) {
    text(
        target,
        origin,
        &format!(
            "VIEW {}%  PAN {},{}",
            viewport.zoom_per_mille() / 10,
            viewport.offset().x,
            viewport.offset().y
        ),
        theme.text_secondary,
    );
    let controls = [
        ("-", ViewportAction::ZoomOut, 0, 28),
        ("+", ViewportAction::ZoomIn, 34, 28),
        ("FIT", ViewportAction::Fit, 68, 40),
        ("CENTER", ViewportAction::CenterSelection, 112, 66),
        ("RESET", ViewportAction::Reset, 182, 58),
    ];
    for (label, action, x, width) in controls {
        let bounds = PixelRect {
            x: origin.x + x,
            y: origin.y + 16,
            width,
            height: 20,
        };
        frame_rect(target, bounds, theme.structure_secondary, 1);
        text(
            target,
            Point::new(bounds.x + 5, bounds.y + 4),
            label,
            theme.text_primary,
        );
        targets.push(HitTarget {
            action: GuiAction::Viewport(action),
            shape: HitShape::Rect(bounds),
        });
    }
}

fn transform_ports(
    viewport: &CanvasViewport,
    ports: &[(String, Point)],
) -> Option<Vec<(String, Point)>> {
    ports
        .iter()
        .map(|(identity, point)| {
            viewport
                .world_to_screen(*point)
                .ok()
                .map(|point| (identity.clone(), point))
        })
        .collect()
}

fn gear_geometry() -> GearGeometry {
    GearGeometry {
        canvas_left: NAV_WIDTH + 28,
        inspector_width: INSPECTOR_WIDTH,
        header_height: HEADER_HEIGHT,
        node_width: NODE_WIDTH,
        minimum_node_height: MINIMUM_NODE_HEIGHT,
    }
}

fn canvas_port_point(
    gears: &[GearLayout<'_>],
    compositions: &[CompositionLayout<'_>],
    boundaries: &[BoundaryLayout],
    identity: &str,
) -> Option<Point> {
    gears
        .iter()
        .flat_map(|layout| layout.inputs.iter().chain(&layout.outputs))
        .find_map(|(candidate, point)| (candidate == identity).then_some(*point))
        .or_else(|| {
            compositions
                .iter()
                .flat_map(|layout| layout.inputs.iter().chain(&layout.outputs))
                .find_map(|(candidate, point)| (candidate == identity).then_some(*point))
        })
        .or_else(|| {
            boundaries
                .iter()
                .find_map(|boundary| (boundary.identity == identity).then_some(boundary.point))
        })
}
