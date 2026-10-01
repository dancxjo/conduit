use crate::prelude::*;
use crate::{
    hash_string, AuthoringFrontBinding, CanonicalBackCatalog, CanonicalExpansionDiagnostic,
    CanonicalStartupValue, CheckedCanonicalForm, CheckedCanonicalGear, CheckedConnection,
    CheckedCordStage, CheckedGear, CheckedSyntaxDocument, ConfigurationValue, ExpandedActivation,
    ExpandedAuthoringForm, ExpandedCanonicalForm, ExpandedGearProvenance, ExpandedSharedPool,
    KindConfigurationRule, ProfileCatalog, RuntimePortDirection, MAXIMUM_FORM_NESTING_DEPTH,
};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{GearId, KindId, PortDescriptor};

mod entry;
mod graph;
mod identity;
mod literal;
mod retained;
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

#[derive(Debug, Clone)]
struct TrackedEndpoint {
    endpoint: Endpoint,
    track: conduit_core::ConnectionTrack,
}

impl TrackedEndpoint {
    fn payload(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            track: conduit_core::ConnectionTrack::Payload,
        }
    }
}

impl core::ops::Deref for TrackedEndpoint {
    type Target = Endpoint;

    fn deref(&self) -> &Self::Target {
        &self.endpoint
    }
}

#[derive(Debug)]
struct Fragment {
    gears: Vec<CheckedGear>,
    connections: Vec<CheckedConnection>,
    shared_pools: Vec<ExpandedSharedPool>,
    provenance: Vec<ExpandedGearProvenance>,
    activations: Vec<ExpandedActivation>,
    inputs: BTreeMap<String, Vec<TrackedEndpoint>>,
    outputs: BTreeMap<String, TrackedEndpoint>,
    abnormal: Option<TrackedEndpoint>,
    shorthand: Option<(String, String)>,
}

#[derive(Debug)]
struct Instance {
    inputs: BTreeMap<String, Vec<TrackedEndpoint>>,
    outputs: BTreeMap<String, TrackedEndpoint>,
    abnormal: Option<TrackedEndpoint>,
    bare_ports: Option<(Option<String>, Option<String>)>,
    terminal_transductions: Vec<conduit_core::TerminalTransductionProfile>,
}

#[derive(Debug, Clone)]
enum StageSource {
    Internal(TrackedEndpoint),
    FaceInput(
        String,
        conduit_core::KindId,
        conduit_core::PortTemporal,
        Option<conduit_core::KindId>,
        conduit_core::ConnectionTrack,
    ),
}

#[derive(Debug, Clone)]
enum StageSink {
    Internal(TrackedEndpoint),
    FaceOutput(
        String,
        conduit_core::KindId,
        conduit_core::PortTemporal,
        Option<conduit_core::KindId>,
    ),
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
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::FormBack>,
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
            format!("recursive form expansion cycle: {}", cycle.join(" > ")),
        ));
    }
    stack.push(form.name.clone());
    let result = expand_instance_inner(
        form,
        forms,
        structured_types,
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
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::FormBack>,
    depth: usize,
) -> Result<Fragment, CanonicalExpansionDiagnostic> {
    let scoped_environment = bind_pool_environment(form, environment, path)?;
    let environment = &scoped_environment;
    let mut gears = Vec::new();
    let mut connections = Vec::new();
    let mut shared_pools = expanded_pool_declarations(form, path);
    let mut provenance = Vec::new();
    let mut activations = Vec::new();
    let mut instances = BTreeMap::new();
    let mut gear_ids = BTreeSet::new();
    for gear in form.gears.iter().filter(|gear| gear.name.is_some()) {
        let name = gear.name.as_deref().expect("named gears were filtered");
        let instance = instantiate_gear(
            gear,
            None,
            name,
            form,
            forms,
            structured_types,
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
            &mut activations,
            &mut gear_ids,
        )?;
        instances.insert(name.to_string(), instance);
    }

    let runtime_front = form.checked_front();
    let front_ports = checked_front_ports(form, &runtime_front);
    let mut inputs = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    let mut anonymous_counts = BTreeMap::<String, usize>::new();
    for cord in &form.cords {
        let mut pending = Vec::with_capacity(cord.stages.len());
        for cord_stage in &cord.stages {
            pending.push(match cord_stage {
                CheckedCordStage::Reference(reference) => structured_selector::PendingStage::Ready(
                    resolve_reference(reference, &instances, &front_ports)?,
                ),
                CheckedCordStage::RelationalGear {
                    operands,
                    gear,
                    input_ports,
                    output_port,
                } => {
                    let key = inline_key(gear);
                    let count = anonymous_counts.entry(key.clone()).or_default();
                    let name = format!("inline-{}-{count}", &hash_string(&key)[..12]);
                    *count += 1;
                    let instance = instantiate_gear(
                        gear,
                        Some(operands.len()),
                        &name,
                        form,
                        forms,
                        structured_types,
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
                        &mut activations,
                        &mut gear_ids,
                    )?;
                    for (operand, input_port) in operands.iter().zip(input_ports) {
                        let source = resolve_reference(operand, &instances, &front_ports)?
                            .output
                            .ok_or_else(|| {
                                CanonicalExpansionDiagnostic::new(
                                    "CND-FRM-036",
                                    format!("glyph operand '{operand}' has no output"),
                                )
                            })?;
                        let sinks = instance.inputs.get(input_port).ok_or_else(|| {
                            CanonicalExpansionDiagnostic::new(
                                "CND-FRM-043",
                                format!(
                                    "glyph Gear '{}' has no checked input port '{}'",
                                    gear.kind, input_port
                                ),
                            )
                        })?;
                        for sink in sinks {
                            connect(
                                source.clone(),
                                StageSink::Internal(sink.clone()),
                                &mut connections,
                                &mut inputs,
                                &mut outputs,
                            )?;
                        }
                    }
                    let output = instance.outputs.get(output_port).cloned().ok_or_else(|| {
                        CanonicalExpansionDiagnostic::new(
                            "CND-FRM-043",
                            format!(
                                "glyph Gear '{}' has no checked output port '{}'",
                                gear.kind, output_port
                            ),
                        )
                    })?;
                    structured_selector::PendingStage::Ready(Stage {
                        input: None,
                        output: Some(StageSource::Internal(output)),
                    })
                }
                CheckedCordStage::TerminalProjection {
                    endpoint,
                    terminal,
                    source_span,
                } => structured_selector::PendingStage::Ready(project_terminal(
                    resolve_terminal_reference(endpoint, *terminal, &instances, &front_ports)?,
                    *terminal,
                    *source_span,
                )?),
                CheckedCordStage::Cancellation { gear, source_span } => {
                    structured_selector::PendingStage::Ready(cancellation_sink(
                        gear,
                        &instances,
                        *source_span,
                    )?)
                }
                CheckedCordStage::When {
                    expression,
                    source_span,
                } => structured_selector::PendingStage::When {
                    expression: expression.clone(),
                    source_span: *source_span,
                },
                CheckedCordStage::PureExpression {
                    expression,
                    source_span,
                } => structured_selector::PendingStage::Expression {
                    expression: expression.clone(),
                    source_span: *source_span,
                },
                CheckedCordStage::InlineGear(gear) => {
                    let key = inline_key(gear);
                    let count = anonymous_counts.entry(key.clone()).or_default();
                    let name = format!("inline-{}-{count}", &hash_string(&key)[..12]);
                    *count += 1;
                    let instance = instantiate_gear(
                        gear,
                        None,
                        &name,
                        form,
                        forms,
                        structured_types,
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
                        &mut activations,
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
                        structured_types,
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
            structured_types,
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
    validate_front_bindings(form, &inputs, &outputs)?;
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
    let abnormal = infer_abnormal_export(&gears, &connections, &outputs)?;
    Ok(Fragment {
        gears,
        connections,
        shared_pools,
        provenance,
        activations,
        inputs,
        outputs,
        abnormal,
        shorthand: form.shorthand.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
fn instantiate_gear(
    gear: &CheckedCanonicalGear,
    relational_input_arity: Option<usize>,
    instance_name: &str,
    source_form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::FormBack>,
    depth: usize,
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
    shared_pools: &mut Vec<ExpandedSharedPool>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    activations: &mut Vec<ExpandedActivation>,
    gear_ids: &mut BTreeSet<GearId>,
) -> Result<Instance, CanonicalExpansionDiagnostic> {
    let mut child_path = path.to_vec();
    child_path.push(instance_name.to_string());
    if let Some(activation) = &gear.activation {
        let initial_accumulator = activation
            .initial_accumulator
            .as_ref()
            .map(|initial| substitute(initial, environment))
            .transpose()?;
        let initial_accumulator_bytes = match (
            activation.accumulator_input.as_ref(),
            initial_accumulator.as_ref(),
        ) {
            (Some(accumulator), Some(initial)) => Some(canonical_initial_bytes(
                accumulator.value_kind.as_str(),
                initial,
            )?),
            _ => None,
        };
        let child = forms
            .get(activation.selected_form.as_str())
            .copied()
            .ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-062",
                    "activation selected source Form disappeared after checking".into(),
                )
            })?;
        let gear_id = GearId::from(child_path.join("/"));
        if !gear_ids.insert(gear_id.clone()) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-038",
                format!("expanded gear path '{}' is not unique", gear_id.as_str()),
            ));
        }
        let mut input = activation.input.clone();
        input.temporal = conduit_core::PortTemporal::Flow { closes: true };
        let mut output = match activation.mode {
            crate::ActivationSyntax::Each { .. } => activation.output.clone(),
            crate::ActivationSyntax::Select { .. } => {
                let [output] = source_form.runtime_front.outputs() else {
                    return Err(CanonicalExpansionDiagnostic::new(
                        "CND-FRM-063",
                        "select coordinator requires one exact retained-item output".into(),
                    ));
                };
                if output.value_kind != activation.input.value_kind
                    || output.abnormal_kind != activation.input.abnormal_kind
                {
                    return Err(CanonicalExpansionDiagnostic::new(
                        "CND-FRM-063",
                        "select output must retain the exact predicate input item and abnormal truth"
                            .into(),
                    ));
                }
                output.clone()
            }
            crate::ActivationSyntax::Fold { .. } | crate::ActivationSyntax::Scan { .. } => {
                activation.output.clone()
            }
        };
        output.temporal = if matches!(activation.mode, crate::ActivationSyntax::Fold { .. }) {
            conduit_core::PortTemporal::Value
        } else {
            conduit_core::PortTemporal::Flow { closes: true }
        };
        let activation_id = format!("{}/activation", gear_id.as_str());
        gears.push(crate::checked_gear_from_parts! {
            gear_id: gear_id.clone(),
            kind_id: KindId::from(match activation.mode { crate::ActivationSyntax::Each { .. } => "flow/each", crate::ActivationSyntax::Select { .. } => "flow/select", crate::ActivationSyntax::Fold { .. } => "flow/fold", crate::ActivationSyntax::Scan { .. } => "flow/scan" }),
            kind_contract_revision: conduit_core::KindIdentity::from(match activation.mode { crate::ActivationSyntax::Each { .. } => "conduit.flow/each@1", crate::ActivationSyntax::Select { .. } => "conduit.flow/select@1", crate::ActivationSyntax::Fold { .. } => "conduit.flow/fold@1", crate::ActivationSyntax::Scan { .. } => "conduit.flow/scan@1" }),
            startup_parameters: Vec::new(),
            shorthand: Some((input.port_id.clone(), output.port_id.clone())),
            inputs: vec![input.clone()],
            outputs: vec![output.clone()],
            semantic_contract: match &activation.mode {
                crate::ActivationSyntax::Each { maximum_items } => conduit_core::flow_each_activation_contract(&activation.input_contract, &activation.output_contract, activation.abnormal_contract.as_ref(), input.port_id.clone(), output.port_id.clone(), *maximum_items),
                crate::ActivationSyntax::Select { maximum_items } => conduit_core::flow_select_activation_contract(&activation.input_contract, activation.abnormal_contract.as_ref(), input.port_id.clone(), output.port_id.clone(), *maximum_items),
                crate::ActivationSyntax::Fold { maximum_items, .. } => conduit_core::flow_fold_activation_contract(&activation.input_contract, activation.accumulator_contract.as_ref().expect("checked fold accumulator contract"), initial_accumulator_bytes.clone().expect("expanded fold initial bytes"), activation.abnormal_contract.as_ref(), input.port_id.clone(), output.port_id.clone(), conduit_core::port_id("accumulator"), conduit_core::port_id("item"), conduit_core::port_id("combined"), *maximum_items),
                crate::ActivationSyntax::Scan { maximum_items, .. } => conduit_core::flow_scan_activation_contract(&activation.input_contract, activation.accumulator_contract.as_ref().expect("checked scan accumulator contract"), initial_accumulator_bytes.clone().expect("expanded scan initial bytes"), activation.abnormal_contract.as_ref(), input.port_id.clone(), output.port_id.clone(), conduit_core::port_id("accumulator"), conduit_core::port_id("item"), conduit_core::port_id("combined"), *maximum_items),
            },
            terminal_transductions: Vec::new(),
            resource_ports: Vec::new(),
            configuration: Vec::new(),
            pool_references: Vec::new(),
        });
        provenance.push(ExpandedGearProvenance {
            gear_id: gear_id.as_str().to_string(),
            form_path: path.to_vec(),
            source_form: source_form.name.clone(),
            source_gear: instance_name.to_string(),
            source_span: gear.source_span,
        });
        activations.push(ExpandedActivation {
            activation_id,
            owner_gear_id: gear_id.clone(),
            mode: activation.mode.clone(),
            selected_form: activation.selected_form.clone(),
            selected_checked_form_id: child.checked_form_id.clone(),
            input: activation.input.clone(),
            accumulator_input: activation.accumulator_input.clone(),
            output: activation.output.clone(),
            initial_accumulator,
            initial_accumulator_bytes,
            input_contract: activation.input_contract.clone(),
            output_contract: activation.output_contract.clone(),
            abnormal_contract: activation.abnormal_contract.clone(),
            accumulator_contract: activation.accumulator_contract.clone(),
            source_span: gear.source_span,
        });
        return Ok(Instance {
            inputs: BTreeMap::from([(
                input.port_id.as_str().to_string(),
                vec![TrackedEndpoint::payload(Endpoint {
                    gear_id: gear_id.clone(),
                    port: input.clone(),
                })],
            )]),
            outputs: BTreeMap::from([(
                output.port_id.as_str().to_string(),
                TrackedEndpoint::payload(Endpoint {
                    gear_id,
                    port: output.clone(),
                }),
            )]),
            abnormal: output.abnormal_kind.as_ref().map(|_| {
                TrackedEndpoint::payload(Endpoint {
                    gear_id: GearId::from(child_path.join("/")),
                    port: output.clone(),
                })
            }),
            bare_ports: Some((
                Some(input.port_id.as_str().to_string()),
                Some(output.port_id.as_str().to_string()),
            )),
            terminal_transductions: Vec::new(),
        });
    }
    if let Some(child) = forms.get(gear.kind.as_str()).copied() {
        let child_environment = bind_child_environment(gear, environment)?;
        let fragment = expand_instance(
            child,
            forms,
            structured_types,
            catalog,
            backs,
            &child_environment,
            &child_path,
            stack,
            realization_backs,
            depth + 1,
        )?;
        gear_ids.extend(fragment.gears.iter().map(|op| op.gear_id.clone()));
        let abnormal = fragment.abnormal;
        gears.extend(fragment.gears);
        connections.extend(fragment.connections);
        shared_pools.extend(fragment.shared_pools);
        provenance.extend(fragment.provenance);
        activations.extend(fragment.activations);
        return Ok(Instance {
            inputs: fragment.inputs,
            outputs: fragment.outputs,
            abnormal,
            bare_ports: fragment
                .shorthand
                .map(|(input, output)| (Some(input), Some(output))),
            terminal_transductions: Vec::new(),
        });
    }

    if let Some(retained) = gear.retained.as_deref() {
        let gear_id = GearId::from(child_path.join("/"));
        if let Some((state, input, output)) =
            retained::initialized_structured_state(retained, gear_id.clone())?
        {
            if !gear_ids.insert(gear_id.clone()) {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-038",
                    format!("expanded gear path '{}' is not unique", gear_id.as_str()),
                ));
            }
            gears.push(state);
            provenance.push(ExpandedGearProvenance {
                gear_id: gear_id.as_str().to_string(),
                form_path: path.to_vec(),
                source_form: source_form.name.clone(),
                source_gear: instance_name.to_string(),
                source_span: gear.source_span,
            });
            return Ok(Instance {
                inputs: BTreeMap::from([(
                    "next".to_string(),
                    vec![TrackedEndpoint::payload(Endpoint {
                        gear_id: gear_id.clone(),
                        port: input,
                    })],
                )]),
                outputs: BTreeMap::from([(
                    "current".to_string(),
                    TrackedEndpoint::payload(Endpoint {
                        gear_id,
                        port: output,
                    }),
                )]),
                abnormal: None,
                bare_ports: Some((Some("next".into()), Some("current".into()))),
                terminal_transductions: Vec::new(),
            });
        }
        let value_kind = retained.value_kind.clone();
        if matches!(
            value_kind.as_str(),
            conduit_core::DISTANCE_INFO_ID | conduit_core::FREQUENCY_INFO_ID
        ) {
            let gear_id = GearId::from(child_path.join("/"));
            if !gear_ids.insert(gear_id.clone()) {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-038",
                    format!("expanded gear path '{}' is not unique", gear_id.as_str()),
                ));
            }
            let expected_dimension = if value_kind.as_str() == conduit_core::DISTANCE_INFO_ID {
                conduit_core::QuantityDimension::Length
            } else {
                conduit_core::QuantityDimension::Frequency
            };
            let mut configuration = vec![
                conduit_core::ConfigurationEntry {
                    key: "retained-duration".into(),
                    value: conduit_core::ConfigurationValue::Text(
                        match retained.duration {
                            crate::RetainedDuration::Step => "step",
                            crate::RetainedDuration::Play => "play",
                            crate::RetainedDuration::Wake => "wake",
                            crate::RetainedDuration::Boot => "boot",
                            crate::RetainedDuration::Body => "body",
                        }
                        .into(),
                    ),
                },
                conduit_core::ConfigurationEntry {
                    key: "maximum-bytes".into(),
                    value: conduit_core::ConfigurationValue::U64(
                        retained
                            .maximum_bytes
                            .unwrap_or(conduit_core::QUANTITY_ENCODED_LEN as u64),
                    ),
                },
            ];
            if retained
                .maximum_bytes
                .is_some_and(|maximum| maximum < conduit_core::QUANTITY_ENCODED_LEN as u64)
            {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-041",
                    format!(
                        "KEEP '{}' bound is smaller than its {}-byte canonical quantity encoding",
                        value_kind.as_str(),
                        conduit_core::QUANTITY_ENCODED_LEN
                    ),
                ));
            }
            if let Some(initial) = retained.initial.as_ref() {
                let CanonicalStartupValue::Quantity(quantity) = initial else {
                    return Err(CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        format!(
                            "KEEP '{}' initializer is not an exact quantity literal",
                            value_kind.as_str()
                        ),
                    ));
                };
                if quantity.dimension() != expected_dimension {
                    return Err(CanonicalExpansionDiagnostic::new(
                        "CND-FRM-040",
                        format!(
                            "KEEP '{}' initializer has the wrong quantity dimension",
                            value_kind.as_str()
                        ),
                    ));
                }
                configuration.push(conduit_core::ConfigurationEntry {
                    key: "initial".into(),
                    value: conduit_core::ConfigurationValue::Quantity(*quantity),
                });
            }
            let input = PortDescriptor {
                port_id: conduit_core::port_id("in"),
                value_kind: value_kind.clone(),
                direction: conduit_core::PortDirection::Input,
                // A retained declaration accepts each admitted value occurrence; upstream
                // flow-to-value lifting remains explicit in the ordinary cord checker.
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            };
            let output = PortDescriptor {
                port_id: conduit_core::port_id("out"),
                value_kind: value_kind.clone(),
                direction: conduit_core::PortDirection::Output,
                temporal: conduit_core::PortTemporal::Current,
                abnormal_kind: None,
            };
            gears.push(crate::checked_gear_from_parts! {
                gear_id: gear_id.clone(),
                kind_id: KindId::from("state/latest"),
                kind_contract_revision: conduit_core::KindIdentity::from(
                    "conduit.form/dimensioned-retained-current@1",
                ),
                startup_parameters: gear.startup_parameters.clone(),
                shorthand: Some((input.port_id.clone(), output.port_id.clone())),
                inputs: vec![input.clone()],
                outputs: vec![output.clone()],
                semantic_contract: conduit_core::KindSemanticContract::default(),
                terminal_transductions: Vec::new(),
                resource_ports: Vec::new(),
                configuration,
                pool_references: Vec::new(),
            });
            provenance.push(ExpandedGearProvenance {
                gear_id: gear_id.as_str().to_string(),
                form_path: path.to_vec(),
                source_form: source_form.name.clone(),
                source_gear: instance_name.to_string(),
                source_span: gear.source_span,
            });
            return Ok(Instance {
                inputs: BTreeMap::from([(
                    "in".to_string(),
                    vec![TrackedEndpoint::payload(Endpoint {
                        gear_id: gear_id.clone(),
                        port: input,
                    })],
                )]),
                outputs: BTreeMap::from([(
                    "out".to_string(),
                    TrackedEndpoint::payload(Endpoint {
                        gear_id,
                        port: output,
                    }),
                )]),
                abnormal: None,
                bare_ports: Some((Some("in".into()), Some("out".into()))),
                terminal_transductions: Vec::new(),
            });
        }
    }

    let kind_id = KindId::from(gear.kind.as_str());
    let specialized_definition;
    let definition = if let Some(input_count) = relational_input_arity {
        specialized_definition = catalog
            .projection_for_arity(&kind_id, input_count)
            .map_err(|message| CanonicalExpansionDiagnostic::new("CND-FRM-043", message))?;
        specialized_definition.as_ref().ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-037",
                format!("primitive gear '{}' has no planning contract", gear.kind),
            )
        })?
    } else {
        if catalog.is_homogeneous_variadic(&kind_id) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-043",
                format!(
                    "variadic Gear '{}' requires an exact relational operand count",
                    gear.kind
                ),
            ));
        }
        catalog.get(&kind_id).ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-037",
                format!("primitive gear '{}' has no planning contract", gear.kind),
            )
        })?
    };
    let semantic_kind = catalog
        .canonical_kind_for_arity(&kind_id, relational_input_arity)
        .map_err(|message| CanonicalExpansionDiagnostic::new("CND-FRM-043", message))?;
    let terminal_transductions = semantic_kind
        .as_ref()
        .map(|kind| kind.terminal_transductions().cloned().collect())
        .unwrap_or_default();
    if let Some(back) = backs.get(&kind_id) {
        let mut selected = back.realization.clone();
        selected.invocation_path = child_path.join("/");
        realization_backs.push(selected);
        let child_environment = bind_child_environment(gear, environment)?;
        let fragment = expand_instance(
            &back.form,
            forms,
            structured_types,
            catalog,
            backs,
            &child_environment,
            &child_path,
            stack,
            realization_backs,
            depth + 1,
        )?;
        gear_ids.extend(fragment.gears.iter().map(|op| op.gear_id.clone()));
        let abnormal = fragment.abnormal;
        gears.extend(fragment.gears);
        connections.extend(fragment.connections);
        shared_pools.extend(fragment.shared_pools);
        provenance.extend(fragment.provenance);
        return Ok(Instance {
            inputs: fragment.inputs,
            outputs: fragment.outputs,
            abnormal,
            bare_ports: fragment
                .shorthand
                .map(|(input, output)| (Some(input), Some(output))),
            terminal_transductions,
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
    gears.push(crate::checked_gear_from_parts! {
        gear_id: gear_id.clone(),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        startup_parameters: gear.startup_parameters.clone(),
        shorthand: match (definition.inputs.as_slice(), definition.outputs.as_slice()) {
            ([input], [output]) => Some((input.port_id.clone(), output.port_id.clone())),
            _ => None,
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        semantic_contract: semantic_kind
            .as_ref()
            .map(conduit_core::Kind::semantic_contract)
            .unwrap_or_else(|| conduit_core::KindSemanticContract {
                configuration: definition.configuration.clone(),
                laws: Vec::new(),
            }),
        terminal_transductions: terminal_transductions.clone(),
        resource_ports: semantic_kind
            .as_ref()
            .map(conduit_core::Kind::resource_ports)
            .unwrap_or_default()
            .to_vec(),
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
    let abnormal = definition
        .outputs
        .iter()
        .filter(|port| port.abnormal_kind.is_some())
        .map(|port| TrackedEndpoint {
            endpoint: Endpoint {
                gear_id: gear_id.clone(),
                port: port.clone(),
            },
            track: conduit_core::ConnectionTrack::AbnormalTerminal,
        })
        .collect::<Vec<_>>();
    Ok(Instance {
        inputs: definition
            .inputs
            .iter()
            .map(|port| {
                (
                    port.port_id.as_str().to_string(),
                    vec![TrackedEndpoint::payload(Endpoint {
                        gear_id: gear_id.clone(),
                        port: port.clone(),
                    })],
                )
            })
            .collect(),
        outputs: definition
            .outputs
            .iter()
            .map(|port| {
                (
                    port.port_id.as_str().to_string(),
                    TrackedEndpoint::payload(Endpoint {
                        gear_id: gear_id.clone(),
                        port: port.clone(),
                    }),
                )
            })
            .collect(),
        abnormal: (abnormal.len() == 1).then(|| abnormal[0].clone()),
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
        terminal_transductions,
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
        CanonicalStartupValue::Literal(_) | CanonicalStartupValue::Quantity(_) => Ok(value.clone()),
        CanonicalStartupValue::Structured(value) if value.try_concrete().is_some() => {
            Ok(CanonicalStartupValue::Structured(value.clone()))
        }
        CanonicalStartupValue::Structured(_) => Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-039",
            "structured startup value remains unresolved".into(),
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

fn canonical_initial_bytes(
    kind: &str,
    value: &CanonicalStartupValue,
) -> Result<Vec<u8>, CanonicalExpansionDiagnostic> {
    let bytes = match value {
        CanonicalStartupValue::Structured(value) => value
            .try_concrete()
            .and_then(|value| value.canonical_bytes().ok()),
        CanonicalStartupValue::Quantity(value) => Some(value.encode().to_vec()),
        CanonicalStartupValue::Literal(literal) => match conduit_core::primitive_info_kind(kind) {
            Some(conduit_core::PrimitiveInfoKind::Unit) if literal == "unit" => Some(Vec::new()),
            Some(conduit_core::PrimitiveInfoKind::Bool) => match literal.as_str() {
                "true" => Some(conduit_core::InfoBool::TRUE.encode().to_vec()),
                "false" => Some(conduit_core::InfoBool::FALSE.encode().to_vec()),
                _ => None,
            },
            Some(conduit_core::PrimitiveInfoKind::Text) => {
                crate::text_value::parse_quoted_text(literal).map(String::into_bytes)
            }
            Some(conduit_core::PrimitiveInfoKind::Count)
            | Some(conduit_core::PrimitiveInfoKind::U64) => literal
                .parse::<u64>()
                .ok()
                .map(u64::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::U8) => {
                literal.parse::<u8>().ok().map(|v| vec![v])
            }
            Some(conduit_core::PrimitiveInfoKind::U16) => literal
                .parse::<u16>()
                .ok()
                .map(u16::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::U32) => literal
                .parse::<u32>()
                .ok()
                .map(u32::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::U128) => literal
                .parse::<u128>()
                .ok()
                .map(u128::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::I8) => {
                literal.parse::<i8>().ok().map(|v| vec![v as u8])
            }
            Some(conduit_core::PrimitiveInfoKind::I16) => literal
                .parse::<i16>()
                .ok()
                .map(i16::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::I32) => literal
                .parse::<i32>()
                .ok()
                .map(i32::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::I64) => literal
                .parse::<i64>()
                .ok()
                .map(i64::to_le_bytes)
                .map(Vec::from),
            Some(conduit_core::PrimitiveInfoKind::I128) => literal
                .parse::<i128>()
                .ok()
                .map(i128::to_le_bytes)
                .map(Vec::from),
            _ => None,
        },
        CanonicalStartupValue::FormParameter(_) | CanonicalStartupValue::PoolReference(_) => None,
    }
    .ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-064",
            "fold initial accumulator has no exact canonical encoding".into(),
        )
    })?;
    if conduit_core::primitive_info_kind(kind).is_some() {
        conduit_core::validate_primitive_info(kind, &bytes).map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-064",
                "fold initial accumulator differs from its exact Value contract".into(),
            )
        })?;
    }
    Ok(bytes)
}
