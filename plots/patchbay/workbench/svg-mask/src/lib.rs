//! Lightweight SVG and Mermaid realizations of the resident Patchbay workbench Mask.
//!
//! The checked Plot owns the Gears, Ports, Cords, and boundary bindings. This
//! crate owns only their bounded static spatial manifestation; it neither plans
//! nor executes the Plot and it introduces no alternate graph truth.
#![no_std]

extern crate alloc;

#[cfg(test)]
mod tests;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::ConnectionTrack;
use conduit_plot::ExpandedAuthoringPlot;
use core::fmt::Write as _;

const BOX_WIDTH: i32 = 280;
const HEADER_HEIGHT: i32 = 52;
const PORT_ROW: i32 = 42;
const MIN_BOX_HEIGHT: i32 = 120;
const COLUMN_GAP: i32 = 340;
const ROW_GAP: i32 = 160;
const BOUNDARY_WIDTH: i32 = 200;
const MARGIN: i32 = 50;

#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

struct PortAnchor {
    point: Point,
    kind: String,
}

struct BoxLayout<'a> {
    gear: &'a conduit_plot::CheckedGear,
    x: i32,
    y: i32,
    height: i32,
}

fn gear_height(gear: &conduit_plot::CheckedGear) -> i32 {
    (HEADER_HEIGHT + gear.inputs.len().max(gear.outputs.len()).max(1) as i32 * PORT_ROW + 24)
        .max(MIN_BOX_HEIGHT)
}

pub fn render_svg(
    authoring: &ExpandedAuthoringPlot,
    type_names: &BTreeMap<String, String>,
) -> String {
    let expanded = &authoring.expanded;
    let levels = gear_levels(authoring);
    let mut by_level = BTreeMap::<usize, Vec<_>>::new();
    for gear in &expanded.gears {
        let level = levels.get(gear.gear_id.as_str()).copied().unwrap_or(0);
        by_level.entry(level).or_default().push(gear);
    }
    let authored_order = expanded
        .gears
        .iter()
        .enumerate()
        .map(|(index, gear)| (gear.gear_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut vertical_rank = BTreeMap::<&str, usize>::new();
    for gears in by_level.values_mut() {
        gears.sort_by_key(|gear| {
            let predecessor_ranks = expanded
                .connections
                .iter()
                .filter(|connection| connection.sink_gear_id == gear.gear_id)
                .filter_map(|connection| {
                    vertical_rank
                        .get(connection.source_gear_id.as_str())
                        .copied()
                })
                .collect::<Vec<_>>();
            let barycenter = if predecessor_ranks.is_empty() {
                authored_order[gear.gear_id.as_str()] * 100
            } else {
                predecessor_ranks.iter().sum::<usize>() * 100 / predecessor_ranks.len()
            };
            (barycenter, authored_order[gear.gear_id.as_str()])
        });
        for (rank, gear) in gears.iter().enumerate() {
            vertical_rank.insert(gear.gear_id.as_str(), rank);
        }
    }
    let level_count = by_level.len().max(1) as i32;
    let width =
        MARGIN * 2 + BOUNDARY_WIDTH * 2 + level_count * BOX_WIDTH + (level_count + 1) * COLUMN_GAP;
    let tallest_column = by_level
        .values()
        .map(|gears| {
            gears.iter().map(|gear| gear_height(gear)).sum::<i32>()
                + gears.len().saturating_sub(1) as i32 * ROW_GAP
        })
        .max()
        .unwrap_or(MIN_BOX_HEIGHT);
    let height = (tallest_column + 480).max(780);
    let mut boxes = Vec::new();
    let mut x = MARGIN + BOUNDARY_WIDTH + COLUMN_GAP;
    for gears in by_level.values() {
        let column_height = gears.iter().map(|gear| gear_height(gear)).sum::<i32>()
            + gears.len().saturating_sub(1) as i32 * ROW_GAP;
        let mut y = (height - column_height) / 2;
        for gear in gears {
            let box_height = gear_height(gear);
            boxes.push(BoxLayout {
                gear,
                x,
                y,
                height: box_height,
            });
            y += box_height + ROW_GAP;
        }
        x += BOX_WIDTH + COLUMN_GAP;
    }
    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">"#
    )
    .unwrap();
    writeln!(
        svg,
        "<title id=\"title\">{}</title>",
        escape(&expanded.name)
    )
    .unwrap();
    writeln!(
        svg,
        "<desc id=\"desc\">Checked Conduit Plot with exact typed port connections.</desc>"
    )
    .unwrap();
    svg.push_str(r#"<style>svg{--diagram-background:#05070b;--diagram-surface:#090d16;--diagram-reading-paper:#0c121c;--diagram-structure:#0dd8f6;--diagram-structure-secondary:#0a1f87;--diagram-text:#93d2f7;--diagram-text-secondary:#578ec9;--diagram-emphasis:#e9a325;--diagram-failure:#ff7272;--diagram-success:#63d69b}@media(prefers-color-scheme:light){svg{--diagram-background:#eef5f8;--diagram-surface:#fff;--diagram-reading-paper:#fff;--diagram-structure:#00758c;--diagram-structure-secondary:#9ab8d1;--diagram-text:#17364d;--diagram-text-secondary:#496b82;--diagram-emphasis:#9a5b00;--diagram-failure:#a32d37;--diagram-success:#08784e}}text{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;fill:var(--conduit-text-primary,var(--diagram-text))}.canvas{fill:var(--conduit-background,var(--diagram-background))}.gear{fill:var(--conduit-reading-paper,var(--diagram-reading-paper));stroke:var(--conduit-text-secondary,var(--diagram-text-secondary));stroke-width:2}.header{fill:var(--conduit-surface,var(--diagram-surface))}.gear-mark{fill:var(--conduit-emphasis,var(--diagram-emphasis))}.gear-hole{fill:var(--conduit-surface,var(--diagram-surface))}.boundary{fill:var(--conduit-surface,var(--diagram-surface));stroke:var(--conduit-structure-primary,var(--diagram-structure));stroke-width:2}.port{fill:var(--conduit-background,var(--diagram-background));stroke-width:3}.input-port{stroke:var(--conduit-structure-primary,var(--diagram-structure))}.output-port{stroke:var(--conduit-emphasis,var(--diagram-emphasis))}.port-name{fill:var(--conduit-text-primary,var(--diagram-text));font-size:13px;font-weight:800}.cord-shadow{fill:none;stroke:var(--conduit-background,var(--diagram-background));stroke-width:9;stroke-linejoin:round}.cord{fill:none;stroke:var(--conduit-structure-primary,var(--diagram-structure));stroke-width:3;stroke-linejoin:round}.close{stroke:var(--conduit-text-secondary,var(--diagram-text-secondary));stroke-dasharray:8 5}.quiescence{stroke:#9d7bd1;stroke-dasharray:3 5}.abnormal{stroke:var(--conduit-failure,var(--diagram-failure));stroke-dasharray:5 5}.split-junction{fill:var(--conduit-emphasis,var(--diagram-emphasis));stroke:var(--conduit-background,var(--diagram-background));stroke-width:3}.join-junction{fill:var(--conduit-structure-primary,var(--diagram-structure));stroke:var(--conduit-background,var(--diagram-background));stroke-width:3}.kind{fill:var(--conduit-success,var(--diagram-success));font-size:13px}.boundary-port{fill:var(--conduit-text-primary,var(--diagram-text));font-size:13px;font-weight:750}.cord-tag{fill:var(--conduit-surface,var(--diagram-surface));stroke:var(--conduit-emphasis,var(--diagram-emphasis));stroke-width:1}.cord-label{fill:var(--conduit-emphasis,var(--diagram-emphasis));font-size:10px;font-weight:600}.label{font-size:16px}.title{font-size:24px;font-weight:750}.legend{font-size:13px;fill:var(--conduit-text-secondary,var(--diagram-text-secondary))}</style>"#);
    svg.push_str(r##"<defs><marker id="arrow" markerWidth="10" markerHeight="10" refX="9" refY="5" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="var(--conduit-structure-primary,var(--diagram-structure))"/></marker><marker id="arrow-abnormal" markerWidth="10" markerHeight="10" refX="9" refY="5" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="var(--conduit-failure,var(--diagram-failure))"/></marker><symbol id="gear-mark" viewBox="0 0 24 24"><path class="gear-mark" d="M19.4 13a7.7 7.7 0 0 0 0-2l2.1-1.6a.6.6 0 0 0 .1-.7l-2-3.5a.6.6 0 0 0-.7-.2l-2.5 1a8 8 0 0 0-1.7-1l-.4-2.6A.6.6 0 0 0 13.8 2h-4a.6.6 0 0 0-.6.4L8.9 5a8 8 0 0 0-1.7 1L4.7 5a.6.6 0 0 0-.7.2L2 8.7a.6.6 0 0 0 .1.7L4.2 11a7.7 7.7 0 0 0 0 2l-2.1 1.6a.6.6 0 0 0-.1.7l2 3.5a.6.6 0 0 0 .7.2l2.5-1a8 8 0 0 0 1.7 1l.4 2.6a.6.6 0 0 0 .6.4h4a.6.6 0 0 0 .6-.4l.4-2.6a8 8 0 0 0 1.7-1l2.5 1a.6.6 0 0 0 .7-.2l2-3.5a.6.6 0 0 0-.1-.7z"/><circle class="gear-hole" cx="12" cy="12" r="3.2"/></symbol></defs>"##);
    writeln!(
        svg,
        "<rect class=\"canvas\" width=\"{width}\" height=\"{height}\"/>"
    )
    .unwrap();
    writeln!(
        svg,
        "<text class=\"title\" x=\"{MARGIN}\" y=\"42\">Plot {}</text>",
        escape(&expanded.name)
    )
    .unwrap();
    writeln!(svg, "<text class=\"legend\" x=\"{MARGIN}\" y=\"68\">ordinary flow —  close - -  quiescence · ·  abnormal - -  ● junction; crossing without dot = separate cords</text>").unwrap();

    let mut nodes = String::new();
    let mut inputs = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    for layout in &boxes {
        draw_gear(&mut nodes, layout, &mut inputs, &mut outputs);
    }
    let front_inputs = authoring.front.inputs();
    let front_outputs = authoring.front.outputs();
    let input_boundary_height = boundary_height(front_inputs.len());
    let output_boundary_height = boundary_height(front_outputs.len());
    let input_boundary_y = (height - input_boundary_height) / 2;
    let output_boundary_x = width - MARGIN - BOUNDARY_WIDTH;
    let output_boundary_y = (height - output_boundary_height) / 2;
    draw_boundary(
        &mut nodes,
        "Plot inputs",
        MARGIN,
        input_boundary_y,
        front_inputs,
        true,
    );
    draw_boundary(
        &mut nodes,
        "Plot outputs",
        output_boundary_x,
        output_boundary_y,
        front_outputs,
        false,
    );
    let front_input_points = boundary_points(MARGIN, input_boundary_y, front_inputs, true);
    let front_output_points =
        boundary_points(output_boundary_x, output_boundary_y, front_outputs, false);

    let mut source_counts = BTreeMap::<(i32, i32), usize>::new();
    let mut sink_counts = BTreeMap::<(i32, i32), usize>::new();
    {
        let mut count_route = |from: &PortAnchor, to: &PortAnchor| {
            *source_counts
                .entry((from.point.x, from.point.y))
                .or_default() += 1;
            *sink_counts.entry((to.point.x, to.point.y)).or_default() += 1;
        };
        for connection in &expanded.connections {
            if let (Some(from), Some(to)) = (
                outputs.get(&(
                    connection.source_gear_id.as_str(),
                    connection.source_port_id.as_str(),
                )),
                inputs.get(&(
                    connection.sink_gear_id.as_str(),
                    connection.sink_port_id.as_str(),
                )),
            ) {
                count_route(from, to);
            }
        }
        for binding in &authoring.input_bindings {
            if let (Some(from), Some(to)) = (
                front_input_points.get(binding.front_port_id.as_str()),
                inputs.get(&(binding.gear_id.as_str(), binding.gear_port_id.as_str())),
            ) {
                count_route(from, to);
            }
        }
        for binding in &authoring.output_bindings {
            if let (Some(from), Some(to)) = (
                outputs.get(&(binding.gear_id.as_str(), binding.gear_port_id.as_str())),
                front_output_points.get(binding.front_port_id.as_str()),
            ) {
                count_route(from, to);
            }
        }
    }

    let mut source_seen = BTreeMap::<(i32, i32), usize>::new();
    let mut route_index = 0;
    for connection in &expanded.connections {
        if let (Some(from), Some(to)) = (
            outputs.get(&(
                connection.source_gear_id.as_str(),
                connection.source_port_id.as_str(),
            )),
            inputs.get(&(
                connection.sink_gear_id.as_str(),
                connection.sink_port_id.as_str(),
            )),
        ) {
            draw_cord(
                &mut svg,
                from.point,
                to.point,
                connection.track,
                information_label(&from.kind, type_names),
                &format!(
                    "{}.{} → {}.{} · {}",
                    connection.source_gear_id.as_str(),
                    connection.source_port_id.as_str(),
                    connection.sink_gear_id.as_str(),
                    connection.sink_port_id.as_str(),
                    from.kind
                ),
                route_index,
                source_counts[&(from.point.x, from.point.y)] > 1,
                sink_counts[&(to.point.x, to.point.y)] > 1,
                height,
                first_source_label(&mut source_seen, &source_counts, from.point),
            );
            route_index += 1;
        }
    }
    for binding in &authoring.input_bindings {
        if let (Some(from), Some(to)) = (
            front_input_points.get(binding.front_port_id.as_str()),
            inputs.get(&(binding.gear_id.as_str(), binding.gear_port_id.as_str())),
        ) {
            draw_cord(
                &mut svg,
                from.point,
                to.point,
                binding.track,
                information_label(&from.kind, type_names),
                &format!(
                    "Plot.{} → {}.{} · {}",
                    binding.front_port_id.as_str(),
                    binding.gear_id.as_str(),
                    binding.gear_port_id.as_str(),
                    from.kind
                ),
                route_index,
                source_counts[&(from.point.x, from.point.y)] > 1,
                sink_counts[&(to.point.x, to.point.y)] > 1,
                height,
                first_source_label(&mut source_seen, &source_counts, from.point),
            );
            route_index += 1;
        }
    }
    for binding in &authoring.output_bindings {
        if let (Some(from), Some(to)) = (
            outputs.get(&(binding.gear_id.as_str(), binding.gear_port_id.as_str())),
            front_output_points.get(binding.front_port_id.as_str()),
        ) {
            draw_cord(
                &mut svg,
                from.point,
                to.point,
                binding.track,
                information_label(&from.kind, type_names),
                &format!(
                    "{}.{} → Plot.{} · {}",
                    binding.gear_id.as_str(),
                    binding.gear_port_id.as_str(),
                    binding.front_port_id.as_str(),
                    from.kind
                ),
                route_index,
                source_counts[&(from.point.x, from.point.y)] > 1,
                sink_counts[&(to.point.x, to.point.y)] > 1,
                height,
                first_source_label(&mut source_seen, &source_counts, from.point),
            );
            route_index += 1;
        }
    }
    svg.push_str(&nodes);
    svg.push_str("</svg>\n");
    svg
}

fn first_source_label(
    seen: &mut BTreeMap<(i32, i32), usize>,
    counts: &BTreeMap<(i32, i32), usize>,
    point: Point,
) -> bool {
    let key = (point.x, point.y);
    let ordinal = seen.entry(key).or_default();
    let show = counts[&key] == 1 || *ordinal == 0;
    *ordinal += 1;
    show
}

fn gear_levels(authoring: &ExpandedAuthoringPlot) -> BTreeMap<&str, usize> {
    let mut levels = authoring
        .expanded
        .gears
        .iter()
        .map(|gear| (gear.gear_id.as_str(), 0))
        .collect::<BTreeMap<_, _>>();
    for _ in 0..authoring.expanded.gears.len() {
        let previous = levels.clone();
        let mut changed = false;
        for connection in &authoring.expanded.connections {
            let source = previous
                .get(connection.source_gear_id.as_str())
                .copied()
                .unwrap_or(0);
            let sink = levels.entry(connection.sink_gear_id.as_str()).or_default();
            let candidate = (source + 1).min(authoring.expanded.gears.len().saturating_sub(1));
            if connection.source_gear_id != connection.sink_gear_id && candidate > *sink {
                *sink = candidate;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    levels
}

fn draw_gear<'a>(
    svg: &mut String,
    layout: &BoxLayout<'a>,
    inputs: &mut BTreeMap<(&'a str, &'a str), PortAnchor>,
    outputs: &mut BTreeMap<(&'a str, &'a str), PortAnchor>,
) {
    let gear = layout.gear;
    writeln!(svg, "<g><rect class=\"gear\" x=\"{}\" y=\"{}\" width=\"{BOX_WIDTH}\" height=\"{}\" rx=\"10\"/><path class=\"header\" d=\"M{},{} q0,-10 10,-10 h260 q10,0 10,10 v42 h-{BOX_WIDTH} z\"/>", layout.x, layout.y, layout.height, layout.x, layout.y + 10).unwrap();
    writeln!(
        svg,
        "<title>{} — kind {}</title>",
        escape(gear.gear_id.as_str()),
        escape(gear.kind_id.as_str())
    )
    .unwrap();
    writeln!(svg, "<use href=\"#gear-mark\" x=\"{}\" y=\"{}\" width=\"28\" height=\"28\"/><text class=\"label\" x=\"{}\" y=\"{}\">{}</text><text class=\"kind\" x=\"{}\" y=\"{}\">{}</text>", layout.x + 12, layout.y + 11, layout.x + 50, layout.y + 22, escape(&shorten(short_name(gear.gear_id.as_str()), 24)), layout.x + 50, layout.y + 43, escape(&shorten(gear.gear_id.as_str(), 29))).unwrap();
    for (index, port) in gear.inputs.iter().enumerate() {
        let point = Point {
            x: layout.x,
            y: port_y(layout.y, index),
        };
        inputs.insert(
            (gear.gear_id.as_str(), port.port_id.as_str()),
            PortAnchor {
                point,
                kind: port.value_kind.as_str().to_string(),
            },
        );
        draw_gear_socket(
            svg,
            point,
            port.port_id.as_str(),
            port.value_kind.as_str(),
            11,
            true,
        );
    }
    for (index, port) in gear.outputs.iter().enumerate() {
        let point = Point {
            x: layout.x + BOX_WIDTH,
            y: port_y(layout.y, index),
        };
        outputs.insert(
            (gear.gear_id.as_str(), port.port_id.as_str()),
            PortAnchor {
                point,
                kind: port.value_kind.as_str().to_string(),
            },
        );
        draw_gear_socket(
            svg,
            point,
            port.port_id.as_str(),
            port.value_kind.as_str(),
            -11,
            false,
        );
    }
    svg.push_str("</g>\n");
}

fn port_y(box_y: i32, index: usize) -> i32 {
    box_y + HEADER_HEIGHT + 28 + index as i32 * PORT_ROW
}

fn shorten(value: &str, maximum_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix = chars.by_ref().take(maximum_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn short_name(value: &str) -> &str {
    value.rsplit('/').next().unwrap_or(value)
}

fn draw_socket(svg: &mut String, point: Point, name: &str, kind: &str, input: bool) {
    writeln!(
        svg,
        "<g><title>{}: {}</title><circle class=\"port {}\" cx=\"{}\" cy=\"{}\" r=\"5\"/></g>",
        escape(name),
        escape(kind),
        if input { "input-port" } else { "output-port" },
        point.x,
        point.y
    )
    .unwrap();
}

fn draw_gear_socket(
    svg: &mut String,
    point: Point,
    name: &str,
    kind: &str,
    label_offset: i32,
    input: bool,
) {
    draw_socket(svg, point, name, kind, input);
    writeln!(
        svg,
        "<text class=\"port-name\" text-anchor=\"{}\" x=\"{}\" y=\"{}\">{}</text>",
        if input { "start" } else { "end" },
        point.x + label_offset,
        point.y + 4,
        escape(&shorten(name, 18))
    )
    .unwrap();
}

fn draw_boundary(
    svg: &mut String,
    title: &str,
    x: i32,
    y: i32,
    ports: &[conduit_core::PortDescriptor],
    input: bool,
) {
    let height = boundary_height(ports.len());
    writeln!(svg, "<rect class=\"boundary\" x=\"{x}\" y=\"{y}\" width=\"{BOUNDARY_WIDTH}\" height=\"{height}\" rx=\"10\"/><text class=\"label\" x=\"{}\" y=\"{}\">{title}</text>", x + 16, y + 28).unwrap();
    for (index, port) in ports.iter().enumerate() {
        let point = Point {
            x: if input { x + BOUNDARY_WIDTH } else { x },
            y: y + 54 + index as i32 * PORT_ROW,
        };
        draw_socket(
            svg,
            point,
            port.port_id.as_str(),
            port.value_kind.as_str(),
            !input,
        );
        writeln!(
            svg,
            "<text class=\"boundary-port\" text-anchor=\"{}\" x=\"{}\" y=\"{}\">{}</text>",
            if input { "end" } else { "start" },
            point.x + if input { -12 } else { 12 },
            point.y + 4,
            escape(&shorten(port.port_id.as_str(), 24)),
        )
        .unwrap();
    }
}

fn boundary_height(port_count: usize) -> i32 {
    (62 + port_count.max(1) as i32 * PORT_ROW).max(88)
}

fn boundary_points(
    x: i32,
    y: i32,
    ports: &[conduit_core::PortDescriptor],
    input: bool,
) -> BTreeMap<&str, PortAnchor> {
    ports
        .iter()
        .enumerate()
        .map(|(index, port)| {
            (
                port.port_id.as_str(),
                PortAnchor {
                    point: Point {
                        x: if input { x + BOUNDARY_WIDTH } else { x },
                        y: y + 54 + index as i32 * PORT_ROW,
                    },
                    kind: port.value_kind.as_str().to_string(),
                },
            )
        })
        .collect()
}

// A cord's semantic and spatial facts stay explicit at each call site; folding
// them into an opaque options bag would make routing mistakes harder to review.
#[allow(clippy::too_many_arguments)]
fn draw_cord(
    svg: &mut String,
    from: Point,
    to: Point,
    track: ConnectionTrack,
    information: String,
    detail: &str,
    route_index: usize,
    fan_out: bool,
    fan_in: bool,
    diagram_height: i32,
    show_label: bool,
) {
    let (class, marker) = match track {
        ConnectionTrack::Payload => ("cord", "arrow"),
        ConnectionTrack::NormalClose => ("cord close", "arrow"),
        ConnectionTrack::AbnormalTerminal => ("cord abnormal", "arrow-abnormal"),
        ConnectionTrack::Quiescence => ("cord quiescence", "arrow"),
    };
    let lane_offsets = [-140, -105, -70, -35, 0, 35, 70, 105, 140];
    let split_x = if fan_out { from.x + 58 } else { from.x };
    let join_x = if fan_in { to.x - 58 } else { to.x };
    let forward_gap = join_x - split_x;
    let (path, label_x, label_line_y) = if from.y == to.y {
        (
            format!("M{},{} H{}", from.x, from.y, to.x),
            (split_x + join_x) / 2,
            from.y,
        )
    } else if forward_gap > 0 && forward_gap <= COLUMN_GAP + 24 {
        let midpoint = (split_x + join_x) / 2;
        let minimum = split_x + 18;
        let maximum = join_x - 18;
        let lane_x = (midpoint + lane_offsets[route_index % lane_offsets.len()])
            .clamp(minimum, maximum.max(minimum));
        let path = format!(
            "M{},{} H{} H{} V{} H{} H{}",
            from.x, from.y, split_x, lane_x, to.y, join_x, to.x
        );
        let source_run = lane_x - split_x;
        let sink_run = join_x - lane_x;
        if sink_run >= source_run {
            (path, (lane_x + join_x) / 2, to.y)
        } else {
            (path, (split_x + lane_x) / 2, from.y)
        }
    } else {
        let bus_slot = (route_index / 2) % 6;
        let bus_y = if route_index.is_multiple_of(2) {
            100 + bus_slot as i32 * 22
        } else {
            diagram_height - 60 - bus_slot as i32 * 22
        };
        let source_exit_x = split_x + 72;
        let sink_entry_x = join_x - 72;
        let path = format!(
            "M{},{} H{} H{} V{} H{} V{} H{} H{}",
            from.x, from.y, split_x, source_exit_x, bus_y, sink_entry_x, to.y, join_x, to.x
        );
        (path, (source_exit_x + sink_entry_x) / 2, bus_y)
    };
    let label = match track {
        ConnectionTrack::Payload => information,
        _ => format!("{} · {}", information, track_name(track)),
    };
    let label_width = (label.chars().count() as i32 * 6 + 16).clamp(72, 300);
    let label_y = label_line_y - 10;
    writeln!(svg, "<g><title>{} · {}</title><path class=\"cord-shadow\" d=\"{path}\"/><path class=\"{class}\" marker-end=\"url(#{marker})\" d=\"{path}\"/>", escape(detail), track_name(track)).unwrap();
    if fan_out {
        writeln!(svg, "<circle class=\"split-junction\" cx=\"{split_x}\" cy=\"{}\" r=\"6\"><title>fan-out junction</title></circle>", from.y).unwrap();
    }
    if fan_in {
        writeln!(svg, "<circle class=\"join-junction\" cx=\"{join_x}\" cy=\"{}\" r=\"6\"><title>fan-in junction</title></circle>", to.y).unwrap();
    }
    if show_label {
        writeln!(svg, "<rect class=\"cord-tag\" x=\"{}\" y=\"{}\" width=\"{label_width}\" height=\"18\" rx=\"6\"/><text class=\"cord-label\" text-anchor=\"middle\" x=\"{label_x}\" y=\"{}\">{}</text>", label_x - label_width / 2, label_y - 12, label_y, escape(&label)).unwrap();
    }
    svg.push_str("</g>\n");
}

pub fn information_labels(
    authoring: &ExpandedAuthoringPlot,
    startup: &conduit_plot::StartupCatalog,
) -> BTreeMap<String, String> {
    authoring
        .expanded
        .gears
        .iter()
        .flat_map(|gear| gear.inputs.iter().chain(&gear.outputs))
        .chain(authoring.front.inputs())
        .chain(authoring.front.outputs())
        .filter_map(|port| {
            startup
                .structured_type_name(&port.value_kind)
                .map(|name| (port.value_kind.as_str().to_string(), name.to_string()))
        })
        .collect()
}

fn information_label(kind: &str, type_names: &BTreeMap<String, String>) -> String {
    if let Some(name) = type_names.get(kind) {
        return name.clone();
    }
    if let Some(profile) = kind.strip_prefix("structured-info/profile-") {
        let (digest, version) = profile.split_once('@').unwrap_or((profile, ""));
        let digest = digest.chars().take(8).collect::<String>();
        if version.is_empty() {
            format!("profile-{digest}…")
        } else {
            format!("profile-{digest}…@{version}")
        }
    } else {
        kind.to_string()
    }
}

fn track_name(track: ConnectionTrack) -> &'static str {
    match track {
        ConnectionTrack::Payload => "payload",
        ConnectionTrack::NormalClose => "close",
        ConnectionTrack::AbnormalTerminal => "abnormal",
        ConnectionTrack::Quiescence => "quiescence",
    }
}

pub fn render_mermaid(
    authoring: &ExpandedAuthoringPlot,
    type_names: &BTreeMap<String, String>,
) -> String {
    let mut output = String::from("flowchart TB\n");
    output.push_str("  plot_in[\"Plot inputs\"]\n  plot_out[\"Plot outputs\"]\n");
    let ids = authoring
        .expanded
        .gears
        .iter()
        .enumerate()
        .map(|(index, gear)| (gear.gear_id.as_str(), format!("gear_{index}")))
        .collect::<BTreeMap<_, _>>();
    for gear in &authoring.expanded.gears {
        writeln!(
            output,
            "  {}[\"{}<br/><small>{}</small>\"]",
            ids[gear.gear_id.as_str()],
            mermaid_escape(gear.gear_id.as_str()),
            mermaid_escape(gear.kind_id.as_str())
        )
        .unwrap();
    }
    let output_information = authoring
        .expanded
        .gears
        .iter()
        .flat_map(|gear| {
            gear.outputs.iter().map(move |port| {
                (
                    (gear.gear_id.as_str(), port.port_id.as_str()),
                    information_label(port.value_kind.as_str(), type_names),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let front_input_information = authoring
        .front
        .inputs()
        .iter()
        .map(|port| {
            (
                port.port_id.as_str(),
                information_label(port.value_kind.as_str(), type_names),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for binding in &authoring.input_bindings {
        writeln!(
            output,
            "  plot_in -- \"{}{}\" --> {}",
            mermaid_escape(&front_input_information[binding.front_port_id.as_str()]),
            track_suffix(binding.track),
            ids[binding.gear_id.as_str()]
        )
        .unwrap();
    }
    for connection in &authoring.expanded.connections {
        writeln!(
            output,
            "  {} -- \"{}{}\" --> {}",
            ids[connection.source_gear_id.as_str()],
            mermaid_escape(
                &output_information[&(
                    connection.source_gear_id.as_str(),
                    connection.source_port_id.as_str()
                )]
            ),
            track_suffix(connection.track),
            ids[connection.sink_gear_id.as_str()]
        )
        .unwrap();
    }
    for binding in &authoring.output_bindings {
        writeln!(
            output,
            "  {} -- \"{}{}\" --> plot_out",
            ids[binding.gear_id.as_str()],
            mermaid_escape(
                &output_information[&(binding.gear_id.as_str(), binding.gear_port_id.as_str())]
            ),
            track_suffix(binding.track)
        )
        .unwrap();
    }
    output
}

fn track_suffix(track: ConnectionTrack) -> &'static str {
    match track {
        ConnectionTrack::Payload => "",
        ConnectionTrack::NormalClose => " [close]",
        ConnectionTrack::AbnormalTerminal => " [abnormal]",
        ConnectionTrack::Quiescence => " [quiescence]",
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn mermaid_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
