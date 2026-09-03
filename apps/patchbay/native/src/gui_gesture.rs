//! Visible bounded manifestation of renderer-local drag state.

use crate::{
    gui::{BoundaryLayout, GearLayout},
    gui_composition::CompositionLayout,
    gui_primitives::{frame_rect, line, text, PixelRect},
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{ApplicationTheme, PatchbayGraph, PatchbayPortCompatibility};

#[derive(Default)]
pub struct GestureView<'a> {
    pub palette_kind: Option<&'a str>,
    pub cord_source: Option<&'a str>,
    pub cord_route: Option<&'a str>,
    pub gear: Option<&'a str>,
    pub cursor: Point,
}

pub(super) fn draw_gesture<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    gears: &[GearLayout<'_>],
    compositions: &[CompositionLayout<'_>],
    boundaries: &[BoundaryLayout],
    gesture: &GestureView<'_>,
    theme: &ApplicationTheme,
) {
    if let Some(kind) = gesture.palette_kind {
        let bounds = ghost_bounds(gesture.cursor);
        frame_rect(target, bounds, theme.focus, 2);
        text(
            target,
            Point::new(bounds.x + 8, bounds.y + 12),
            &format!("ADD {kind}"),
            theme.focus,
        );
        text(
            target,
            Point::new(bounds.x + 8, bounds.y + 30),
            "RELEASE TO PLACE",
            theme.text_primary,
        );
    }
    if let Some(identity) = gesture.gear {
        let bounds = ghost_bounds(gesture.cursor);
        frame_rect(target, bounds, theme.emphasis, 2);
        text(
            target,
            Point::new(bounds.x + 8, bounds.y + 12),
            &format!("MOVE {identity}"),
            theme.emphasis,
        );
    }
    if let Some(source) = gesture.cord_source {
        if let Some(start) = port_point(gears, compositions, boundaries, source) {
            line(target, start, gesture.cursor, theme.focus);
            text(
                target,
                Point::new(gesture.cursor.x + 8, gesture.cursor.y - 8),
                "CONNECT",
                theme.focus,
            );
            let candidates = graph.connection_candidates(source);
            for (identity, point) in sink_points(gears, compositions, boundaries) {
                let Some(candidate) = candidates
                    .iter()
                    .find(|candidate| candidate.sink_identity == identity)
                else {
                    continue;
                };
                let compatibility = &candidate.compatibility;
                let (label, color) = match compatibility {
                    PatchbayPortCompatibility::Compatible => ("OK", theme.success),
                    PatchbayPortCompatibility::DuplicateCord => ("USED", theme.emphasis),
                    PatchbayPortCompatibility::IncompatibleInfo { .. }
                    | PatchbayPortCompatibility::IncompatibleTemporal { .. }
                    | PatchbayPortCompatibility::InvalidDirection
                    | PatchbayPortCompatibility::UnknownPort => ("X", theme.failure),
                };
                frame_rect(
                    target,
                    PixelRect {
                        x: point.x - 8,
                        y: point.y - 8,
                        width: 16,
                        height: 16,
                    },
                    color,
                    1,
                );
                text(target, Point::new(point.x + 10, point.y - 3), label, color);
            }
        }
    }
    if let Some(cord_identity) = gesture.cord_route {
        if let Some(cord) = graph
            .cords
            .iter()
            .find(|cord| cord.identity == cord_identity)
        {
            if let (Some(source), Some(sink)) = (
                port_point(gears, compositions, boundaries, &cord.source_port),
                port_point(gears, compositions, boundaries, &cord.sink_port),
            ) {
                line(target, source, gesture.cursor, theme.emphasis);
                line(target, gesture.cursor, sink, theme.emphasis);
                text(
                    target,
                    Point::new(gesture.cursor.x + 8, gesture.cursor.y - 8),
                    "ROUTE — DROP ON PORT TO REROUTE",
                    theme.emphasis,
                );
            }
        }
    }
}

fn ghost_bounds(cursor: Point) -> PixelRect {
    PixelRect {
        x: cursor.x - 88,
        y: cursor.y - 36,
        width: 176,
        height: 72,
    }
}

fn port_point(
    gears: &[GearLayout<'_>],
    compositions: &[CompositionLayout<'_>],
    boundaries: &[BoundaryLayout],
    identity: &str,
) -> Option<Point> {
    gears
        .iter()
        .flat_map(|layout| layout.inputs.iter().chain(&layout.outputs))
        .find_map(|(candidate, point)| (candidate == identity).then_some(*point))
        .or_else(|| crate::gui_composition::composition_port_point(compositions, identity))
        .or_else(|| {
            boundaries
                .iter()
                .find_map(|boundary| (boundary.identity == identity).then_some(boundary.point))
        })
}

fn sink_points<'a>(
    gears: &'a [GearLayout<'_>],
    compositions: &'a [CompositionLayout<'_>],
    boundaries: &'a [BoundaryLayout],
) -> impl Iterator<Item = (&'a str, Point)> {
    gears
        .iter()
        .flat_map(|layout| layout.inputs.iter())
        .chain(compositions.iter().flat_map(|layout| layout.inputs.iter()))
        .map(|(identity, point)| (identity.as_str(), *point))
        .chain(
            boundaries
                .iter()
                .filter(|boundary| boundary.is_output)
                .map(|boundary| (boundary.identity.as_str(), boundary.point)),
        )
}
