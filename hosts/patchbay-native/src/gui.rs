//! Small renderer-local Patchbay composition, geometry, and hit testing.

use crate::{
    canvas::SoftwareCanvas,
    gui_hit::HitShape,
    gui_inspector::draw_inspector,
    gui_primitives::{
        draw_regions, fill_rect, frame_rect, icon_label, layer_label, line, rgb, text, PixelRect,
        RegionMetrics,
    },
    icon::{draw_icon, Icon},
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Primitive},
    primitives::{Circle, PrimitiveStyle},
    Drawable,
};
use patchbay_model::{PatchbayGear, PatchbayGraph, PatchbayTheme, PHOSPHOR_THEME};

pub use crate::gui_hit::{GuiAction, HitTarget};

pub const MAX_HIT_TARGETS: usize = patchbay_model::MAX_PATCHBAY_GEARS
    + patchbay_model::MAX_PATCHBAY_PORTS
    + patchbay_model::MAX_PATCHBAY_CORDS
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

#[derive(Clone)]
struct GearLayout<'a> {
    gear: &'a PatchbayGear,
    bounds: PixelRect,
    inputs: Vec<(String, Point)>,
    outputs: Vec<(String, Point)>,
}

pub fn draw_patchbay(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    graph: &PatchbayGraph,
    selected: Option<&str>,
    lifecycle: &LifecycleContext,
) -> Vec<HitTarget> {
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
    draw_header(&mut canvas, graph, lifecycle, theme);
    let layouts = layout_gears(graph, width);
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
    draw_navigator(&mut canvas, theme, &mut targets);
    draw_cords(&mut canvas, graph, &layouts, selected, theme, &mut targets);
    for layout in &layouts {
        draw_gear(&mut canvas, graph, layout, selected, theme, &mut targets);
    }
    draw_inspector(&mut canvas, graph, selected, width, INSPECTOR_WIDTH, theme);
    draw_footer(&mut canvas, graph, selected, height, theme);
    targets.truncate(MAX_HIT_TARGETS);
    targets
}

fn draw_header<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    lifecycle: &LifecycleContext,
    theme: &PatchbayTheme,
) {
    icon_label(
        target,
        Icon::Form,
        Point::new(14, 10),
        &format!("FORM  {}", graph.form_name),
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
        GuiAction::OpenNextForm,
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

fn layout_gears(graph: &PatchbayGraph, width: i32) -> Vec<GearLayout<'_>> {
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
            let x = canvas_left + column as i32 * (NODE_WIDTH + 64);
            let prior_height = graph
                .gears
                .chunks(columns)
                .take(row)
                .map(|gears| gears.iter().map(gear_height).max().unwrap_or(0) + 36)
                .sum::<i32>();
            let y = HEADER_HEIGHT + 28 + prior_height;
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
            }
        })
        .collect()
}

fn gear_height(gear: &PatchbayGear) -> i32 {
    let port_rows = gear.inputs.len().max(gear.outputs.len()) as i32;
    MINIMUM_NODE_HEIGHT.max(58 + port_rows * 18)
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

fn draw_gear<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layout: &GearLayout<'_>,
    selected: Option<&str>,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let is_selected = selected == Some(layout.gear.identity.as_str());
    fill_rect(target, layout.bounds, theme.surface);
    frame_rect(
        target,
        layout.bounds,
        if is_selected {
            theme.focus
        } else {
            theme.structure_primary
        },
        if is_selected { 2 } else { 1 },
    );
    draw_icon(
        target,
        Icon::Gear,
        Point::new(layout.bounds.x + 10, layout.bounds.y + 9),
        rgb(theme.emphasis),
    );
    text(
        target,
        Point::new(layout.bounds.x + 34, layout.bounds.y + 10),
        layout.gear.gear_id.as_str(),
        theme.text_primary,
    );
    text(
        target,
        Point::new(layout.bounds.x + 12, layout.bounds.y + 29),
        layout.gear.kind_id.as_str(),
        theme.emphasis,
    );
    targets.push(HitTarget {
        action: select_action(graph, &layout.gear.identity),
        shape: HitShape::Rect(layout.bounds),
    });
    for (identity, point) in layout.inputs.iter().chain(&layout.outputs) {
        let selected_port = selected == Some(identity.as_str());
        let _ = Circle::with_center(*point, 9)
            .into_styled(PrimitiveStyle::with_fill(rgb(if selected_port {
                theme.focus
            } else {
                theme.structure_primary
            })))
            .draw(target);
        let port = layout
            .gear
            .inputs
            .iter()
            .chain(&layout.gear.outputs)
            .find(|port| port.identity == *identity)
            .expect("layout Ports come from the Gear");
        let label_x = if port.descriptor.direction == conduit_core::PortDirection::Input {
            point.x + 12
        } else {
            point.x - 72
        };
        text(
            target,
            Point::new(label_x, point.y - 7),
            port.descriptor.port_id.as_str(),
            theme.text_secondary,
        );
        targets.push(HitTarget {
            action: select_action(graph, identity),
            shape: HitShape::Rect(PixelRect {
                x: point.x - 10,
                y: point.y - 10,
                width: 20,
                height: 20,
            }),
        });
    }
}

fn draw_cords<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layouts: &[GearLayout<'_>],
    selected: Option<&str>,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    for cord in &graph.cords {
        let Some(source) = find_port(layouts, &cord.source_port) else {
            continue;
        };
        let Some(sink) = find_port(layouts, &cord.sink_port) else {
            continue;
        };
        let middle_x = source.x + (sink.x - source.x) / 2;
        let color = if selected == Some(cord.identity.as_str()) {
            theme.focus
        } else {
            theme.structure_primary
        };
        line(target, source, Point::new(middle_x, source.y), color);
        line(
            target,
            Point::new(middle_x, source.y),
            Point::new(middle_x, sink.y),
            color,
        );
        line(target, Point::new(middle_x, sink.y), sink, color);
        targets.push(HitTarget {
            action: select_action(graph, &cord.identity),
            shape: HitShape::Cord {
                source,
                middle_x,
                sink,
            },
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

fn find_port(layouts: &[GearLayout<'_>], identity: &str) -> Option<Point> {
    layouts
        .iter()
        .flat_map(|layout| layout.inputs.iter().chain(&layout.outputs))
        .find_map(|(candidate, point)| (candidate == identity).then_some(*point))
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
