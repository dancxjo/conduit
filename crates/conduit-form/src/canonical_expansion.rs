use crate::prelude::*;
use crate::{
    hash_string, AuthoringFaceBinding, CanonicalBackCatalog, CanonicalExpansionDiagnostic,
    CanonicalStartupValue, CheckedCanonicalForm, CheckedCanonicalGear, CheckedConnection,
    CheckedCordStage, CheckedGear, CheckedSyntaxDocument, ConfigurationRule, ConfigurationValue,
    ExpandedAuthoringForm, ExpandedCanonicalForm, ExpandedGearProvenance, ExpandedSharedPool,
    ProfileCatalog, RuntimePortDirection, MAXIMUM_FORM_NESTING_DEPTH,
};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{GearId, KindId, PortDescriptor};

mod entry;
mod graph;
mod identity;
mod literal;
mod shared_pool;
mod structured_selector;
pub use entry::{
    expand_canonical_form, expand_canonical_form_for_authoring,
    expand_canonical_form_for_authoring_with_backs, expand_canonical_form_with_backs,
};
use graph::*;
use identity::{expanded_identity, provenance_digest};
use shared_pool::{bind_pool_environment, expanded_pool_declarations, seal_pool_consumers};
pub use structured_selector::structured_selector_definition;

#[derive(Debug, Clone)]
struct Endpoint {
    gear_id: GearId,
    port: PortDescriptor,
}

#[derive(Debug)]
struct Fragment {
    gears: Vec<CheckedGear>,
    connections: Vec<CheckedConnection>,
    shared_pools: Vec<ExpandedSharedPool>,
    provenance: Vec<ExpandedGearProvenance>,
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
    FaceInput(String, conduit_core::KindId, conduit_core::PortTemporal),
}

#[derive(Debug, Clone)]
enum StageSink {
    Internal(Endpoint),
    FaceOutput(String, conduit_core::KindId, conduit_core::PortTemporal),
}

#[derive(Debug, Clone)]
struct Stage {
    input: Option<Vec<StageSink>>,
    output: Option<StageSource>,
}

#[allow(clippy::too_many_arguments)]
fn expand_instance(
    form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::RealizationBack>,
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
    let result = expand_instance_inner(
        form,
        forms,
        catalog,
        backs,
        environment,
        path,
        stack,
        realization_backs,
        depth,
    );
    stack.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn expand_instance_inner(
    form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::RealizationBack>,
    depth: usize,
) -> Result<Fragment, CanonicalExpansionDiagnostic> {
    let scoped_environment = bind_pool_environment(form, environment, path)?;
    let environment = &scoped_environment;
    let mut gears = Vec::new();
    let mut connections = Vec::new();
    let mut shared_pools = expanded_pool_declarations(form, path);
    let mut provenance = Vec::new();
    let mut instances = BTreeMap::new();
    let mut gear_ids = BTreeSet::new();
    for gear in form.gears.iter().filter(|gear| gear.name.is_some()) {
        let name = gear.name.as_deref().expect("named gears were filtered");
        let instance = instantiate_gear(
            gear,
            name,
            form,
            forms,
            catalog,
            backs,
            environment,
            path,
            stack,
            realization_backs,
            depth,
            &mut gears,
            &mut connections,
            &mut shared_pools,
            &mut provenance,
            &mut gear_ids,
        )?;
        instances.insert(name.to_string(), instance);
    }

    let runtime_face = form.checked_face();
    let face_ports = checked_face_ports(form, &runtime_face);
    let mut inputs = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    let mut anonymous_counts = BTreeMap::<String, usize>::new();
    for cord in &form.cords {
        let mut pending = Vec::with_capacity(cord.stages.len());
        for cord_stage in &cord.stages {
            pending.push(match cord_stage {
                CheckedCordStage::Reference(reference) => structured_selector::PendingStage::Ready(
                    resolve_reference(reference, &instances, &face_ports)?,
                ),
                CheckedCordStage::InlineGear(gear) => {
                    let key = inline_key(gear);
                    let count = anonymous_counts.entry(key.clone()).or_default();
                    let name = format!("inline-{}-{count}", &hash_string(&key)[..12]);
                    *count += 1;
                    let instance = instantiate_gear(
                        gear,
                        &name,
                        form,
                        forms,
                        catalog,
                        backs,
                        environment,
                        path,
                        stack,
                        realization_backs,
                        depth,
                        &mut gears,
                        &mut connections,
                        &mut shared_pools,
                        &mut provenance,
                        &mut gear_ids,
                    )?;
                    structured_selector::PendingStage::Ready(stage_for_instance(
                        &name, &instance, None,
                    )?)
                }
                CheckedCordStage::Literal { value, source_span } => {
                    structured_selector::PendingStage::Ready(literal::expand_literal(
                        value,
                        *source_span,
                        form,
                        forms,
                        catalog,
                        backs,
                        environment,
                        path,
                        stack,
                        realization_backs,
                        depth,
                        &mut gears,
                        &mut connections,
                        &mut shared_pools,
                        &mut provenance,
                        &mut gear_ids,
                        &mut anonymous_counts,
                    )?)
                }
                CheckedCordStage::StructuredSelector {
                    selector,
                    source_span,
                } => structured_selector::PendingStage::Selector {
                    selector: selector.clone(),
                    source_span: *source_span,
                },
            });
        }
        let stages = structured_selector::resolve_selectors(
            pending,
            form,
            forms,
            catalog,
            backs,
            environment,
            path,
            stack,
            realization_backs,
            depth,
            &mut gears,
            &mut connections,
            &mut shared_pools,
            &mut provenance,
            &mut gear_ids,
            &mut anonymous_counts,
        )?;
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
        gears,
        connections,
        shared_pools,
        provenance,
        inputs,
        outputs,
        shorthand: form.shorthand.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
fn instantiate_gear(
    gear: &CheckedCanonicalGear,
    instance_name: &str,
    source_form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::RealizationBack>,
    depth: usize,
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
    shared_pools: &mut Vec<ExpandedSharedPool>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
) -> Result<Instance, CanonicalExpansionDiagnostic> {
    let mut child_path = path.to_vec();
    child_path.push(instance_name.to_string());
    if let Some(child) = forms.get(gear.kind.as_str()).copied() {
        let child_environment = bind_child_environment(gear, environment)?;
        let fragment = expand_instance(
            child,
            forms,
            catalog,
            backs,
            &child_environment,
            &child_path,
            stack,
            realization_backs,
            depth + 1,
        )?;
        gear_ids.extend(fragment.gears.iter().map(|op| op.gear_id.clone()));
        gears.extend(fragment.gears);
        connections.extend(fragment.connections);
        shared_pools.extend(fragment.shared_pools);
        provenance.extend(fragment.provenance);
        return Ok(Instance {
            inputs: fragment.inputs,
            outputs: fragment.outputs,
            bare_ports: fragment
                .shorthand
                .map(|(input, output)| (Some(input), Some(output))),
        });
    }

    let kind_id = KindId::from(gear.kind.as_str());
    let definition = catalog.get(&kind_id).ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-037",
            format!("primitive gear '{}' has no planning contract", gear.kind),
        )
    })?;
    if let Some(back) = backs.get(&kind_id) {
        let mut selected = back.realization.clone();
        selected.invocation_path = child_path.join("/");
        realization_backs.push(selected);
        let child_environment = bind_child_environment(gear, environment)?;
        let fragment = expand_instance(
            &back.form,
            forms,
            catalog,
            backs,
            &child_environment,
            &child_path,
            stack,
            realization_backs,
            depth + 1,
        )?;
        gear_ids.extend(fragment.gears.iter().map(|op| op.gear_id.clone()));
        gears.extend(fragment.gears);
        connections.extend(fragment.connections);
        shared_pools.extend(fragment.shared_pools);
        provenance.extend(fragment.provenance);
        return Ok(Instance {
            inputs: fragment.inputs,
            outputs: fragment.outputs,
            bare_ports: fragment
                .shorthand
                .map(|(input, output)| (Some(input), Some(output))),
        });
    }
    let gear_id = GearId::from(child_path.join("/"));
    if !gear_ids.insert(gear_id.clone()) {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-038",
            format!("expanded gear path '{}' is not unique", gear_id.as_str()),
        ));
    }
    let configuration = configuration(gear, environment, definition)?;
    let pool_references = pool_references(gear, environment)?;
    gears.push(CheckedGear {
        gear_id: gear_id.clone(),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        startup_parameters: gear
            .startup_parameters
            .iter()
            .map(|parameter| conduit_core::FaceStartupParameter {
                name: parameter.name.clone(),
                value_type: parameter.value_type.clone(),
                has_default: parameter.default.is_some(),
            })
            .collect(),
        shorthand: match (definition.inputs.as_slice(), definition.outputs.as_slice()) {
            ([input], [output]) => Some((input.port_id.clone(), output.port_id.clone())),
            _ => None,
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        configuration,
        pool_references,
    });
    provenance.push(ExpandedGearProvenance {
        gear_id: gear_id.as_str().to_string(),
        form_path: path.to_vec(),
        source_form: source_form.name.clone(),
        source_gear: instance_name.to_string(),
        source_span: gear.source_span,
    });
    Ok(Instance {
        inputs: definition
            .inputs
            .iter()
            .map(|port| {
                (
                    port.port_id.as_str().to_string(),
                    vec![Endpoint {
                        gear_id: gear_id.clone(),
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
                        gear_id: gear_id.clone(),
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
    gear: &CheckedCanonicalGear,
    parent: &BTreeMap<String, CanonicalStartupValue>,
) -> Result<BTreeMap<String, CanonicalStartupValue>, CanonicalExpansionDiagnostic> {
    gear.startup_bindings
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
        CanonicalStartupValue::Structured(_) => Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-039",
            "structured startup propagation is not implemented in this Form layer".into(),
        )),
        CanonicalStartupValue::PoolReference(pool) => environment
            .get(pool.as_str())
            .cloned()
            .map_or_else(|| Ok(value.clone()), Ok),
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
