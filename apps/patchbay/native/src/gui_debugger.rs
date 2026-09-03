//! Native drawing for renderer-neutral live debugger annotations.

use crate::{
    canvas_viewport::CanvasViewport,
    gui::{cord_route_points, find_port, BoundaryLayout, GearLayout},
    gui_composition::CompositionLayout,
    gui_primitives::{frame_rect, line, text, PixelRect},
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{
    ApplicationTheme, DebuggerActivityPhase, DebuggerPresentation, DebuggerSubjectActivity,
    PatchbayGraph,
};

pub(super) fn draw_debugger_overlay<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layout: (
        &[GearLayout<'_>],
        &[CompositionLayout<'_>],
        &[BoundaryLayout],
    ),
    presentation: (&patchbay_model::PatchbayLayout, &CanvasViewport),
    debugger: &DebuggerPresentation,
    theme: &ApplicationTheme,
) {
    let (layouts, compositions, boundaries) = layout;
    for activity in &debugger.activities {
        let color = match activity.phase {
            DebuggerActivityPhase::Faulted => theme.failure,
            DebuggerActivityPhase::Recent => theme.structure_secondary,
            DebuggerActivityPhase::Active => theme.focus,
            DebuggerActivityPhase::Inactive => continue,
        };
        let label = activity_label(activity);
        if let Some(layout) = layouts
            .iter()
            .find(|layout| layout.gear.identity == activity.subject)
        {
            frame_rect(target, layout.bounds, color, 2);
            text(
                target,
                Point::new(layout.bounds.x + 8, layout.bounds.y + 8),
                &label,
                color,
            );
            continue;
        }
        if let Some(point) = find_port(layouts, compositions, boundaries, &activity.subject) {
            frame_rect(
                target,
                PixelRect {
                    x: point.x - 5,
                    y: point.y - 5,
                    width: 11,
                    height: 11,
                },
                color,
                2,
            );
            text(target, Point::new(point.x + 8, point.y - 4), &label, color);
            continue;
        }
        let Some(cord) = graph
            .cords
            .iter()
            .find(|cord| cord.identity == activity.subject)
        else {
            continue;
        };
        let Some(points) = cord_route_points(cord, layout, presentation) else {
            continue;
        };
        for segment in points.windows(2) {
            line(target, segment[0], segment[1], color);
        }
        text(
            target,
            Point::new(points[2].x + 5, points[2].y - 8),
            &label,
            color,
        );
    }
}

fn activity_label(activity: &DebuggerSubjectActivity) -> String {
    let core = if activity.phase == DebuggerActivityPhase::Faulted {
        format!(
            "fault {}",
            activity
                .retained_fault_code
                .map_or_else(|| "unknown".to_owned(), |code| code.to_string())
        )
    } else {
        activity.latest_value.as_ref().map_or_else(
            || format!("{}  {}", activity.latest_kind, activity.observed_count),
            |value| {
                format!(
                    "{}  {}  {}",
                    activity.latest_kind, value.summary, activity.observed_count
                )
            },
        )
    };
    activity.line_subject.as_ref().map_or(core.clone(), |line| {
        format!("{core}  h{} {line}", activity.host)
    })
}
