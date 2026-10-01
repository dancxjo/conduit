use crate::cli::DiagramFormat;
use conduit_core::ConnectionTrack;
use conduit_form::ExpandedAuthoringForm;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

const BOX_WIDTH: i32 = 250;
const COLUMN_GAP: i32 = 100;
const ROW_GAP: i32 = 40;
const HEADER_HEIGHT: i32 = 48;
const PORT_ROW: i32 = 30;
const MARGIN: i32 = 40;

pub(crate) fn run(form: &Path, format: DiagramFormat, output: Option<&Path>) -> Result<(), String> {
    let source = crate::form_source::load(form)?;
    let authoring = source.expand_entry_for_authoring()?;
    let rendered = match format {
        DiagramFormat::Svg => render_svg(&authoring),
        DiagramFormat::Mermaid => render_mermaid(&authoring),
    };
    if let Some(path) = output {
        std::fs::write(path, rendered).map_err(|error| error.to_string())
    } else {
        print!("{rendered}");
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

struct BoxLayout<'a> {
    gear: &'a conduit_form::CheckedGear,
    x: i32,
    y: i32,
    height: i32,
}

fn render_svg(authoring: &ExpandedAuthoringForm) -> String {
    let expanded = &authoring.expanded;
    let levels = gear_levels(authoring);
    let mut next_y = BTreeMap::<usize, i32>::new();
    let mut boxes = Vec::new();
    for gear in &expanded.gears {
        let level = levels.get(gear.gear_id.as_str()).copied().unwrap_or(0);
        let height =
            HEADER_HEIGHT + PORT_ROW * gear.inputs.len().max(gear.outputs.len()).max(1) as i32 + 12;
        let y = *next_y.entry(level).or_insert(MARGIN + 70);
        boxes.push(BoxLayout {
            gear,
            x: MARGIN + 170 + level as i32 * (BOX_WIDTH + COLUMN_GAP),
            y,
            height,
        });
        next_y.insert(level, y + height + ROW_GAP);
    }
    let maximum_level = levels.values().copied().max().unwrap_or(0);
    let width = MARGIN * 2 + 340 + (maximum_level as i32 + 1) * (BOX_WIDTH + COLUMN_GAP);
    let height = boxes
        .iter()
        .map(|layout| layout.y + layout.height + MARGIN)
        .max()
        .unwrap_or(240)
        .max(240);
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
        "<desc id=\"desc\">Checked Conduit Form with exact typed port connections.</desc>"
    )
    .unwrap();
    svg.push_str(r#"<style>text{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;fill:#172033}.gear{fill:#f7f9fc;stroke:#52647d;stroke-width:2}.header{fill:#dce7f5}.boundary{fill:#eef8ee;stroke:#4f7654;stroke-width:2}.port{fill:#fff;stroke:#52647d;stroke-width:2}.cord{fill:none;stroke:#3973ac;stroke-width:2}.close{stroke:#777;stroke-dasharray:6 4}.quiescence{stroke:#7851a9;stroke-dasharray:2 4}.abnormal{stroke:#b64040;stroke-dasharray:4 4}.kind{fill:#52647d;font-size:11px}.label{font-size:12px}.title{font-size:18px;font-weight:700}</style>"#);
    svg.push_str(r##"<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8 z" fill="#3973ac"/></marker><marker id="arrow-abnormal" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8 z" fill="#b64040"/></marker></defs>"##);
    writeln!(
        svg,
        "<text class=\"title\" x=\"{MARGIN}\" y=\"32\">Form {}</text>",
        escape(&expanded.name)
    )
    .unwrap();

    let mut nodes = String::new();
    let mut inputs = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    for layout in &boxes {
        draw_gear(&mut nodes, layout, &mut inputs, &mut outputs);
    }
    let boundary_output_x = width - MARGIN - 130;
    let front_inputs = authoring.front.inputs();
    let front_outputs = authoring.front.outputs();
    draw_boundary(&mut nodes, "Form inputs", MARGIN, 75, front_inputs, true);
    draw_boundary(
        &mut nodes,
        "Form outputs",
        boundary_output_x,
        75,
        front_outputs,
        false,
    );
    let front_input_points = boundary_points(MARGIN, 75, front_inputs, true);
    let front_output_points = boundary_points(boundary_output_x, 75, front_outputs, false);

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
            draw_cord(&mut svg, *from, *to, connection.track);
        }
    }
    for binding in &authoring.input_bindings {
        if let (Some(from), Some(to)) = (
            front_input_points.get(binding.front_port_id.as_str()),
            inputs.get(&(binding.gear_id.as_str(), binding.gear_port_id.as_str())),
        ) {
            draw_cord(&mut svg, *from, *to, binding.track);
        }
    }
    for binding in &authoring.output_bindings {
        if let (Some(from), Some(to)) = (
            outputs.get(&(binding.gear_id.as_str(), binding.gear_port_id.as_str())),
            front_output_points.get(binding.front_port_id.as_str()),
        ) {
            draw_cord(&mut svg, *from, *to, binding.track);
        }
    }
    svg.push_str(&nodes);
    svg.push_str("</svg>\n");
    svg
}

fn gear_levels(authoring: &ExpandedAuthoringForm) -> BTreeMap<&str, usize> {
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
    inputs: &mut BTreeMap<(&'a str, &'a str), Point>,
    outputs: &mut BTreeMap<(&'a str, &'a str), Point>,
) {
    let gear = layout.gear;
    writeln!(svg, "<g><rect class=\"gear\" x=\"{}\" y=\"{}\" width=\"{BOX_WIDTH}\" height=\"{}\" rx=\"8\"/><path class=\"header\" d=\"M{},{} h{BOX_WIDTH} v40 h-{BOX_WIDTH} z\"/>", layout.x, layout.y, layout.height, layout.x, layout.y).unwrap();
    writeln!(svg, "<text class=\"label\" x=\"{}\" y=\"{}\">{}</text><text class=\"kind\" x=\"{}\" y=\"{}\">{}</text>", layout.x + 12, layout.y + 18, escape(gear.gear_id.as_str()), layout.x + 12, layout.y + 34, escape(gear.kind_id.as_str())).unwrap();
    for (index, port) in gear.inputs.iter().enumerate() {
        let point = Point {
            x: layout.x,
            y: layout.y + HEADER_HEIGHT + index as i32 * PORT_ROW + 13,
        };
        inputs.insert((gear.gear_id.as_str(), port.port_id.as_str()), point);
        draw_port(
            svg,
            point,
            port.port_id.as_str(),
            port.value_kind.as_str(),
            true,
        );
    }
    for (index, port) in gear.outputs.iter().enumerate() {
        let point = Point {
            x: layout.x + BOX_WIDTH,
            y: layout.y + HEADER_HEIGHT + index as i32 * PORT_ROW + 13,
        };
        outputs.insert((gear.gear_id.as_str(), port.port_id.as_str()), point);
        draw_port(
            svg,
            point,
            port.port_id.as_str(),
            port.value_kind.as_str(),
            false,
        );
    }
    svg.push_str("</g>\n");
}

fn draw_port(svg: &mut String, point: Point, name: &str, kind: &str, input: bool) {
    writeln!(
        svg,
        "<circle class=\"port\" cx=\"{}\" cy=\"{}\" r=\"5\"/>",
        point.x, point.y
    )
    .unwrap();
    let (x, anchor) = if input {
        (point.x + 10, "start")
    } else {
        (point.x - 10, "end")
    };
    writeln!(svg, "<text class=\"label\" text-anchor=\"{anchor}\" x=\"{x}\" y=\"{}\">{} <tspan class=\"kind\">{}</tspan></text>", point.y + 4, escape(name), escape(kind)).unwrap();
}

fn draw_boundary(
    svg: &mut String,
    title: &str,
    x: i32,
    y: i32,
    ports: &[conduit_core::PortDescriptor],
    input: bool,
) {
    let height = HEADER_HEIGHT + PORT_ROW * ports.len().max(1) as i32;
    writeln!(svg, "<rect class=\"boundary\" x=\"{x}\" y=\"{y}\" width=\"130\" height=\"{height}\" rx=\"8\"/><text class=\"label\" x=\"{}\" y=\"{}\">{title}</text>", x + 10, y + 24).unwrap();
    for (index, port) in ports.iter().enumerate() {
        let point = Point {
            x: if input { x + 130 } else { x },
            y: y + HEADER_HEIGHT + index as i32 * PORT_ROW + 5,
        };
        draw_port(
            svg,
            point,
            port.port_id.as_str(),
            port.value_kind.as_str(),
            !input,
        );
    }
}

fn boundary_points(
    x: i32,
    y: i32,
    ports: &[conduit_core::PortDescriptor],
    input: bool,
) -> BTreeMap<&str, Point> {
    ports
        .iter()
        .enumerate()
        .map(|(index, port)| {
            (
                port.port_id.as_str(),
                Point {
                    x: if input { x + 130 } else { x },
                    y: y + HEADER_HEIGHT + index as i32 * PORT_ROW + 5,
                },
            )
        })
        .collect()
}

fn draw_cord(svg: &mut String, from: Point, to: Point, track: ConnectionTrack) {
    let bend = ((to.x - from.x).abs() / 2).max(35);
    let (class, marker) = match track {
        ConnectionTrack::Payload => ("cord", "arrow"),
        ConnectionTrack::NormalClose => ("cord close", "arrow"),
        ConnectionTrack::AbnormalTerminal => ("cord abnormal", "arrow-abnormal"),
        ConnectionTrack::Quiescence => ("cord quiescence", "arrow"),
    };
    writeln!(
        svg,
        "<path class=\"{class}\" marker-end=\"url(#{marker})\" d=\"M{},{} C{},{} {},{} {},{}\"/>",
        from.x,
        from.y,
        from.x + bend,
        from.y,
        to.x - bend,
        to.y,
        to.x,
        to.y
    )
    .unwrap();
}

fn render_mermaid(authoring: &ExpandedAuthoringForm) -> String {
    let mut output = String::from("flowchart LR\n");
    output.push_str("  form_in[\"Form inputs\"]\n  form_out[\"Form outputs\"]\n");
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
    for binding in &authoring.input_bindings {
        writeln!(
            output,
            "  form_in -- \"{} → {}\" --> {}",
            mermaid_escape(binding.front_port_id.as_str()),
            mermaid_escape(binding.gear_port_id.as_str()),
            ids[binding.gear_id.as_str()]
        )
        .unwrap();
    }
    for connection in &authoring.expanded.connections {
        writeln!(
            output,
            "  {} -- \"{} → {}{}\" --> {}",
            ids[connection.source_gear_id.as_str()],
            mermaid_escape(connection.source_port_id.as_str()),
            mermaid_escape(connection.sink_port_id.as_str()),
            track_suffix(connection.track),
            ids[connection.sink_gear_id.as_str()]
        )
        .unwrap();
    }
    for binding in &authoring.output_bindings {
        writeln!(
            output,
            "  {} -- \"{} → {}\" --> form_out",
            ids[binding.gear_id.as_str()],
            mermaid_escape(binding.gear_port_id.as_str()),
            mermaid_escape(binding.front_port_id.as_str())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(source: &str) -> ExpandedAuthoringForm {
        crate::form_source::parse(source)
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap()
    }

    #[test]
    fn svg_connects_exact_ports_and_escapes_labels() {
        let form = checked("form main (\n  >> text: Text\n  shown: Text >>\n) {\n  pass: text/join(\"&\")\n  text >> pass >> shown\n}\n");
        let svg = render_svg(&form);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Form inputs"));
        assert!(svg.contains("text"));
        assert!(svg.contains("marker-end=\"url(#arrow)\""));
        assert!(!svg.contains("text/join(\"&\")"));
    }

    #[test]
    fn mermaid_names_both_ends_of_every_cord() {
        let form = checked("form main (\n  >> text: Text\n  shown: Text >>\n) {\n  pass: text/join(\" \" )\n  text >> pass >> shown\n}\n");
        let mermaid = render_mermaid(&form);
        assert!(mermaid.contains("text → text"));
        assert!(mermaid.contains("text → shown"));
    }
}
