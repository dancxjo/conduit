//! Build-Host structural calibration through the ordinary Planner and lowering.
//! Descriptive template inputs establish no Ready provider, resource custody,
//! current member, remote approval or permission to execute the resulting work.
use conduit_core as c;
use conduit_embedded_build as e;
use conduit_planner as p;
use std::collections::BTreeMap;

pub struct Inputs<'a> {
    /// Preserve the checked external Fore as well as the expanded Gear graph.
    pub plot: &'a conduit_plot::ExpandedAuthoringPlot,
    pub host: &'a c::HostAdvertisement,
    pub placements: &'a p::PlacementChoices,
    pub planning: p::PlanningOptions<'a>,
    pub connections: &'a BTreeMap<p::ConnectionEndpoints, p::ConnectionQueueLimits>,
    pub boundaries: &'a BTreeMap<p::ForeBoundaryKey, p::ConnectionQueueLimits>,
    pub resident: &'a [&'a e::PreparedResidentExpression],
    /// Exact descriptive definitions for every selected non-resident Gear,
    /// including pure software. These are not initialized native handles.
    pub other: &'a [&'a c::CapabilityOffer],
    pub image: e::EmbeddedImageBounds,
    pub computations: e::EmbeddedComputationBounds,
    pub maxima: c::AssignedPlanMaxima,
    pub maximum_fore_ports: usize,
}

pub struct Layout {
    /// Inert canonical identity program, not a sealed current-Boot fragment.
    pub fragment_seal_recipe: c::FragmentSealRecipe,
    /// Canonical placement ordinal to actual lowered kernel node, derived by
    /// exact Gear identity; never inferred from Source-list or kernel order.
    pub fragment_seal_to_kernel: Vec<u16>,
    pub fragment_seal_placement_ids: Vec<String>,
    /// Checked calibration bytes and Boot-independent static digest. These are
    /// inert construction material; fresh sealing/admission remain mandatory.
    pub projection_template: Vec<u8>,
    pub projection_template_digest: [u8; 32],
    pub maxima: c::AssignedPlanMaxima,
    pub ports_per_node: usize,
    pub sign_items: u16,
    pub sign_bytes: u32,
    pub host_call_contracts: Vec<c::AssignedIdentity>,
    pub resource_ordinals: Vec<u16>,
    pub computations: Vec<c::AssignedComputationRequirement>,
    pub maximum_computation_text_bytes: u32,
    pub maximum_total_computation_text_bytes: u32,
    /// Payload reservation for every actual checked Cord item. Not a grant.
    pub cord_value_slot_capacities: Vec<u32>,
    /// Additional generated-output staging, one slot per routed output port.
    pub step_value_slot_capacities: Vec<u32>,
    /// Native results may exceed their outgoing typed Cord representation.
    pub host_call_value_slot_capacities: Vec<u32>,
    pub records: Vec<c::AssignedRecordRequirement>,
    /// Every exact configuration record, in ordinary encoded order.
    pub configurations: Vec<c::AssignedRecordRequirement>,
    pub counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
    /// Kernel ordinal to original checked Source Gear, never source-list guesses.
    pub nodes: Vec<(u16, c::GearId)>,
    pub fore: Vec<c::AssignedForeRequirement>,
}

/// Independently generate the immutable receiver layout. Current runtime
/// assignments still need signed containment, original finite admission and
/// actual local provider/Fore possession; this helper issues none of those.
pub fn compile(input: Inputs<'_>) -> Result<Layout, String> {
    if !input.planning.line_offers.is_empty()
        || !input.planning.connection_bases.is_empty()
        || !input.planning.line_candidates.is_empty()
        || input
            .placements
            .by_gear
            .values()
            .any(|p| p.host_id != input.host.host_id)
    {
        return Err("structural template requires one complete local Host without Lines".into());
    }
    let plan = p::plan_expanded_authoring_with_connection_limits(
        input.plot,
        std::slice::from_ref(input.host),
        input.placements,
        &[c::LOCAL_BASE_IMPLEMENTATION_ID.into()],
        input.planning,
        input.connections,
        input.boundaries,
    )
    .map_err(|e| format!("structural planning: {e:?}"))?;
    if plan.fragments.len() != 1 {
        return Err("structural template must lower one complete local fragment".into());
    }
    let plot_seal = c::PlainPlotPlanSeal::capture(&plan)
        .map_err(|e| format!("canonical Plot Plan seal eligibility: {e:?}"))?;
    let fragment = &plan.fragments[0];
    let digest = plot_seal.digest(&[c::PlotFragmentSeal {
        host_id: fragment.host_id.as_str(), fragment_id: fragment.fragment_id.as_str(),
    }]).map_err(|e| format!("canonical Plot Plan commitment seal: {e:?}"))?;
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    if hex != plan.plan_id.as_str() {
        return Err("canonical Plot Plan commitment differs from ordinary seal".into());
    }
    let seal_limits = c::FragmentSealRecipe::required_limits(fragment)
        .map_err(|e| format!("canonical fragment recipe measurement: {e:?}"))?;
    let fragment_seal_recipe = c::FragmentSealRecipe::capture(fragment, seal_limits)
        .map_err(|e| format!("canonical fragment recipe capture: {e:?}"))?;
    let resident = e::bind_current_mixed_resident_expressions(
        &plan,
        &fragment.fragment_id,
        input.host,
        input.resident,
        input.other,
    )
    .map_err(|e| format!("resident Source correspondence: {e:?}"))?;
    let generated = e::generate_fore_native_plan(
        &plan,
        &fragment.fragment_id,
        input.image,
        input.computations,
        input.maximum_fore_ports,
    )
    .map_err(|e| format!("native lowering: {e:?}"))?;
    let bytes = generated
        .encode_with_resident_computations(input.maxima, &resident)
        .map_err(|e| format!("assigned encoding: {e:?}"))?;
    let projection_template_digest =
        c::assigned_local_projection_template_digest(&bytes, input.maxima).map_err(debug)?;
    let records = generated
        .layout_requirements_with_resident_computations(input.maxima, &resident)
        .map_err(debug)?;
    let mut configurations = Vec::new();
    let mut counts = [0_u8; c::ASSIGNED_PLAN_COUNT_KINDS];
    for record in c::AssignedPlanRecords::new(&bytes).map_err(debug)? {
        let record = record.map_err(debug)?;
        if record.tag == c::ASSIGNED_CONFIGURATION {
            configurations.push(c::AssignedRecordRequirement::from_record(record));
        }
        let count = counts
            .get_mut(
                usize::from(record.tag)
                    .checked_sub(1)
                    .ok_or("invalid generated record tag")?,
            )
            .ok_or("generated record count width")?;
        *count = count
            .checked_add(1)
            .ok_or("generated record count overflow")?;
    }
    let nodes = generated
        .kernel()
        .nodes
        .iter()
        .map(|node| {
            let placement = fragment
                .placements
                .iter()
                .find(|p| p.placement_id.as_str() == node.placement_id)
                .ok_or("generated node lost original Source placement")?;
            Ok((node.node, placement.gear_id.clone()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let fragment_seal_to_kernel = fragment
        .placements
        .iter()
        .map(|placement| {
            nodes
                .iter()
                .find(|(_, gear)| gear == &placement.gear_id)
                .map(|(node, _)| *node)
                .ok_or("canonical placement lost lowered Gear")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let kernel = generated.kernel();
    let mut cord_value_slot_capacities = Vec::new();
    for cord in &kernel.cords {
        cord_value_slot_capacities.extend(std::iter::repeat_n(
            cord.maximum_value_bytes.max(1),
            usize::from(cord.item_capacity),
        ));
    }
    // A prepared Step can materialize all routed outputs before its atomic
    // delivery retires input values. These are distinct from queue occupancy.
    let mut step_value_slot_capacities = Vec::new();
    for output in &kernel.output_ports {
        let maximum = kernel
            .cords
            .iter()
            .filter_map(|cord| match cord.source {
                e::GeneratedCordEndpoint::Local { node, port }
                    if node == output.node && port == output.port =>
                {
                    Some(cord.maximum_value_bytes)
                }
                _ => None,
            })
            .max();
        if let Some(maximum) = maximum {
            // HostedValueStore's physical slots are nonempty even for the
            // intrinsic zero-byte Empty ABI; logical Cord bounds stay zero.
            step_value_slot_capacities.push(maximum.max(1));
        }
    }
    let mut host_call_value_slot_capacities = Vec::new();
    for call in &kernel.host_calls {
        if call.maximum_output_bytes > 0 {
            host_call_value_slot_capacities.extend(std::iter::repeat_n(
                call.maximum_output_bytes,
                usize::from(call.maximum_in_flight),
            ));
        }
    }
    let mut computations = Vec::new();
    for configuration in &kernel.configuration {
        if matches!(configuration.key.as_str(), "program" | "selector" | "reference") {
            let e::GeneratedConfigurationValue::Text(text) = &configuration.value else {
                return Err("non-text checked computation".into());
            };
            computations.push(
                c::AssignedComputationRequirement::from_text(
                    configuration.node,
                    &configuration.key,
                    text,
                )
                .map_err(debug)?,
            );
        }
    }
    Ok(Layout {
        fragment_seal_recipe,
        fragment_seal_to_kernel,
        fragment_seal_placement_ids: fragment.placements.iter().map(|p| p.placement_id.as_str().to_owned()).collect(),
        projection_template: bytes,
        projection_template_digest,
        maxima: input.maxima,
        ports_per_node: kernel
            .input_ports
            .iter()
            .chain(&kernel.output_ports)
            .map(|port| usize::from(port.port) + 1)
            .max()
            .unwrap_or(1),
        sign_items: kernel.sign_items,
        sign_bytes: kernel.sign_bytes,
        host_call_contracts: kernel
            .host_calls
            .iter()
            .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
            .collect(),
        resource_ordinals: kernel.resources.iter().map(|r| r.resource).collect(),
        computations,
        maximum_computation_text_bytes: input
            .computations
            .maximum_program_bytes
            .checked_mul(2)
            .ok_or("computation ceiling overflow")?,
        maximum_total_computation_text_bytes: input
            .computations
            .maximum_total_program_bytes
            .checked_mul(2)
            .ok_or("computation ceiling overflow")?,
        cord_value_slot_capacities,
        step_value_slot_capacities,
        host_call_value_slot_capacities,
        records,
        configurations,
        counts,
        nodes,
        fore: generated.fore().to_vec(),
    })
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("structural template: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_resident_fore_lowers_independently_and_preserves_source_node() {
        let checked = crate::checked_dht11_tm1637_source().unwrap();
        let plot = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "dht11-capture-request",
            &conduit_plot::ProfileCatalog::new(),
        )
        .unwrap();
        let gear = &plot.expanded.gears[0];
        let c::ConfigurationValue::Text(program) = &gear.configuration[0].value else {
            panic!("checked expression program");
        };
        let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
        let image = e::PreparedResidentExpression::prepare(
            &checked,
            &program,
            gear.inputs[0].temporal,
            e::ResidentExpressionBounds {
                native: conduit_plot::rust_binding::PreparedNativeBounds {
                    canonical_bytes: 4096,
                    scopes: 64,
                    invariants: 64,
                    contracts: 64,
                },
                maximum_occurrences: 4096,
                maximum_program_bytes: 512 * 1024,
                maximum_generated_bytes: 1024 * 1024,
                private_work_units: 1113,
            },
        )
        .unwrap();
        let host = c::HostAdvertisement {
            declared_name: None,
            protocol_version: c::PROTOCOL_VERSION,
            host_id: "template/descriptive-host".into(),
            boot_id: "template/descriptive-boot".into(),
            offer_generation: c::OfferGeneration(1),
            profile: "template/pure-resident".into(),
            bases: vec![],
            resources: vec![c::ResourceOffer {
                pool_id: "template/reviewed-work".into(),
                class_id: c::COOPERATIVE_STEP_WORK_CLASS_ID.into(),
                capacity_units: 1113,
                compute: None,
                content: None,
            }],
            planner_capabilities: vec![],
            capabilities: vec![image.offer_description().clone()],
        };
        let placements = p::PlacementChoices {
            by_gear: [(
                gear.gear_id.clone(),
                p::PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: image.offer_description().capability_id.clone(),
                },
            )]
            .into(),
        };
        let queue_bytes = image.offer_description().limits.max_queue_bytes;
        let mut boundaries = BTreeMap::new();
        for (direction, bindings) in [
            (c::PortDirection::Input, &plot.input_bindings),
            (c::PortDirection::Output, &plot.output_bindings),
        ] {
            for binding in bindings {
                boundaries.insert(
                    p::ForeBoundaryKey {
                        direction,
                        front_port_id: binding.front_port_id.clone(),
                        track: binding.track,
                    },
                    p::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: queue_bytes,
                    },
                );
            }
        }
        let empty = BTreeMap::new();
        let connections = BTreeMap::new();
        let residents = [&image];
        let make = |host, boundaries| {
            compile(Inputs {
                plot: &plot,
                host,
                placements: &placements,
                planning: p::PlanningOptions {
                    connection_bases: &empty,
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: queue_bytes,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                connections: &connections,
                boundaries,
                resident: &residents,
                other: &[],
                image: e::EmbeddedImageBounds {
                    maximum_nodes: 1,
                    maximum_remote_endpoints: 0,
                    ..e::EmbeddedImageBounds::HOST_TOOLING
                },
                computations: e::EmbeddedComputationBounds {
                    maximum_program_bytes: 512 * 1024,
                    maximum_total_program_bytes: 512 * 1024,
                },
                maxima: c::AssignedPlanMaxima::TINY_HOST,
                maximum_fore_ports: 2,
            })
        };
        let layout = make(&host, &boundaries).unwrap();
        let mut restarted = host.clone();
        restarted.boot_id = "template/later-boot".into();
        restarted.offer_generation = c::OfferGeneration(2);
        let later = make(&restarted, &boundaries).unwrap();
        assert_eq!(layout.nodes, later.nodes);
        assert_eq!(layout.fragment_seal_placement_ids, later.fragment_seal_placement_ids);
        assert_eq!(c::fragment_seal_recipe_identity(layout.fragment_seal_recipe.pieces()), c::fragment_seal_recipe_identity(later.fragment_seal_recipe.pieces()));
        assert_eq!(layout.counts, later.counts);
        assert_eq!(
            layout.projection_template_digest,
            later.projection_template_digest
        );
        c::AssignedLocalProjectionTemplate::check(
            &layout.projection_template,
            &layout.projection_template_digest,
            layout.maxima,
        )
        .unwrap();
        assert_eq!(layout.records, later.records);
        assert_eq!(layout.configurations, later.configurations);
        assert!(!layout.configurations.is_empty());
        assert!(layout
            .configurations
            .iter()
            .all(|r| r.tag == c::ASSIGNED_CONFIGURATION));
        assert_eq!(layout.nodes, vec![(0, gear.gear_id.clone())]);
        assert_eq!(layout.counts[usize::from(c::ASSIGNED_NODE - 1)], 1);
        assert_eq!(layout.fore.len(), 2);
        assert!(!layout.records.is_empty());
        assert!(layout.records.iter().all(|r| !matches!(
            r.tag,
            c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
        )));
        assert!(
            make(&host, &BTreeMap::new()).is_err(),
            "missing independently selected Fore refuses"
        );
    }
}
