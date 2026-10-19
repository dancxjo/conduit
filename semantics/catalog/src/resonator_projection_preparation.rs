//! Bounded preparation of the existing public resonator Source chain.
//! This evaluates pure expressions only; it is not an installed Host or Play.
use crate::DspSourcePackage;
use alloc::{format, string::String, vec::Vec};
use conduit_core::{
    ConfigurationValue, ConnectionTrack, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{expand_canonical_plot_for_authoring, PortableExpressionProgram};

pub struct ResonatorNumericExecution {
    program_hex: String,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl ResonatorNumericExecution {
    pub fn program_hex(&self) -> &str {
        &self.program_hex
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}
pub struct ResonatorNumericReceipt {
    original: StructuredInfoValue,
    canonical: Vec<u8>,
    candidate: StructuredInfoValue,
    outcome: StructuredInfoValue,
    executions: Vec<ResonatorNumericExecution>,
    source_id: String,
    checked_plot_id: String,
    expanded_plot_id: String,
}
impl ResonatorNumericReceipt {
    pub fn original(&self) -> &StructuredInfoValue {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.canonical
    }
    pub fn candidate(&self) -> &StructuredInfoValue {
        &self.candidate
    }
    pub fn outcome(&self) -> &StructuredInfoValue {
        &self.outcome
    }
    pub fn executions(&self) -> &[ResonatorNumericExecution] {
        &self.executions
    }
    pub fn source_id(&self) -> &str {
        &self.source_id
    }
    pub fn checked_plot_id(&self) -> &str {
        &self.checked_plot_id
    }
    pub fn expanded_plot_id(&self) -> &str {
        &self.expanded_plot_id
    }
}
pub struct PreparedResonatorNumericProjection {
    programs: Vec<(String, PortableExpressionProgram)>,
    source_id: String,
    checked_plot_id: String,
    expanded_plot_id: String,
}
impl PreparedResonatorNumericProjection {
    pub fn new() -> Result<Self, String> {
        let package = crate::prepare_dsp_source_package()?;
        Self::from_owned_package(&package)
    }
    fn from_owned_package(package: &DspSourcePackage) -> Result<Self, String> {
        let export = package
            .exports
            .resolve("audio/dsp/resonator-prepare")
            .ok_or_else(|| String::from("missing resonator public export"))?;
        let expanded = expand_canonical_plot_for_authoring(
            &package.checked,
            &export.name.text,
            &package.profiles,
        )
        .map_err(|e| format!("resonator expansion: {e:?}"))?;
        let graph = &expanded.expanded;
        if graph.gears.len() != 24
            || graph.connections.len() != 23
            || !graph.activations.is_empty()
            || !graph.shared_pools.is_empty()
            || expanded.input_bindings.len() != 1
            || expanded.output_bindings.len() != 1
            || expanded.input_bindings[0].track != ConnectionTrack::Payload
            || expanded.output_bindings[0].track != ConnectionTrack::Payload
        {
            return Err(String::from("unsupported resonator preparation graph"));
        }
        let mut id = expanded.input_bindings[0].gear_id.clone();
        let mut seen = Vec::with_capacity(24);
        let mut programs: Vec<(String, PortableExpressionProgram)> = Vec::with_capacity(24);
        loop {
            if seen.contains(&id) {
                return Err(String::from("cyclic resonator preparation"));
            }
            seen.push(id.clone());
            let gear = graph
                .gears
                .iter()
                .find(|g| g.gear_id == id)
                .ok_or_else(|| String::from("missing resonator Gear"))?;
            if !gear.resource_ports.is_empty()
                || !gear.pool_references.is_empty()
                || gear.configuration.len() != 1
                || gear.inputs.len() != 1
                || gear.outputs.len() != 1
            {
                return Err(String::from("unsupported resonator expression Fore"));
            }
            let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
                return Err(String::from("resonator expression program missing"));
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded)
                .map_err(|e| format!("resonator program: {e:?}"))?;
            if programs
                .last()
                .is_some_and(|(_, previous)| previous.output_type != program.input_type)
            {
                return Err(String::from("resonator chain Type mismatch"));
            }
            programs.push((encoded.clone(), program));
            let outgoing: Vec<_> = graph
                .connections
                .iter()
                .filter(|e| e.source_gear_id == id)
                .collect();
            if outgoing.is_empty() {
                if expanded.output_bindings[0].gear_id != id {
                    return Err(String::from("resonator output mismatch"));
                }
                break;
            }
            if outgoing.len() != 1 || outgoing[0].track != ConnectionTrack::Payload {
                return Err(String::from("unsupported resonator fanout/track"));
            }
            id = outgoing[0].sink_gear_id.clone();
        }
        if programs.len() != 24 {
            return Err(String::from("disconnected resonator preparation"));
        }
        Ok(Self {
            programs,
            source_id: graph.source_document_id.as_str().into(),
            checked_plot_id: graph.checked_plot_id.as_str().into(),
            expanded_plot_id: graph.expanded_plot_id.as_str().into(),
        })
    }
    pub fn input_type(&self) -> &StructuredInfoType {
        &self.programs[0].1.input_type
    }
    pub fn project(&self, canonical: &[u8]) -> Result<ResonatorNumericReceipt, String> {
        if canonical.is_empty() || canonical.len() > 65536 {
            return Err(String::from("resonator canonical bound"));
        }
        let original = StructuredInfoValue::from_canonical_bytes(canonical)
            .map_err(|e| format!("resonator original: {e:?}"))?;
        if original.value_type() != self.input_type() {
            return Err(String::from("foreign resonator input Type"));
        }
        let mut value = canonical.to_vec();
        let mut executions = Vec::with_capacity(24);
        for (hex, program) in &self.programs {
            let output = program
                .evaluate(&value)
                .map_err(|e| format!("resonator Source: {e:?}"))?;
            if output.len() > 65536 {
                return Err(String::from("resonator intermediate bound"));
            }
            executions.push(ResonatorNumericExecution {
                program_hex: hex.clone(),
                input: value,
                output: output.clone(),
            });
            value = output;
        }
        let outcome = StructuredInfoValue::from_canonical_bytes(&value)
            .map_err(|e| format!("resonator outcome: {e:?}"))?;
        if !matches!(
            outcome.shape(),
            StructuredInfoValueShape::Variant { tag: "started", .. }
        ) {
            return Err(String::from("resonator Source refused preparation"));
        }
        let candidate = StructuredInfoValue::from_canonical_bytes(&executions[23].input)
            .map_err(|e| format!("resonator candidate: {e:?}"))?;
        if field(&candidate, "request")? != &original {
            return Err(String::from("resonator original changed"));
        }
        Ok(ResonatorNumericReceipt {
            original,
            canonical: canonical.into(),
            candidate,
            outcome,
            executions,
            source_id: self.source_id.clone(),
            checked_plot_id: self.checked_plot_id.clone(),
            expanded_plot_id: self.expanded_plot_id.clone(),
        })
    }
}
pub(super) fn field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, String> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(String::from("resonator record expected"));
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value())
        .ok_or_else(|| format!("resonator missing {name}"))
}
pub(super) fn integer(value: &StructuredInfoValue, name: &str) -> Result<i128, String> {
    let StructuredInfoValueShape::Leaf(bytes) = field(value, name)?.shape() else {
        return Err(String::from("resonator integer expected"));
    };
    Ok(i128::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| String::from("resonator I128 expected"))?,
    ))
}
