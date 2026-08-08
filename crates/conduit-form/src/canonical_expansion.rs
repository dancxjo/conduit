use crate::{
    hash_string, CanonicalExpansionDiagnostic, CanonicalStartupValue, CheckedCanonicalCell,
    CheckedCanonicalForm, CheckedConnection, CheckedCordStage, CheckedOperation,
    CheckedSyntaxDocument, ConfigurationRule, ConfigurationValue, ExpandedCanonicalForm,
    ExpandedCellProvenance, ProfileCatalog, RuntimePortDirection, MAXIMUM_FORM_NESTING_DEPTH,
};
use conduit_core::{KindId, OperationId, PortDescriptor};
use std::collections::{BTreeMap, BTreeSet};

mod graph;
mod identity;
use graph::*;
use identity::{expanded_identity, provenance_digest};

pub fn expand_canonical_form(
    document: &CheckedSyntaxDocument,
    form_name: &str,
    catalog: &ProfileCatalog,
) -> Result<ExpandedCanonicalForm, CanonicalExpansionDiagnostic> {
    let forms = document
        .forms
        .iter()
        .map(|form| (form.name.as_str(), form))
        .collect::<BTreeMap<_, _>>();
    let form = forms.get(form_name).copied().ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-031",
            format!("canonical form '{form_name}' is not defined"),
        )
    })?;
    let mut environment = BTreeMap::new();
    for parameter in &form.startup_parameters {
        let value = parameter.default.clone().ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-032",
                format!(
                    "root form '{form_name}' requires startup parameter '{}'",
                    parameter.name
                ),
            )
        })?;
        environment.insert(parameter.name.clone(), value);
    }
    let mut stack = Vec::new();
    let fragment = expand_instance(
        form,
        &forms,
        catalog,
        &environment,
        std::slice::from_ref(&form.name),
        &mut stack,
        0,
    )?;
    if !fragment.inputs.is_empty() || !fragment.outputs.is_empty() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-033",
            format!("root form '{form_name}' has unbound runtime face ports"),
        ));
    }
    let mut operations = fragment.operations;
    let mut connections = fragment.connections;
    let mut provenance = fragment.provenance;
    operations.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
    connections.sort_by(|left, right| {
        (
            &left.source_operation_id,
            &left.source_port_id,
            &left.sink_operation_id,
            &left.sink_port_id,
        )
            .cmp(&(
                &right.source_operation_id,
                &right.source_port_id,
                &right.sink_operation_id,
                &right.sink_port_id,
            ))
    });
    provenance.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
    let expanded_form_id = expanded_identity(form, &operations, &connections, &provenance);
    let provenance_digest = provenance_digest(&document.source_document_id, &provenance);
    Ok(ExpandedCanonicalForm {
        source_document_id: document.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
        expanded_form_id,
        name: form.name.clone(),
        operations,
        connections,
        provenance,
        provenance_digest,
    })
}

#[derive(Debug, Clone)]
struct Endpoint {
    operation_id: OperationId,
    port: PortDescriptor,
}

#[derive(Debug)]
struct Fragment {
    operations: Vec<CheckedOperation>,
    connections: Vec<CheckedConnection>,
    provenance: Vec<ExpandedCellProvenance>,
    inputs: BTreeMap<String, Vec<Endpoint>>,
    outputs: BTreeMap<String, Endpoint>,
    shorthand: Option<(String, String)>,
}

#[derive(Debug)]
struct Instance {
    inputs: BTreeMap<String, Vec<Endpoint>>,
    outputs: BTreeMap<String, Endpoint>,
    bare_ports: Option<(Option<String>, Option<String>)>,
}

#[derive(Debug, Clone)]
enum StageSource {
    Internal(Endpoint),
    FaceInput(String, String),
}

#[derive(Debug, Clone)]
enum StageSink {
    Internal(Endpoint),
    FaceOutput(String, String),
}

#[derive(Debug)]
struct Stage {
    input: Option<Vec<StageSink>>,
    output: Option<StageSource>,
}

fn expand_instance(
    form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    catalog: &ProfileCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    depth: usize,
) -> Result<Fragment, CanonicalExpansionDiagnostic> {
    if depth > MAXIMUM_FORM_NESTING_DEPTH {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-034",
            format!("expanded form exceeds the {MAXIMUM_FORM_NESTING_DEPTH}-level bound"),
        ));
    }
    if stack.iter().any(|name| name == &form.name) {
        let mut cycle = stack.clone();
        cycle.push(form.name.clone());
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-035",
            format!("recursive form expansion cycle: {}", cycle.join(" -> ")),
        ));
    }
    stack.push(form.name.clone());
    let result = expand_instance_inner(form, forms, catalog, environment, path, stack, depth);
    stack.pop();
    result
}

fn expand_instance_inner(
    form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    catalog: &ProfileCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    depth: usize,
) -> Result<Fragment, CanonicalExpansionDiagnostic> {
    let mut operations = Vec::new();
    let mut connections = Vec::new();
    let mut provenance = Vec::new();
    let mut instances = BTreeMap::new();
    let mut operation_ids = BTreeSet::new();
    for cell in form.cells.iter().filter(|cell| cell.name.is_some()) {
        let name = cell.name.as_deref().expect("named cells were filtered");
        let instance = instantiate_cell(
            cell,
            name,
            form,
            forms,
            catalog,
            environment,
            path,
            stack,
            depth,
            &mut operations,
            &mut connections,
            &mut provenance,
            &mut operation_ids,
        )?;
        instances.insert(name.to_string(), instance);
    }

    let face_ports = form
        .runtime_ports
        .iter()
        .map(|port| (port.name.text.as_str(), port))
        .collect::<BTreeMap<_, _>>();
    let mut inputs = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    let mut anonymous_counts = BTreeMap::<String, usize>::new();
    for cord in &form.cords {
        let mut stages = Vec::with_capacity(cord.stages.len());
        for cord_stage in &cord.stages {
            stages.push(match cord_stage {
                CheckedCordStage::Reference(reference) => {
                    resolve_reference(reference, &instances, &face_ports)?
                }
                CheckedCordStage::InlineCell(cell) => {
                    let key = inline_key(cell);
                    let count = anonymous_counts.entry(key.clone()).or_default();
                    let name = format!("inline-{}-{count}", &hash_string(&key)[..12]);
                    *count += 1;
                    let instance = instantiate_cell(
                        cell,
                        &name,
                        form,
                        forms,
                        catalog,
                        environment,
                        path,
                        stack,
                        depth,
                        &mut operations,
                        &mut connections,
                        &mut provenance,
                        &mut operation_ids,
                    )?;
                    stage_for_instance(&name, &instance, None)?
                }
            });
        }
        for pair in stages.windows(2) {
            let source = pair[0].output.clone().ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-036",
                    "cord stage has no output; use an explicit output port".into(),
                )
            })?;
            let sinks = pair[1].input.clone().ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-036",
                    "cord stage has no input; use an explicit input port".into(),
                )
            })?;
            for sink in sinks {
                connect(
                    source.clone(),
                    sink,
                    &mut connections,
                    &mut inputs,
                    &mut outputs,
                )?;
            }
        }
    }
    validate_face_bindings(form, &inputs, &outputs)?;
    if let Some((input, output)) = &form.shorthand {
        if !inputs.contains_key(input) || !outputs.contains_key(output) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-044",
                format!(
                    "form '{}' shorthand path does not bind its declared input and output",
                    form.name
                ),
            ));
        }
    }
    Ok(Fragment {
        operations,
        connections,
        provenance,
        inputs,
        outputs,
        shorthand: form.shorthand.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
fn instantiate_cell(
    cell: &CheckedCanonicalCell,
    instance_name: &str,
    source_form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    catalog: &ProfileCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    depth: usize,
    operations: &mut Vec<CheckedOperation>,
    connections: &mut Vec<CheckedConnection>,
    provenance: &mut Vec<ExpandedCellProvenance>,
    operation_ids: &mut BTreeSet<OperationId>,
) -> Result<Instance, CanonicalExpansionDiagnostic> {
    let mut child_path = path.to_vec();
    child_path.push(instance_name.to_string());
    if let Some(child) = forms.get(cell.operation.as_str()).copied() {
        let child_environment = bind_child_environment(cell, environment)?;
        let fragment = expand_instance(
            child,
            forms,
            catalog,
            &child_environment,
            &child_path,
            stack,
            depth + 1,
        )?;
        operation_ids.extend(fragment.operations.iter().map(|op| op.operation_id.clone()));
        operations.extend(fragment.operations);
        connections.extend(fragment.connections);
        provenance.extend(fragment.provenance);
        return Ok(Instance {
            inputs: fragment.inputs,
            outputs: fragment.outputs,
            bare_ports: fragment
                .shorthand
                .map(|(input, output)| (Some(input), Some(output))),
        });
    }

    let kind_id = KindId::from(cell.operation.as_str());
    let definition = catalog.get(&kind_id).ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-037",
            format!(
                "primitive operation '{}' has no planning contract",
                cell.operation
            ),
        )
    })?;
    let operation_id = OperationId::from(child_path.join("/"));
    if !operation_ids.insert(operation_id.clone()) {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-038",
            format!(
                "expanded operation path '{}' is not unique",
                operation_id.as_str()
            ),
        ));
    }
    let configuration = configuration(cell, environment, definition)?;
    operations.push(CheckedOperation {
        operation_id: operation_id.clone(),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        configuration,
    });
    provenance.push(ExpandedCellProvenance {
        operation_id: operation_id.as_str().to_string(),
        form_path: path.to_vec(),
        source_form: source_form.name.clone(),
        source_cell: instance_name.to_string(),
        source_span: cell.source_span,
    });
    Ok(Instance {
        inputs: definition
            .inputs
            .iter()
            .map(|port| {
                (
                    port.port_id.as_str().to_string(),
                    vec![Endpoint {
                        operation_id: operation_id.clone(),
                        port: port.clone(),
                    }],
                )
            })
            .collect(),
        outputs: definition
            .outputs
            .iter()
            .map(|port| {
                (
                    port.port_id.as_str().to_string(),
                    Endpoint {
                        operation_id: operation_id.clone(),
                        port: port.clone(),
                    },
                )
            })
            .collect(),
        bare_ports: if definition.inputs.len() <= 1 && definition.outputs.len() <= 1 {
            Some((
                definition
                    .inputs
                    .first()
                    .map(|port| port.port_id.as_str().to_string()),
                definition
                    .outputs
                    .first()
                    .map(|port| port.port_id.as_str().to_string()),
            ))
        } else {
            None
        },
    })
}

fn bind_child_environment(
    cell: &CheckedCanonicalCell,
    parent: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<BTreeMap<String, CanonicalStartupValue>, CanonicalExpansionDiagnostic> {
    cell.startup_bindings
        .iter()
        .map(|binding| {
            let value = substitute(&binding.value, parent)?;
            Ok((binding.name.clone(), value))
        })
        .collect()
}

fn substitute(
    value: &CanonicalStartupValue,
    environment: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<CanonicalStartupValue, CanonicalExpansionDiagnostic> {
    match value {
        CanonicalStartupValue::Literal(_) => Ok(value.clone()),
        CanonicalStartupValue::FormParameter(name) => {
            environment.get(name).cloned().ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-039",
                    format!("back references undeclared outer startup value '{name}'"),
                )
            })
        }
    }
}
