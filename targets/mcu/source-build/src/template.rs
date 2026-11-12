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

#[path = "template_continuation.rs"]
mod continuation;
pub use continuation::ContinuationPins;

#[path = "host_call_storage.rs"]
mod host_call_storage;

pub struct Layout {
    /// Independently lowered immutable laws; never supplied by an assignment.
    pub terminal_transductions_rust: String,
    /// Exact independently lowered parent index for the single residence.
    pub record_service_resource: Option<u16>,
    pub continuation: ContinuationPins,
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
    /// Physical node/operation table geometry, not packed Host Call count.
    pub host_call_slots: usize,
    /// One pending request per independently checked callable node.
    pub pending_host_call_slots: usize,
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
    let digest = plot_seal
        .digest(&[c::PlotFragmentSeal {
            host_id: fragment.host_id.as_str(),
            fragment_id: fragment.fragment_id.as_str(),
        }])
        .map_err(|e| format!("canonical Plot Plan commitment seal: {e:?}"))?;
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
        if matches!(
            configuration.key.as_str(),
            "program" | "selector" | "reference"
        ) {
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
    let mut services = kernel
        .nodes
        .iter()
        .filter(|node| node.kind_id == conduit_semantic_catalog::RESOURCE_RECORD_SERVICE_KIND);
    let record_service_resource = if let Some(node) = services.next() {
        if services.next().is_some() {
            return Err("multiple native record-service owners".into());
        }
        let mut resources = kernel
            .resources
            .iter()
            .filter(|resource| resource.node == node.node);
        let resource = resources
            .next()
            .ok_or("record-service lacks its residence")?;
        if resources.next().is_some() || resource.units != 1 {
            return Err("record-service requires one exact residence".into());
        }
        Some(resource.resource)
    } else {
        None
    };
    let continuation = continuation::capture(fragment, kernel, generated.fore())?;
    Ok(Layout {
        terminal_transductions_rust: kernel.render_terminal_transductions(),
        record_service_resource,
        continuation,
        fragment_seal_recipe,
        fragment_seal_to_kernel,
        fragment_seal_placement_ids: fragment
            .placements
            .iter()
            .map(|p| p.placement_id.as_str().to_owned())
            .collect(),
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
        host_call_slots: host_call_storage::slot_count(
            kernel.nodes.len(), kernel.host_calls.iter().map(|call| call.call),
        ).map_err(str::to_owned)?,
        pending_host_call_slots: host_call_storage::pending_count(
            kernel.nodes.len(),
            kernel.host_calls.iter().map(|call| (call.node, call.maximum_in_flight)),
        ).map_err(str::to_owned)?,
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
#[path = "template_continuation_tests.rs"]
mod continuation_tests;

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
