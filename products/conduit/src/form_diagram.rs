use crate::cli::DiagramFormat;
use conduit_core::ConnectionTrack;
use conduit_form::ExpandedAuthoringForm;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

const BOX_WIDTH: i32 = 420;
const BOX_HEIGHT: i32 = 112;
const COLUMN_GAP: i32 = 70;
const ROW_GAP: i32 = 70;
const LEVEL_GAP: i32 = 90;
const MARGIN: i32 = 50;
const COLUMNS: usize = 2;

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
}

fn render_svg(authoring: &ExpandedAuthoringForm) -> String {
    let expanded = &authoring.expanded;
    let levels = gear_levels(authoring);
    let mut by_level = BTreeMap::<usize, Vec<_>>::new();
    for gear in &expanded.gears {
        let level = levels.get(gear.gear_id.as_str()).copied().unwrap_or(0);
        by_level.entry(level).or_default().push(gear);
    }
    let width = MARGIN * 2 + COLUMNS as i32 * BOX_WIDTH + (COLUMNS as i32 - 1) * COLUMN_GAP;
    let mut boxes = Vec::new();
    let mut y = 220;
    for gears in by_level.values() {
        for (index, gear) in gears.iter().enumerate() {
            let column = index % COLUMNS;
            let row = index / COLUMNS;
            boxes.push(BoxLayout {
                gear,
                x: MARGIN + column as i32 * (BOX_WIDTH + COLUMN_GAP),
                y: y + row as i32 * (BOX_HEIGHT + ROW_GAP),
            });
        }
        let rows = gears.len().div_ceil(COLUMNS).max(1) as i32;
        y += rows * BOX_HEIGHT + (rows - 1) * ROW_GAP + LEVEL_GAP;
    }
    let output_y = y.max(250);
    let height = output_y + 180;
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
    svg.push_str(r#"<style>text{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;fill:#172033}.canvas{fill:#fbfcfe}.gear{fill:#fff;stroke:#52647d;stroke-width:2}.header{fill:#dce7f5}.boundary{fill:#eef8ee;stroke:#4f7654;stroke-width:2}.port{fill:#fff;stroke:#315c87;stroke-width:3}.cord-shadow{fill:none;stroke:#fff;stroke-width:7}.cord{fill:none;stroke:#3973ac;stroke-width:3}.close{stroke:#666;stroke-dasharray:8 5}.quiescence{stroke:#7851a9;stroke-dasharray:3 5}.abnormal{stroke:#b64040;stroke-dasharray:5 5}.kind{fill:#52647d;font-size:13px}.port-label{font-size:14px;font-weight:650}.label{font-size:16px}.title{font-size:24px;font-weight:750}.legend{font-size:13px;fill:#52647d}</style>"#);
    svg.push_str(r##"<defs><marker id="arrow" markerWidth="10" markerHeight="10" refX="9" refY="5" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="#3973ac"/></marker><marker id="arrow-abnormal" markerWidth="10" markerHeight="10" refX="9" refY="5" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="#b64040"/></marker></defs>"##);
    writeln!(
        svg,
        "<rect class=\"canvas\" width=\"{width}\" height=\"{height}\"/>"
    )
    .unwrap();
    writeln!(
        svg,
        "<text class=\"title\" x=\"{MARGIN}\" y=\"42\">Form {}</text>",
        escape(&expanded.name)
    )
    .unwrap();
    writeln!(svg, "<text class=\"legend\" x=\"{MARGIN}\" y=\"68\">payload —  close - -  quiescence · ·  abnormal - -</text>").unwrap();

    let mut nodes = String::new();
    let mut inputs = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    for layout in &boxes {
        draw_gear(&mut nodes, layout, &mut inputs, &mut outputs);
    }
    let front_inputs = authoring.front.inputs();
    let front_outputs = authoring.front.outputs();
    let boundary_x = (width - BOX_WIDTH) / 2;
    draw_boundary(
        &mut nodes,
        "Form inputs",
        boundary_x,
        95,
        front_inputs,
        true,
    );
    draw_boundary(
        &mut nodes,
        "Form outputs",
        boundary_x,
        output_y,
        front_outputs,
        false,
    );
    let front_input_points = boundary_points(boundary_x, 95, front_inputs, true);
    let front_output_points = boundary_points(boundary_x, output_y, front_outputs, false);

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
    writeln!(svg, "<g><rect class=\"gear\" x=\"{}\" y=\"{}\" width=\"{BOX_WIDTH}\" height=\"{BOX_HEIGHT}\" rx=\"10\"/><path class=\"header\" d=\"M{},{} h{BOX_WIDTH} v52 h-{BOX_WIDTH} z\"/>", layout.x, layout.y, layout.x, layout.y).unwrap();
    writeln!(
        svg,
        "<title>{} — kind {}</title>",
        escape(gear.gear_id.as_str()),
        escape(gear.kind_id.as_str())
    )
    .unwrap();
    writeln!(svg, "<text class=\"label\" x=\"{}\" y=\"{}\">{}</text><text class=\"kind\" x=\"{}\" y=\"{}\">kind · {}</text>", layout.x + 16, layout.y + 22, escape(&shorten(gear.gear_id.as_str(), 45)), layout.x + 16, layout.y + 43, escape(&shorten(gear.kind_id.as_str(), 52))).unwrap();
    for (index, port) in gear.inputs.iter().enumerate() {
        let point = Point {
            x: port_x(layout.x, index, gear.inputs.len()),
            y: layout.y,
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
            x: port_x(layout.x, index, gear.outputs.len()),
            y: layout.y + BOX_HEIGHT,
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

fn port_x(box_x: i32, index: usize, count: usize) -> i32 {
    box_x + ((index + 1) as i32 * BOX_WIDTH) / (count.max(1) as i32 + 1)
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

fn draw_port(svg: &mut String, point: Point, name: &str, kind: &str, input: bool) {
    writeln!(
        svg,
        "<circle class=\"port\" cx=\"{}\" cy=\"{}\" r=\"5\"/>",
        point.x, point.y
    )
    .unwrap();
    let y = if input { point.y + 20 } else { point.y - 10 };
    writeln!(svg, "<text class=\"port-label\" text-anchor=\"middle\" x=\"{}\" y=\"{y}\">{}</text><title>{}: {}</title>", point.x, escape(&shorten(name, 18)), escape(name), escape(kind)).unwrap();
}

fn draw_boundary(
    svg: &mut String,
    title: &str,
    x: i32,
    y: i32,
    ports: &[conduit_core::PortDescriptor],
    input: bool,
) {
    let height = 74;
    writeln!(svg, "<rect class=\"boundary\" x=\"{x}\" y=\"{y}\" width=\"{BOX_WIDTH}\" height=\"{height}\" rx=\"10\"/><text class=\"label\" x=\"{}\" y=\"{}\">{title}</text>", x + 16, y + 28).unwrap();
    for (index, port) in ports.iter().enumerate() {
        let point = Point {
            x: port_x(x, index, ports.len()),
            y: if input { y + height } else { y },
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
                    x: port_x(x, index, ports.len()),
                    y: if input { y + 74 } else { y },
                },
            )
        })
        .collect()
}

fn draw_cord(svg: &mut String, from: Point, to: Point, track: ConnectionTrack) {
    let bend = ((to.y - from.y).abs() / 2).max(40);
    let (class, marker) = match track {
        ConnectionTrack::Payload => ("cord", "arrow"),
        ConnectionTrack::NormalClose => ("cord close", "arrow"),
        ConnectionTrack::AbnormalTerminal => ("cord abnormal", "arrow-abnormal"),
        ConnectionTrack::Quiescence => ("cord quiescence", "arrow"),
    };
    let path = format!(
        "M{},{} C{},{} {},{} {},{}",
        from.x,
        from.y,
        from.x,
        from.y + bend,
        to.x,
        to.y - bend,
        to.x,
        to.y
    );
    writeln!(svg, "<path class=\"cord-shadow\" d=\"{path}\"/><path class=\"{class}\" marker-end=\"url(#{marker})\" d=\"{path}\"/>").unwrap();
}

fn render_mermaid(authoring: &ExpandedAuthoringForm) -> String {
    let mut output = String::from("flowchart TB\n");
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
            "  {}[\"{}<br/><small>kind · {}</small>\"]",
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
        assert!(svg.contains("kind · text/join"));
        assert!(svg.contains("width=\"1010\""));
        assert!(!svg.contains("text/join(\"&\")"));
    }

    #[test]
    fn mermaid_names_both_ends_of_every_cord() {
        let form = checked("form main (\n  >> text: Text\n  shown: Text >>\n) {\n  pass: text/join(\" \" )\n  text >> pass >> shown\n}\n");
        let mermaid = render_mermaid(&form);
        assert!(mermaid.contains("text → text"));
        assert!(mermaid.contains("text → shown"));
        assert!(mermaid.contains("kind · text/join"));
    }

    #[test]
    fn svg_includes_every_expanded_gear_and_internal_cord() {
        let form = checked("form main (\n  >> text: Text\n  shown: Text >>\n) {\n  first: text/join(\" \" )\n  second: text/join(\" \" )\n  text >> first >> second >> shown\n}\n");
        let svg = render_svg(&form);
        assert!(svg.contains("first</text>"));
        assert!(svg.contains("second</text>"));
        assert_eq!(svg.matches("marker-end=\"url(#arrow)\"").count(), 3);
    }
}
