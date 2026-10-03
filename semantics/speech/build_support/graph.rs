//! Static composition of an exact acyclic checked pure-expression graph.
//! Preparation resolves each wire once; generated code has no graph scheduler.
use super::lower;
use conduit_core::{ConfigurationValue, StructuredInfoType};
use conduit_plot::{ExpandedAuthoringPlot, PortableExpressionProgram};

pub struct Lowered {
    pub source: String,
    pub programs: Vec<(String, String)>,
    pub graph: Vec<(usize, String)>,
    pub result: usize,
}

pub fn function(
    name: &str,
    plot: &ExpandedAuthoringPlot,
    types: &[(StructuredInfoType, String)],
) -> Result<Lowered, String> {
    let expanded = &plot.expanded;
    if expanded.gears.is_empty() || expanded.gears.len() > 256 {
        return Err("compiled graph must have 1..=256 expression gears".into());
    }
    if !expanded.activations.is_empty()
        || !expanded.shared_pools.is_empty()
        || plot.abnormal_export.is_some()
        || plot.front.inputs().len() != 1
        || plot.front.outputs().len() != 1
        || plot.output_bindings.len() != 1
    {
        return Err("only unary finite pure-expression composition is eligible".into());
    }
    let mut programs = Vec::new();
    let mut encoded_programs = Vec::new();
    for gear in &expanded.gears {
        if gear.kind_contract_revision.as_str() != conduit_plot::PURE_EXPRESSION_REVISION
            || gear.inputs.len() != 1
            || gear.outputs.len() != 1
            || !gear.resource_ports.is_empty()
            || !gear.pool_references.is_empty()
            || gear.inputs[0].temporal != conduit_core::PortTemporal::Value
            || gear.outputs[0].temporal != conduit_core::PortTemporal::Value
        {
            return Err("graph contains a non-expression or non-unary gear".into());
        }
        let [entry] = gear.configuration.as_slice() else {
            return Err("expression configuration must be exact".into());
        };
        let ConfigurationValue::Text(encoded) = &entry.value else {
            return Err("expression program must be text".into());
        };
        if entry.key != "program" {
            return Err("expression program configuration key differs".into());
        }
        programs.push(
            PortableExpressionProgram::from_canonical_hex(encoded).map_err(|e| format!("{e:?}"))?,
        );
        encoded_programs.push(encoded.clone());
    }
    let mut inputs = vec![None; expanded.gears.len()];
    let mut root_type = None;
    for binding in &plot.input_bindings {
        let index = expanded
            .gears
            .iter()
            .position(|gear| gear.gear_id == binding.gear_id)
            .ok_or("front gear absent")?;
        if binding.track != conduit_core::ConnectionTrack::Payload
            || binding.front_port_id != plot.front.inputs()[0].port_id
            || binding.gear_port_id != expanded.gears[index].inputs[0].port_id
            || inputs[index].is_some()
        {
            return Err("front input binding is not exact".into());
        }
        if root_type
            .as_ref()
            .is_some_and(|known| known != &programs[index].input_type)
        {
            return Err("front fan-out has differing exact types".into());
        }
        root_type = Some(programs[index].input_type.clone());
        inputs[index] = Some(usize::MAX);
    }
    for cord in &expanded.connections {
        let source = expanded
            .gears
            .iter()
            .position(|gear| gear.gear_id == cord.source_gear_id)
            .ok_or("source absent")?;
        let sink = expanded
            .gears
            .iter()
            .position(|gear| gear.gear_id == cord.sink_gear_id)
            .ok_or("sink absent")?;
        if cord.track != conduit_core::ConnectionTrack::Payload
            || cord.temporal != conduit_core::PortTemporal::Value
            || cord.source_port_id != expanded.gears[source].outputs[0].port_id
            || cord.sink_port_id != expanded.gears[sink].inputs[0].port_id
            || inputs[sink].is_some()
            || programs[source].output_type != programs[sink].input_type
        {
            return Err("cord does not join exact unary types and ports".into());
        }
        inputs[sink] = Some(source);
    }
    let binding = &plot.output_bindings[0];
    let result = expanded
        .gears
        .iter()
        .position(|gear| gear.gear_id == binding.gear_id)
        .ok_or("output absent")?;
    if binding.track != conduit_core::ConnectionTrack::Payload
        || binding.front_port_id != plot.front.outputs()[0].port_id
        || binding.gear_port_id != expanded.gears[result].outputs[0].port_id
    {
        return Err("front output binding is not exact".into());
    }
    let root_type = root_type.ok_or("no front input binding")?;
    let mut source = String::new();
    let mut recorded = Vec::new();
    for (index, program) in programs.iter().enumerate() {
        let member = if programs.len() == 1 {
            name.into()
        } else {
            format!("{name}_step_{index}")
        };
        source.push_str(&lower::function(&member, program, types)?);
        recorded.push((member, encoded_programs[index].clone()));
    }
    let mut available = vec![false; programs.len()];
    let mut graph = Vec::new();
    let mut ordinals = vec![0; programs.len()];
    let mut body = String::new();
    while graph.len() < programs.len() {
        let index = (0..programs.len())
            .find(|index| {
                !available[*index]
                    && inputs[*index].is_some_and(|input| input == usize::MAX || available[input])
            })
            .ok_or("cycle or unbound graph input")?;
        let input = inputs[index].unwrap();
        let argument = if input == usize::MAX {
            "input".into()
        } else {
            format!("step{input}")
        };
        body.push_str(&format!(
            "let step{index} = {name}_step_{index}({argument})?;\n"
        ));
        ordinals[index] = graph.len();
        graph.push((
            if input == usize::MAX {
                usize::MAX
            } else {
                ordinals[input]
            },
            encoded_programs[index].clone(),
        ));
        available[index] = true;
    }
    if programs.len() > 1 {
        source.push_str(&format!(
            "pub fn {name}(input: {}) -> Option<{}> {{ {body} Some(step{result}) }}\n",
            lower::ty(&root_type, types)?,
            lower::ty(&programs[result].output_type, types)?
        ));
    }
    Ok(Lowered {
        source,
        programs: recorded,
        graph,
        result: ordinals[result],
    })
}

/// Rust symbol spelling only; semantic and checked identities remain exact.
pub fn symbol(name: &str) -> String {
    let mut result: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if result.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        result.insert_str(0, "plot_");
    }
    result
}
