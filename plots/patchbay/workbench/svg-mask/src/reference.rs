//! Accessible reference facts retained by canonical checking and expansion.

use alloc::{format, string::String};
use conduit_core::PortDescriptor;
use conduit_plot::{CheckedGear, ExpandedAuthoringPlot};
use core::fmt::Write as _;

use crate::escape;

pub(super) fn write_description(svg: &mut String, authoring: &ExpandedAuthoringPlot) {
    svg.push_str("<desc id=\"desc\">Checked Conduit Plot with exact typed port connections.\n");
    svg.push_str("Semantic contracts, not current Host availability or a Plan.\n");
    for gear in &authoring.expanded.gears {
        write_gear(svg, gear);
    }
    svg.push_str("</desc>\n");
}

fn write_gear(svg: &mut String, gear: &CheckedGear) {
    writeln!(
        svg,
        "Gear {}; Kind {}; contract {}.",
        escape(gear.gear_id.as_str()),
        escape(gear.kind_id.as_str()),
        escape(gear.kind_contract_revision.as_str())
    )
    .unwrap();
    for parameter in &gear.startup_parameters {
        writeln!(
            svg,
            "Startup {}: {}; {}.",
            escape(&parameter.name),
            escape(parameter.value_type.as_str()),
            if parameter.has_default {
                "has default"
            } else {
                "required"
            }
        )
        .unwrap();
    }
    for port in gear.inputs.iter().chain(&gear.outputs) {
        write_port(svg, port);
    }
    for field in &gear.semantic_contract.configuration {
        writeln!(
            svg,
            "Configuration {}: contract default {}; rule {}.",
            escape(&field.key),
            escape(&format!("{:?}", field.default_value)),
            escape(&format!("{:?}", field.rule))
        )
        .unwrap();
    }
    for entry in &gear.configuration {
        writeln!(
            svg,
            "Configured {} = {}.",
            escape(&entry.key),
            escape(&format!("{:?}", entry.value))
        )
        .unwrap();
    }
    for law in &gear.semantic_contract.laws {
        writeln!(svg, "Semantic law {}.", escape(&format!("{law:?}"))).unwrap();
    }
}

fn write_port(svg: &mut String, port: &PortDescriptor) {
    writeln!(
        svg,
        "Port {:?} {}: {}; temporal {:?}; abnormal {}.",
        port.direction,
        escape(port.port_id.as_str()),
        escape(port.value_kind.as_str()),
        port.temporal,
        escape(&format!("{:?}", port.abnormal_kind))
    )
    .unwrap();
}
