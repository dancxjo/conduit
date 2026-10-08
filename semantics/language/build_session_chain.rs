//! Retain the complete checked, single-input/output pure Source composition.
//! This is build-time topology admission, never a runtime scheduler.
use conduit_core::{ConfigurationValue, ConnectionTrack};
use conduit_plot::{ExpandedAuthoringPlot, PortableExpressionProgram};
use std::{collections::BTreeSet, fs, path::Path};

pub fn retain(expanded: &ExpandedAuthoringPlot, file: &Path) {
    let plot = &expanded.expanded;
    assert!((1..=64).contains(&plot.gears.len()), "finite Source chain");
    assert!(plot.activations.is_empty() && plot.shared_pools.is_empty());
    assert!(expanded.abnormal_export.is_none());
    let [input] = expanded.input_bindings.as_slice() else {
        panic!("one Source input")
    };
    let [output] = expanded.output_bindings.as_slice() else {
        panic!("one Source output")
    };
    assert_eq!(input.track, ConnectionTrack::Payload);
    assert_eq!(output.track, ConnectionTrack::Payload);
    assert_eq!(
        plot.connections.len(),
        plot.gears.len() - 1,
        "exact chain cords"
    );
    let mut current = &input.gear_id;
    let mut used = BTreeSet::new();
    let mut programs = String::new();
    let mut material = format!(
        "fixed Source chain v1\nsource={}\nchecked={}\nexpanded={}\ninput={}:{}\noutput={}:{}\n",
        plot.source_document_id.as_str(),
        plot.checked_plot_id.as_str(),
        plot.expanded_plot_id.as_str(),
        input.gear_id.as_str(),
        input.gear_port_id.as_str(),
        output.gear_id.as_str(),
        output.gear_port_id.as_str()
    );
    let mut previous_output = None;
    loop {
        assert!(used.insert(current.as_str()), "acyclic Source chain");
        let gear = plot
            .gears
            .iter()
            .find(|g| &g.gear_id == current)
            .expect("exact gear");
        assert_eq!(
            gear.kind_contract_revision.as_str(),
            "conduitese/pure-expression-operation@1"
        );
        assert!(gear
            .kind_id
            .as_str()
            .starts_with("conduitese/pure-expression/"));
        let [input_port] = gear.inputs.as_slice() else {
            panic!("one expression input")
        };
        let [output_port] = gear.outputs.as_slice() else {
            panic!("one expression output")
        };
        assert!(gear.resource_ports.is_empty() && gear.pool_references.is_empty());
        let [entry] = gear.configuration.as_slice() else {
            panic!("one exact program")
        };
        assert_eq!(entry.key, "program");
        let ConfigurationValue::Text(program_hex) = &entry.value else {
            panic!("exact encoded program")
        };
        let program =
            PortableExpressionProgram::from_canonical_hex(program_hex).expect("checked program");
        if let Some(previous) = previous_output.take() {
            assert_eq!(previous, program.input_type);
        }
        previous_output = Some(program.output_type);
        programs.push_str(program_hex);
        programs.push('\n');
        material.push_str(&format!(
            "gear={}:{}:{}:{}:{}\n",
            current.as_str(),
            gear.kind_id.as_str(),
            gear.kind_contract_revision.as_str(),
            input_port.port_id.as_str(),
            output_port.port_id.as_str()
        ));
        if used.len() == 1 {
            assert_eq!(input.gear_port_id, input_port.port_id);
        }
        let outgoing: Vec<_> = plot
            .connections
            .iter()
            .filter(|c| &c.source_gear_id == current)
            .collect();
        if current == &output.gear_id {
            assert!(outgoing.is_empty());
            assert_eq!(output.gear_port_id, output_port.port_id);
            break;
        }
        let [cord] = outgoing.as_slice() else {
            panic!("exactly one next Source cord")
        };
        assert_eq!(cord.track, ConnectionTrack::Payload);
        assert_eq!(cord.source_port_id, output_port.port_id);
        let sink = plot
            .gears
            .iter()
            .find(|g| g.gear_id == cord.sink_gear_id)
            .expect("sink gear");
        let [sink_port] = sink.inputs.as_slice() else {
            panic!("one sink input")
        };
        assert_eq!(cord.sink_port_id, sink_port.port_id);
        assert_eq!(cord.value_kind, output_port.value_kind);
        assert_eq!(cord.value_kind, sink_port.value_kind);
        material.push_str(&format!(
            "cord={}:{}>{}:{}:{}\n",
            cord.source_gear_id.as_str(),
            cord.source_port_id.as_str(),
            cord.sink_gear_id.as_str(),
            cord.sink_port_id.as_str(),
            cord.value_kind.as_str()
        ));
        current = &cord.sink_gear_id;
    }
    assert_eq!(used.len(), plot.gears.len(), "every expanded gear retained");
    fs::write(file, programs).expect("retain exact ordered programs");
    fs::write(file.with_extension("custody"), material).expect("retain original topology custody");
}
