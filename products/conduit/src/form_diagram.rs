use crate::cli::DiagramFormat;
use conduit_core::ConnectionTrack;
use conduit_form::ExpandedAuthoringForm;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

const BOX_WIDTH: i32 = 420;
const BOX_HEIGHT: i32 = 78;
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

struct PortAnchor {
    point: Point,
    kind: String,
    name: String,
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
    svg.push_str(r#"<style>text{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;fill:#17233c}.canvas{fill:#f5f1e8}.gear{fill:#fffdf8;stroke:#344766;stroke-width:2}.header{fill:#d9e8f2}.boundary{fill:#dfeee5;stroke:#39705a;stroke-width:2}.port{fill:#fffdf8;stroke:#176b87;stroke-width:3}.cord-shadow{fill:none;stroke:#f5f1e8;stroke-width:8}.cord{fill:none;stroke:#1677a6;stroke-width:3}.close{stroke:#657083;stroke-dasharray:8 5}.quiescence{stroke:#7656a8;stroke-dasharray:3 5}.abnormal{stroke:#c34f52;stroke-dasharray:5 5}.kind{fill:#52647d;font-size:13px}.boundary-port{fill:#284c3c;font-size:12px;font-weight:650}.cord-tag{fill:#fff0cf;stroke:#bd8127;stroke-width:1}.cord-label{fill:#49320f;font-size:11px;font-weight:650}.label{font-size:16px}.title{font-size:24px;font-weight:750}.legend{font-size:13px;fill:#52647d}</style>"#);
    svg.push_str(r##"<defs><marker id="arrow" markerWidth="10" markerHeight="10" refX="9" refY="5" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="#1677a6"/></marker><marker id="arrow-abnormal" markerWidth="10" markerHeight="10" refX="9" refY="5" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="#c34f52"/></marker></defs>"##);
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
    writeln!(svg, "<text class=\"legend\" x=\"{MARGIN}\" y=\"68\">ordinary flow —  close - -  quiescence · ·  abnormal - -</text>").unwrap();

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
            draw_cord(
                &mut svg,
                from.point,
                to.point,
                connection.track,
                information_label(&from.kind, &from.name),
                &format!(
                    "{}.{} → {}.{} · {}",
                    connection.source_gear_id.as_str(),
                    connection.source_port_id.as_str(),
                    connection.sink_gear_id.as_str(),
                    connection.sink_port_id.as_str(),
                    from.kind
                ),
            );
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
                information_label(&from.kind, &from.name),
                &format!(
                    "Form.{} → {}.{} · {}",
                    binding.front_port_id.as_str(),
                    binding.gear_id.as_str(),
                    binding.gear_port_id.as_str(),
                    from.kind
                ),
            );
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
                information_label(&from.kind, &from.name),
                &format!(
                    "{}.{} → Form.{} · {}",
                    binding.gear_id.as_str(),
                    binding.gear_port_id.as_str(),
                    binding.front_port_id.as_str(),
                    from.kind
                ),
            );
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
    inputs: &mut BTreeMap<(&'a str, &'a str), PortAnchor>,
    outputs: &mut BTreeMap<(&'a str, &'a str), PortAnchor>,
) {
    let gear = layout.gear;
    writeln!(svg, "<g><rect class=\"gear\" x=\"{}\" y=\"{}\" width=\"{BOX_WIDTH}\" height=\"{BOX_HEIGHT}\" rx=\"10\"/><path class=\"header\" d=\"M{},{} q0,-10 10,-10 h400 q10,0 10,10 v42 h-{BOX_WIDTH} z\"/>", layout.x, layout.y, layout.x, layout.y + 10).unwrap();
    writeln!(
        svg,
        "<title>{} — kind {}</title>",
        escape(gear.gear_id.as_str()),
        escape(gear.kind_id.as_str())
    )
    .unwrap();
    writeln!(svg, "<text class=\"label\" x=\"{}\" y=\"{}\">{}</text><text class=\"kind\" x=\"{}\" y=\"{}\">{}</text>", layout.x + 16, layout.y + 22, escape(&shorten(gear.gear_id.as_str(), 45)), layout.x + 16, layout.y + 43, escape(&shorten(gear.kind_id.as_str(), 52))).unwrap();
    for (index, port) in gear.inputs.iter().enumerate() {
        let point = Point {
            x: port_x(layout.x, index, gear.inputs.len()),
            y: layout.y,
        };
        inputs.insert(
            (gear.gear_id.as_str(), port.port_id.as_str()),
            PortAnchor {
                point,
                kind: port.value_kind.as_str().to_string(),
                name: port.port_id.as_str().to_string(),
            },
        );
        draw_socket(svg, point, port.port_id.as_str(), port.value_kind.as_str());
    }
    for (index, port) in gear.outputs.iter().enumerate() {
        let point = Point {
            x: port_x(layout.x, index, gear.outputs.len()),
            y: layout.y + BOX_HEIGHT,
        };
        outputs.insert(
            (gear.gear_id.as_str(), port.port_id.as_str()),
            PortAnchor {
                point,
                kind: port.value_kind.as_str().to_string(),
                name: port.port_id.as_str().to_string(),
            },
        );
        draw_socket(svg, point, port.port_id.as_str(), port.value_kind.as_str());
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

fn draw_socket(svg: &mut String, point: Point, name: &str, kind: &str) {
    writeln!(
        svg,
        "<g><title>{}: {}</title><circle class=\"port\" cx=\"{}\" cy=\"{}\" r=\"5\"/></g>",
        escape(name),
        escape(kind),
        point.x,
        point.y
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
    let height = 74;
    writeln!(svg, "<rect class=\"boundary\" x=\"{x}\" y=\"{y}\" width=\"{BOX_WIDTH}\" height=\"{height}\" rx=\"10\"/><text class=\"label\" x=\"{}\" y=\"{}\">{title}</text>", x + 16, y + 28).unwrap();
    for (index, port) in ports.iter().enumerate() {
        let point = Point {
            x: port_x(x, index, ports.len()),
            y: if input { y + height } else { y },
        };
        draw_socket(svg, point, port.port_id.as_str(), port.value_kind.as_str());
        writeln!(
            svg,
            "<text class=\"boundary-port\" text-anchor=\"middle\" x=\"{}\" y=\"{}\">{} : {}</text>",
            point.x,
            y + 55,
            escape(&shorten(port.port_id.as_str(), 16)),
            escape(&shorten(
                &information_label(port.value_kind.as_str(), port.port_id.as_str()),
                18
            ))
        )
        .unwrap();
    }
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
                        x: port_x(x, index, ports.len()),
                        y: if input { y + 74 } else { y },
                    },
                    kind: port.value_kind.as_str().to_string(),
                    name: port.port_id.as_str().to_string(),
                },
            )
        })
        .collect()
}

fn draw_cord(
    svg: &mut String,
    from: Point,
    to: Point,
    track: ConnectionTrack,
    information: String,
    detail: &str,
) {
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
    let label = match track {
        ConnectionTrack::Payload => information,
        _ => format!("{} · {}", information, track_name(track)),
    };
    let label_width = (label.chars().count() as i32 * 7 + 18).clamp(90, 360);
    let label_x = (from.x + to.x) / 2;
    let label_y = (from.y + to.y) / 2;
    writeln!(svg, "<g><title>{} · {}</title><path class=\"cord-shadow\" d=\"{path}\"/><path class=\"{class}\" marker-end=\"url(#{marker})\" d=\"{path}\"/><rect class=\"cord-tag\" x=\"{}\" y=\"{}\" width=\"{label_width}\" height=\"22\" rx=\"6\"/><text class=\"cord-label\" text-anchor=\"middle\" x=\"{label_x}\" y=\"{}\">{}</text></g>", escape(detail), track_name(track), label_x - label_width / 2, label_y - 15, label_y, escape(&label)).unwrap();
}

fn information_label(kind: &str, port_name: &str) -> String {
    if kind.starts_with("structured-info/profile-") {
        port_name.replace('-', " ")
    } else {
        kind.rsplit('/').next().unwrap_or(kind).replace('-', " ")
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
                    information_label(port.value_kind.as_str(), port.port_id.as_str()),
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
                information_label(port.value_kind.as_str(), port.port_id.as_str()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for binding in &authoring.input_bindings {
        writeln!(
            output,
            "  form_in -- \"{}{}\" --> {}",
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
            "  {} -- \"{}{}\" --> form_out",
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
        assert!(svg.contains(">text/join</text>"));
        assert!(!svg.contains("class=\"port-kind\""));
        assert!(svg.contains("Form.shown · value/text · payload"));
        assert!(svg.contains("class=\"cord-tag\""));
        assert!(!svg.contains(">payload</text>"));
        assert!(svg.contains(".canvas{fill:#f5f1e8}"));
        assert!(svg.contains(".boundary{fill:#dfeee5"));
        assert!(svg.contains("fill=\"#1677a6\""));
        assert!(svg.contains("q0,-10 10,-10"));
        assert!(svg.contains("width=\"1010\""));
        assert!(!svg.contains("text/join(\"&\")"));
    }

    #[test]
    fn mermaid_labels_cords_once_with_information_kind() {
        let form = checked("form main (\n  >> text: Text\n  shown: Text >>\n) {\n  pass: text/join(\" \" )\n  text >> pass >> shown\n}\n");
        let mermaid = render_mermaid(&form);
        assert_eq!(mermaid.matches("-- \"text\" -->").count(), 2);
        assert!(!mermaid.contains("text → text"));
        assert!(mermaid.contains("<small>text/join</small>"));
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
