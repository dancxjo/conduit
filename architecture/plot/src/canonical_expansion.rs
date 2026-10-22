use crate::prelude::*;
use crate::{
    hash_string, AuthoringFrontBinding, CanonicalBackCatalog, CanonicalExpansionDiagnostic,
    CanonicalStartupValue, CheckedCanonicalGear, CheckedCanonicalPlot, CheckedConnection,
    CheckedCordStage, CheckedGear, CheckedSyntaxDocument, ConfigurationValue, ExpandedActivation,
    ExpandedAuthoringPlot, ExpandedCanonicalPlot, ExpandedGearProvenance, ExpandedSharedPool,
    ProfileCatalog, RuntimePortDirection, MAXIMUM_PLOT_NESTING_DEPTH,
};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{GearId, KindId, PortDescriptor};

mod connection_inventory;
use connection_inventory::ConnectionInventory;
mod connection;
use connection::connect;
mod connection_contract;
mod construction;
pub use connection_contract::{validate_connection_contract, validate_front_contract};
mod configuration_contract;
pub use configuration_contract::{validate_configuration_value, validate_startup_configuration};
mod entry;
mod graph;
mod instance;
mod qualified_front;
use instance::expand_instance_inner;
mod identity;
mod literal;
mod retained;
mod shared_pool;
mod startup;
mod structured_selector;
use crate::checked_syntax::provenance_digest;
pub use entry::{
    expand_canonical_plot, expand_canonical_plot_for_authoring,
    expand_canonical_plot_for_authoring_with_backs, expand_canonical_plot_with_backs,
};
use graph::*;
use identity::expanded_identity;
use shared_pool::{bind_pool_environment, expanded_pool_declarations, seal_pool_consumers};
use startup::{bind_child_environment, canonical_initial_bytes, substitute};
pub use structured_selector::structured_selector_definition;

#[derive(Debug, Clone)]
struct Endpoint {
    gear_id: GearId,
    port: PortDescriptor,
}

#[derive(Debug, Clone)]
struct TrackedEndpoint {
    source_origin: Option<crate::AuthoringCordOrigin>,
    endpoint: Endpoint,
    track: conduit_core::ConnectionTrack,
}

impl TrackedEndpoint {
    fn payload(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            track: conduit_core::ConnectionTrack::Payload,
            source_origin: None,
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
    connections: ConnectionInventory,
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
    plot: &CheckedCanonicalPlot,
    plots: &BTreeMap<&str, &CheckedCanonicalPlot>,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    exact_initial_info: &BTreeMap<(conduit_core::KindId, String), Vec<u8>>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::PlotBack>,
    depth: usize,
) -> Result<Fragment, CanonicalExpansionDiagnostic> {
    if depth > MAXIMUM_PLOT_NESTING_DEPTH {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-034",
            format!("expanded plot exceeds the {MAXIMUM_PLOT_NESTING_DEPTH}-level bound"),
        ));
    }
    // Local names and even a root's checked identity may coincide while its
    // private helper definitions differ. Detect recursion in the lexical scope.
    let scope = plots
        .values()
        .map(|plot| plot.checked_plot_id.as_str())
        .collect::<Vec<_>>()
        .join("|");
    let scoped_plot = format!("{}@{}", plot.name, hash_string(&scope));
    if stack.iter().any(|identity| identity == &scoped_plot) {
        let mut cycle = stack.clone();
        cycle.push(scoped_plot.clone());
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-035",
            format!(
                "recursive plot expansion cycle: {}",
                cycle
                    .iter()
                    .map(|entry| entry
                        .rsplit_once('@')
                        .map_or(entry.as_str(), |(name, _)| name))
                    .collect::<Vec<_>>()
                    .join(" > ")
            ),
        ));
    }
    stack.push(scoped_plot);
    let result = expand_instance_inner(
        plot,
        plots,
        structured_types,
        exact_initial_info,
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
fn instantiate_gear(
    gear: &CheckedCanonicalGear,
    relational_input_arity: Option<usize>,
    instance_name: &str,
    source_plot: &CheckedCanonicalPlot,
    plots: &BTreeMap<&str, &CheckedCanonicalPlot>,
    structured_types: &BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
    exact_initial_info: &BTreeMap<(conduit_core::KindId, String), Vec<u8>>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::PlotBack>,
    depth: usize,
    gears: &mut Vec<CheckedGear>,
    connections: &mut ConnectionInventory,
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
                exact_initial_info,
            )?),
            _ => None,
        };
        if initial_accumulator_bytes.as_ref().is_some_and(|bytes| {
            activation
                .accumulator_contract
                .as_ref()
                .is_none_or(|contract| contract.validate(bytes).is_err())
        }) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-064",
                "fold/scan initial Form differs from its exact accumulator Value contract".into(),
            ));
        }
        let child = plots
            .get(activation.selected_plot.as_str())
            .copied()
            .ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-062",
                    "activation selected source Plot disappeared after checking".into(),
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
                let [output] = source_plot.runtime_front.outputs() else {
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
            construction_owner: None,
            gear_id: gear_id.as_str().to_string(),
            plot_path: path.to_vec(),
            source_plot: source_plot.name.clone(),
            source_gear: instance_name.to_string(),
            source_span: gear.source_span,
        });
        activations.push(ExpandedActivation {
            activation_id,
            owner_gear_id: gear_id.clone(),
            mode: activation.mode.clone(),
            selected_plot: activation.selected_plot.clone(),
            selected_checked_plot_id: child.checked_plot_id.clone(),
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
    if let Some(child) = plots.get(gear.kind.as_str()).copied() {
        let child_environment = bind_child_environment(gear, environment)?;
        let fragment = expand_instance(
            child,
            plots,
            structured_types,
            exact_initial_info,
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
                construction_owner: None,
                gear_id: gear_id.as_str().to_string(),
                plot_path: path.to_vec(),
                source_plot: source_plot.name.clone(),
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
                if quantity.value().dimension() != expected_dimension {
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
                    value: conduit_core::ConfigurationValue::Quantity(quantity.clone()),
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
                    "conduit.plot/dimensioned-retained-current@1",
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
                construction_owner: None,
                gear_id: gear_id.as_str().to_string(),
                plot_path: path.to_vec(),
                source_plot: source_plot.name.clone(),
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
        let owner = backs.context(&kind_id).ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-037",
                "selected Source Back has no checked owner context".into(),
            )
        })?;
        let owner_plots = owner
            .document
            .plots
            .iter()
            .map(|plot| (plot.name.as_str(), plot))
            .collect();
        let mut owner_catalog = owner.catalog.as_ref().unwrap_or(catalog).clone();
        owner_catalog.physical = owner.document.physical.clone();
        owner_catalog.install_type_invariants(&owner.document.native_types);
        let mut fragment = expand_instance(
            &back.plot,
            &owner_plots,
            &owner.document.structured_types,
            &owner.document.exact_initial_info,
            &owner_catalog,
            backs,
            &child_environment,
            &child_path,
            stack,
            realization_backs,
            depth + 1,
        )?;
        fragment
            .connections
            .bind_document(&owner.document.source_document_id);
        for provenance in &mut fragment.provenance {
            if provenance.construction_owner.is_none() {
                provenance.construction_owner = Some(kind_id.clone());
            }
        }
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
        construction_owner: None,
        gear_id: gear_id.as_str().to_string(),
        plot_path: path.to_vec(),
        source_plot: source_plot.name.clone(),
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
            source_origin: None,
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
