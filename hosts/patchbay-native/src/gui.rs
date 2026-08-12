//! Small renderer-local Patchbay composition, geometry, and hit testing.

use crate::{
    canvas::SoftwareCanvas,
    gui_gear::{draw_gear, GearViewContext},
    gui_hit::HitShape,
    gui_inspector::draw_inspector,
    gui_primitives::{
        draw_regions, frame_rect, icon_label, layer_label, line, text, PixelRect, RegionMetrics,
    },
    icon::Icon,
    palette_view::draw_palette,
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{PatchbayGear, PatchbayGraph, PatchbayTheme, PHOSPHOR_THEME};

pub use crate::gui_hit::{GuiAction, HitTarget};

pub const MAX_HIT_TARGETS: usize = patchbay_model::MAX_PATCHBAY_GEARS
    + patchbay_model::MAX_PATCHBAY_GEARS
    + patchbay_model::MAX_PATCHBAY_GEARS
    + patchbay_model::MAX_PATCHBAY_PORTS
    + patchbay_model::MAX_PATCHBAY_CORDS
    + patchbay_model::MAX_PALETTE_ENTRIES
    + patchbay_model::MAX_PATCHBAY_GEARS * patchbay_model::MAX_FACE_CONTROLS * 2
    + 3;

const HEADER_HEIGHT: i32 = 52;
const FOOTER_HEIGHT: i32 = 42;
const NAV_WIDTH: i32 = 176;
const INSPECTOR_WIDTH: i32 = 284;
const NODE_WIDTH: i32 = 190;
const MINIMUM_NODE_HEIGHT: i32 = 92;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecycleContext {
    pub body_id: Option<String>,
    pub wake_id: Option<String>,
    pub plan_id: Option<String>,
    pub play_id: Option<String>,
}

pub struct PatchbayViewContext<'a> {
    pub selected: Option<&'a str>,
    pub breadcrumb: &'a str,
    pub lifecycle: &'a LifecycleContext,
    pub palette_query: &'a str,
    pub presentation_layout: &'a patchbay_model::PatchbayLayout,
    pub realization_plan: Option<&'a conduit_core::Plan>,
    pub realization_hosts: &'a [conduit_core::HostAdvertisement],
}

#[derive(Clone)]
pub(super) struct GearLayout<'a> {
    pub(super) gear: &'a PatchbayGear,
    pub(super) bounds: PixelRect,
    pub(super) inputs: Vec<(String, Point)>,
    pub(super) outputs: Vec<(String, Point)>,
    pub(super) group: Option<String>,
}

#[derive(Clone)]
struct BoundaryLayout {
    identity: String,
    point: Point,
    bounds: PixelRect,
    label: String,
}

pub fn draw_patchbay(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    graph: &PatchbayGraph,
    view: PatchbayViewContext<'_>,
) -> Vec<HitTarget> {
    let PatchbayViewContext {
        selected,
        breadcrumb,
        lifecycle,
        palette_query,
        presentation_layout,
        realization_plan,
        realization_hosts,
    } = view;
    debug_assert!(Icon::ALL
        .iter()
        .all(|icon| !icon.accessibility_name().is_empty()));
    let mut canvas = SoftwareCanvas::new(pixels, width, height);
    let theme = &PHOSPHOR_THEME;
    let width = i32::try_from(width).unwrap_or(i32::MAX);
    let height = i32::try_from(height).unwrap_or(i32::MAX);
    draw_regions(
        &mut canvas,
        width,
        height,
        RegionMetrics {
            header_height: HEADER_HEIGHT,
            footer_height: FOOTER_HEIGHT,
            nav_width: NAV_WIDTH,
            inspector_width: INSPECTOR_WIDTH,
        },
        theme,
    );
    draw_header(&mut canvas, graph, breadcrumb, lifecycle, theme);
    let layouts = layout_gears(graph, width, presentation_layout);
    let boundaries = layout_boundaries(graph, width);
    let mut targets = Vec::with_capacity(
        MAX_HIT_TARGETS.min(
            3 + graph.gears.len()
                + graph.cords.len()
                + graph
                    .gears
                    .iter()
                    .map(|gear| gear.inputs.len() + gear.outputs.len())
                    .sum::<usize>(),
        ),
    );
    draw_navigator(&mut canvas, palette_query, theme, &mut targets);
    draw_cords(
        &mut canvas,
        graph,
        (&layouts, &boundaries),
        selected,
        presentation_layout,
        theme,
        &mut targets,
    );
    draw_boundaries(&mut canvas, graph, &boundaries, theme, &mut targets);
    let gear_view = GearViewContext {
        presentation_layout,
        realization_plan,
        realization_hosts,
    };
    for layout in &layouts {
        draw_gear(
            &mut canvas,
            graph,
            layout,
            selected,
            &gear_view,
            theme,
            &mut targets,
        );
    }
    draw_inspector(&mut canvas, graph, selected, width, INSPECTOR_WIDTH, theme);
    draw_footer(&mut canvas, graph, selected, height, theme);
    targets.truncate(MAX_HIT_TARGETS);
    targets
}

fn draw_header<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    breadcrumb: &str,
    lifecycle: &LifecycleContext,
    theme: &PatchbayTheme,
) {
    icon_label(
        target,
        Icon::Form,
        Point::new(14, 10),
        &format!(
            "FORM  {}",
            if breadcrumb.is_empty() {
                graph.form_name.as_str()
            } else {
                breadcrumb
            }
        ),
        theme.emphasis,
    );
    let layers = [
        (
            Icon::Body,
            layer_label("BODY", &lifecycle.body_id, "BUILD", "BORN"),
        ),
        (
            Icon::Wake,
            layer_label("WAKE", &lifecycle.wake_id, "LULLED", "AWAKE"),
        ),
        (
            Icon::Plan,
            layer_label("PLAN", &lifecycle.plan_id, "NONE", "READY"),
        ),
        (
            Icon::Play,
            layer_label("PLAY", &lifecycle.play_id, "NONE", "ACTIVE"),
        ),
    ];
    for (index, (icon, label)) in layers.into_iter().enumerate() {
        icon_label(
            target,
            icon,
            Point::new(260 + index as i32 * 112, 10),
            &label,
            theme.text_secondary,
        );
    }
}

fn draw_navigator<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    palette_query: &str,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    text(target, Point::new(14, 66), "NAVIGATOR", theme.emphasis);
    for (index, (icon, label)) in [
        (Icon::Form, "Forms"),
        (Icon::Body, "Bodies"),
        (Icon::Host, "Hosts"),
        (Icon::Sign, "Signs"),
    ]
    .into_iter()
    .enumerate()
    {
        icon_label(
            target,
            icon,
            Point::new(14, 92 + index as i32 * 30),
            label,
            theme.text_primary,
        );
    }
    text(target, Point::new(14, 226), "ACTIONS", theme.emphasis);
    action_button(
        target,
        Icon::Open,
        "Open Back",
        246,
        GuiAction::OpenBack,
        theme,
        targets,
    );
    action_button(
        target,
        Icon::Save,
        "Save",
        278,
        GuiAction::SaveForm,
        theme,
        targets,
    );
    action_button(
        target,
        Icon::Inspect,
        "Linear (F2)",
        310,
        GuiAction::ToggleLinearView,
        theme,
        targets,
    );
    text(
        target,
        Point::new(14, 354),
        &format!("PALETTE /{}", palette_query),
        theme.emphasis,
    );
    draw_palette(target, palette_query, 374, theme, targets);
}

fn action_button<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    icon: Icon,
    label: &str,
    y: i32,
    action: GuiAction,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let bounds = PixelRect {
        x: 12,
        y,
        width: 150,
        height: 26,
    };
    frame_rect(target, bounds, theme.structure_secondary, 1);
    icon_label(
        target,
        icon,
        Point::new(18, y + 5),
        label,
        theme.text_primary,
    );
    targets.push(HitTarget {
        action,
        shape: HitShape::Rect(bounds),
    });
}

fn layout_gears<'a>(
    graph: &'a PatchbayGraph,
    width: i32,
    presentation_layout: &patchbay_model::PatchbayLayout,
) -> Vec<GearLayout<'a>> {
    let canvas_left = NAV_WIDTH + 28;
    let canvas_right = (width - INSPECTOR_WIDTH - 28).max(canvas_left + NODE_WIDTH);
    let columns = ((canvas_right - canvas_left) / (NODE_WIDTH + 64)).max(1) as usize;
    graph
        .gears
        .iter()
        .enumerate()
        .map(|(index, gear)| {
            let column = index % columns;
            let row = index / columns;
            let default_x = canvas_left + column as i32 * (NODE_WIDTH + 64);
            let prior_height = graph
                .gears
                .chunks(columns)
                .take(row)
                .map(|gears| gears.iter().map(gear_height).max().unwrap_or(0) + 36)
                .sum::<i32>();
            let default_y = HEADER_HEIGHT + 28 + prior_height;
            let (x, y) = presentation_layout
                .position(&gear.identity)
                .unwrap_or((default_x, default_y));
            let node_height = gear_height(gear);
            let bounds = PixelRect {
                x,
                y,
                width: NODE_WIDTH as u32,
                height: node_height as u32,
            };
            GearLayout {
                gear,
                bounds,
                inputs: port_points(&gear.inputs, x, y),
                outputs: port_points(&gear.outputs, x + NODE_WIDTH, y),
                group: presentation_layout
                    .gears
                    .iter()
                    .find(|placement| placement.gear_identity == gear.identity)
                    .and_then(|placement| placement.group.clone()),
            }
        })
        .collect()
}

fn gear_height(gear: &PatchbayGear) -> i32 {
    let port_rows = gear.inputs.len().max(gear.outputs.len()) as i32;
    MINIMUM_NODE_HEIGHT.max(58 + port_rows * 18 + gear.controls.len() as i32 * 22)
}

fn port_points(ports: &[patchbay_model::PatchbayPort], x: i32, y: i32) -> Vec<(String, Point)> {
    ports
        .iter()
        .enumerate()
        .map(|(index, port)| {
            let offset = 48 + index as i32 * 18;
            (port.identity.clone(), Point::new(x, y + offset))
        })
        .collect()
}

fn draw_cords<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layout: (&[GearLayout<'_>], &[BoundaryLayout]),
    selected: Option<&str>,
    presentation_layout: &patchbay_model::PatchbayLayout,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let (layouts, boundaries) = layout;
    for cord in &graph.cords {
        let Some(source) = find_port(layouts, boundaries, &cord.source_port) else {
            continue;
        };
        let Some(sink) = find_port(layouts, boundaries, &cord.sink_port) else {
            continue;
        };
        let default_x = source.x + (sink.x - source.x) / 2;
        let (bend_x, bend_y) = presentation_layout
            .cord_route(&cord.source_port, &cord.sink_port)
            .unwrap_or((default_x, source.y + (sink.y - source.y) / 2));
        let points = [
            source,
            Point::new(bend_x, source.y),
            Point::new(bend_x, bend_y),
            Point::new(sink.x, bend_y),
            sink,
        ];
        let color = if selected == Some(cord.identity.as_str()) {
            theme.focus
        } else {
            theme.structure_primary
        };
        for segment in points.windows(2) {
            line(target, segment[0], segment[1], color);
        }
        targets.push(HitTarget {
            action: select_action(graph, &cord.identity),
            shape: HitShape::Cord { points },
        });
    }
}

fn select_action(graph: &PatchbayGraph, identity: &str) -> GuiAction {
    GuiAction::SelectSubject(
        graph
            .subject_ref(identity)
            .expect("drawn subject belongs to the exact graph"),
    )
}

fn find_port(
    layouts: &[GearLayout<'_>],
    boundaries: &[BoundaryLayout],
    identity: &str,
) -> Option<Point> {
    layouts
        .iter()
        .flat_map(|layout| layout.inputs.iter().chain(&layout.outputs))
        .find_map(|(candidate, point)| (candidate == identity).then_some(*point))
        .or_else(|| {
            boundaries
                .iter()
                .find_map(|boundary| (boundary.identity == identity).then_some(boundary.point))
        })
}

fn layout_boundaries(graph: &PatchbayGraph, width: i32) -> Vec<BoundaryLayout> {
    let left = NAV_WIDTH + 8;
    let right = (width - INSPECTOR_WIDTH - 8).max(left + 80);
    graph
        .face_inputs
        .iter()
        .enumerate()
        .map(|(index, port)| {
            let y = HEADER_HEIGHT + 42 + index as i32 * 34;
            BoundaryLayout {
                identity: port.identity.clone(),
                point: Point::new(left + 10, y + 10),
                bounds: PixelRect {
                    x: left,
                    y,
                    width: 82,
                    height: 22,
                },
                label: format!("> {}", port.descriptor.port_id.as_str()),
            }
        })
        .chain(graph.face_outputs.iter().enumerate().map(|(index, port)| {
            let y = HEADER_HEIGHT + 42 + index as i32 * 34;
            BoundaryLayout {
                identity: port.identity.clone(),
                point: Point::new(right - 10, y + 10),
                bounds: PixelRect {
                    x: right - 82,
                    y,
                    width: 82,
                    height: 22,
                },
                label: format!("{} >", port.descriptor.port_id.as_str()),
            }
        }))
        .collect()
}

fn draw_boundaries<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    boundaries: &[BoundaryLayout],
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    for boundary in boundaries {
        frame_rect(target, boundary.bounds, theme.focus, 1);
        text(
            target,
            Point::new(boundary.bounds.x + 7, boundary.bounds.y + 7),
            &boundary.label,
            theme.text_primary,
        );
        targets.push(HitTarget {
            action: select_action(graph, &boundary.identity),
            shape: HitShape::Rect(boundary.bounds),
        });
    }
}

fn draw_footer<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    selected: Option<&str>,
    height: i32,
    theme: &PatchbayTheme,
) {
    let y = height - FOOTER_HEIGHT + 12;
    icon_label(
        target,
        Icon::Success,
        Point::new(14, y - 4),
        "CHECKED",
        theme.success,
    );
    text(
        target,
        Point::new(132, y),
        &format!(
            "Gears {}  Ports {}  Cords {}",
            graph.gears.len(),
            graph
                .gears
                .iter()
                .map(|g| g.inputs.len() + g.outputs.len())
                .sum::<usize>(),
            graph.cords.len()
        ),
        theme.text_primary,
    );
    if selected.is_some() {
        text(
            target,
            Point::new(430, y),
            "selection is presentation-only",
            theme.text_secondary,
        );
    }
}
