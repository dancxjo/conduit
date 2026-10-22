//! Exact canonical checked export to its Plan-sealed independent payload Fore.
//! No legacy CheckedPlot projection, identity rewriting or default queue exists here.
use crate::{
    KernelCompositeBoundary, KernelCompositeDefinition, KernelCompositeDefinitionError,
    KernelCompositeFrontBinding,
};
use alloc::{format, vec, vec::Vec};
use conduit_core::*;
use conduit_plot::{
    expand_canonical_plot_for_authoring, CheckedSyntaxDocument, CompositeFrontTerminal,
    ProfileCatalog,
};
fn invalid(reason: &str) -> KernelCompositeDefinitionError {
    KernelCompositeDefinitionError::InvalidInternalPlan(reason.into())
}
impl KernelCompositeDefinition {
    /// Re-expand the actual checked Source and bind every declared payload Fore.
    /// Initial profile: one fragment, no Fore fanout, independent payload tracks only.
    #[allow(clippy::too_many_arguments)]
    pub fn from_canonical_export(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile: HostProfileId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
        document: &CheckedSyntaxDocument,
        entry: &str,
        catalog: &ProfileCatalog,
        internal_plan: Plan,
        failure_translation: FailureReason,
    ) -> Result<Self, KernelCompositeDefinitionError> {
        let authored = expand_canonical_plot_for_authoring(document, entry, catalog)
            .map_err(|_| invalid("canonical entry expansion refused"))?;
        if !verify_plan(&internal_plan)
            || internal_plan.source_document_id != authored.expanded.source_document_id
            || internal_plan.checked_plot_id != authored.expanded.checked_plot_id
            || internal_plan.expanded_plot_id != authored.expanded.expanded_plot_id
            || internal_plan.fragments.len() != 1
        {
            return Err(invalid(
                "canonical Source and sealed Plan identity mismatch",
            ));
        }
        if !authored.front.startup_parameters().is_empty()
            || !authored.front.resource_ports().is_empty()
            || authored.abnormal_export.is_some()
        {
            return Err(invalid(
                "canonical export startup/resource/abnormal Fore unsupported",
            ));
        }
        verify_authored_graph(&authored.expanded, &internal_plan)?;
        let fragment = &internal_plan.fragments[0];
        let expected = authored.input_bindings.len() + authored.output_bindings.len();
        if expected == 0
            || fragment.fore_ports.len() != expected
            || authored.front.inputs().len() + authored.front.outputs().len() != expected
        {
            return Err(invalid("canonical export missing/extra/fanout Fore"));
        }
        let mut boundary = KernelCompositeBoundary {
            input_fronts: Vec::new(),
            output_fronts: Vec::new(),
        };
        let mut seen = Vec::new();
        let mut maximum_items = 0;
        let mut maximum_bytes = 0;
        for (direction, bindings, ports) in [
            (
                PortDirection::Input,
                authored.input_bindings.as_slice(),
                authored.front.inputs(),
            ),
            (
                PortDirection::Output,
                authored.output_bindings.as_slice(),
                authored.front.outputs(),
            ),
        ] {
            for binding in bindings {
                if binding.track != ConnectionTrack::Payload
                    || seen.contains(&binding.front_port_id)
                {
                    return Err(invalid(
                        "canonical export duplicate/unsupported track or fanout",
                    ));
                }
                seen.push(binding.front_port_id.clone());
                let descriptor = ports
                    .iter()
                    .find(|port| port.port_id == binding.front_port_id)
                    .ok_or_else(|| invalid("canonical authored descriptor missing"))?;
                let mut fores = fragment.fore_ports.iter().filter(|fore| {
                    fore.front_port_id == binding.front_port_id && fore.direction == direction
                });
                let fore = fores
                    .next()
                    .ok_or_else(|| invalid("canonical sealed Fore missing"))?;
                if fores.next().is_some() || fore.track != ConnectionTrack::Payload {
                    return Err(invalid("canonical sealed Fore duplicate/unsupported track"));
                }
                if fore.pressure_policy != DeliveryPressurePolicy::PreserveOrder
                    || fore.selected_line.is_some()
                {
                    return Err(invalid(
                        "canonical export pressure/selected Line unsupported",
                    ));
                }
                let mut selected = fragment
                    .placements
                    .iter()
                    .filter(|gear| gear.gear_id == binding.gear_id);
                let gear = selected
                    .next()
                    .ok_or_else(|| invalid("canonical selected gear missing"))?;
                if selected.next().is_some()
                    || fore.placement_id != gear.placement_id
                    || fore.gear_port_id != binding.gear_port_id
                {
                    return Err(invalid("canonical selected gear/binding mismatch"));
                }
                let actual = match direction {
                    PortDirection::Input => &gear.inputs,
                    PortDirection::Output => &gear.outputs,
                }
                .iter()
                .find(|port| port.port_id == binding.gear_port_id)
                .ok_or_else(|| invalid("canonical selected port missing"))?;
                if descriptor.abnormal_kind.is_some() {
                    return Err(invalid(
                        "canonical export abnormal payload Fore unsupported",
                    ));
                }
                if actual.direction != direction
                    || descriptor.direction != direction
                    || actual.value_kind != descriptor.value_kind
                    || actual.temporal != descriptor.temporal
                    || actual.abnormal_kind != descriptor.abnormal_kind
                    || fore.value_kind != descriptor.value_kind
                    || fore.temporal != descriptor.temporal
                    || fore.abnormal_kind != descriptor.abnormal_kind
                {
                    return Err(invalid("canonical Fore descriptor mismatch"));
                }
                let external_location = match direction {
                    PortDirection::Input => {
                        FrontValueLocation::Input(binding.front_port_id.clone())
                    }
                    PortDirection::Output => {
                        FrontValueLocation::Output(binding.front_port_id.clone())
                    }
                };
                let internal_location = match direction {
                    PortDirection::Input => FrontValueLocation::Input(binding.gear_port_id.clone()),
                    PortDirection::Output => {
                        FrontValueLocation::Output(binding.gear_port_id.clone())
                    }
                };
                let contract = authored.front.value_contract(&external_location);
                let selected_front = gear.checked_port_front();
                if contract != fore.value_contract.as_ref()
                    || contract != selected_front.value_contract(&internal_location)
                {
                    return Err(invalid("canonical Fore value contract mismatch"));
                }
                if fore.item_capacity == 0
                    || fore.byte_capacity == 0
                    || fore.item_capacity > gear.limits.max_queue_items
                    || fore.byte_capacity > gear.limits.max_queue_bytes
                {
                    return Err(invalid("canonical Fore queue outside selected bounds"));
                }
                if let Some(contract) = contract {
                    contract
                        .validate_definition()
                        .map_err(|_| invalid("canonical Fore contract invalid"))?;
                    if contract.maximum_bytes > fore.byte_capacity {
                        return Err(invalid("canonical Fore cannot retain typed payload bound"));
                    }
                }
                maximum_items = maximum_items.max(fore.item_capacity);
                maximum_bytes = maximum_bytes.max(fore.byte_capacity);
                let front = KernelCompositeFrontBinding {
                    external_port: descriptor.clone(),
                    internal_child: fragment.host_id.clone(),
                    internal_placement_id: fore.placement_id.clone(),
                    internal_port_id: fore.gear_port_id.clone(),
                    terminal: CompositeFrontTerminal::Independent,
                };
                match direction {
                    PortDirection::Input => boundary.input_fronts.push(front),
                    PortDirection::Output => boundary.output_fronts.push(front),
                }
            }
        }
        Ok(Self {
            host_id,
            boot_id,
            offer_generation,
            profile,
            external_capability: conduit_core::capability_offer_from_parts! {
                semantic_contract: KindSemanticContract { configuration: vec![], laws: vec![KindSemanticLaw::ValueContracts(authored.front.value_contracts().to_vec())] },
                startup_parameters: vec![], shorthand: None,
                capability_id: CapabilityId::from(entry.rsplit('/').next().unwrap_or(entry)),
                kind_id: KindId::from(entry), kind_contract_revision: KindIdentity::from(authored.expanded.checked_plot_id.as_str()),
                implementation: ImplementationOffer { execution_profile_id: ExecutionProfileId::from(format!("composite:{}@1", implementation_id.as_str())), implementation_id, artifact_id },
                inputs: authored.front.inputs().to_vec(), outputs: authored.front.outputs().to_vec(),
                host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
                limits: CapabilityLimits { max_active_instances: 1, max_queue_items: maximum_items, max_queue_bytes: maximum_bytes },
            },
            internal_plan,
            boundary,
            failure_translation,
        })
    }
}

/// Plan integrity is distinct from correspondence to the supplied checked Source.
fn verify_authored_graph(
    authored: &conduit_plot::ExpandedCanonicalPlot,
    plan: &Plan,
) -> Result<(), KernelCompositeDefinitionError> {
    let fragment = &plan.fragments[0];
    let completion = match authored.completion {
        conduit_plot::PlotCompletionPolicy::Live => PlanCompletionPolicy::Live,
        conduit_plot::PlotCompletionPolicy::SemanticCompletion => {
            PlanCompletionPolicy::SemanticCompletion
        }
    };
    if plan.completion_policy != completion
        || authored.realization_backs != plan.realization_backs
        || !authored.activations.is_empty()
        || !plan.activations.is_empty()
        || !plan.activation_preparations.is_empty()
        || !authored.shared_pools.is_empty()
        || !fragment.shared_pools.is_empty()
        || !fragment.states.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.execution_regions.is_empty()
        || authored.gears.len() != fragment.placements.len()
        || authored.connections.len() != fragment.connections.len()
    {
        return Err(invalid("canonical internal graph/profile mismatch"));
    }
    let mut seen_gears = Vec::new();
    let mut seen_placements = Vec::new();
    for expected in &authored.gears {
        if seen_gears.contains(&expected.gear_id) || !expected.resource_ports.is_empty() {
            return Err(invalid(
                "canonical duplicate gear/resource Cord unsupported",
            ));
        }
        seen_gears.push(expected.gear_id.clone());
        let mut matches = fragment
            .placements
            .iter()
            .filter(|gear| gear.gear_id == expected.gear_id);
        let actual = matches
            .next()
            .ok_or_else(|| invalid("canonical internal gear missing"))?;
        if matches.next().is_some()
            || seen_placements.contains(&actual.placement_id)
            || actual.kind_id != expected.kind_id
            || actual.kind_contract_revision != expected.kind_contract_revision
            || actual.configuration != expected.configuration
            || actual.inputs != expected.inputs
            || actual.outputs != expected.outputs
            || actual.semantic_contract != expected.semantic_contract
            || actual.terminal_transductions != expected.terminal_transductions
            || actual.pool_references != expected.pool_references
            || actual.host_id != fragment.host_id
            || actual.boot_id != fragment.boot_id
            || actual.offer_generation != fragment.offer_generation
        {
            return Err(invalid("canonical internal gear meaning mismatch"));
        }
        seen_placements.push(actual.placement_id.clone());
    }
    let mut seen_cords = Vec::new();
    for expected in &authored.connections {
        let source = fragment
            .placements
            .iter()
            .find(|gear| gear.gear_id == expected.source_gear_id)
            .ok_or_else(|| invalid("canonical internal source missing"))?;
        let sink = fragment
            .placements
            .iter()
            .find(|gear| gear.gear_id == expected.sink_gear_id)
            .ok_or_else(|| invalid("canonical internal sink missing"))?;
        let mut matches = fragment.connections.iter().filter(|cord| {
            cord.source_placement_id == source.placement_id
                && cord.source_port_id == expected.source_port_id
                && cord.sink_placement_id == sink.placement_id
                && cord.sink_port_id == expected.sink_port_id
                && cord.track == expected.track
        });
        let actual = matches
            .next()
            .ok_or_else(|| invalid("canonical internal Cord missing"))?;
        if matches.next().is_some()
            || seen_cords.contains(&actual.connection_id)
            || actual.value_kind != expected.value_kind
            || actual.temporal != expected.temporal
            || actual.track != ConnectionTrack::Payload
            || actual.resource.is_some()
            || actual.abnormal_kind.is_some()
            || actual.item_capacity == 0
            || actual.byte_capacity == 0
            || actual.item_capacity > source.limits.max_queue_items
            || actual.item_capacity > sink.limits.max_queue_items
            || actual.byte_capacity > source.limits.max_queue_bytes
            || actual.byte_capacity > sink.limits.max_queue_bytes
            || actual.pressure_policy != DeliveryPressurePolicy::PreserveOrder
            || actual.selected_line.is_some()
            || !actual.admitted_lines.is_empty()
        {
            return Err(invalid("canonical internal Cord meaning/profile mismatch"));
        }
        // Typed endpoints and exact authored field comparisons above are the
        // semantic graph. Queue allowance remains the already admitted Plan fact.
        if !source.outputs.iter().any(|port| {
            port.port_id == actual.source_port_id
                && port.value_kind == actual.value_kind
                && port.temporal == actual.temporal
        }) || !sink.inputs.iter().any(|port| {
            port.port_id == actual.sink_port_id
                && port.value_kind == actual.value_kind
                && port.temporal == actual.temporal
        }) {
            return Err(invalid("canonical internal Cord endpoint mismatch"));
        }
        // Whole-payload queues must retain the largest admitted source value;
        // a fresh seal and selected ceiling do not establish that lower bound.
        let front = source.checked_port_front();
        if let Some(contract) =
            front.value_contract(&FrontValueLocation::Output(actual.source_port_id.clone()))
        {
            contract
                .validate_definition()
                .map_err(|_| invalid("canonical internal Cord source contract invalid"))?;
            if contract.maximum_bytes > actual.byte_capacity {
                return Err(invalid(
                    "canonical internal Cord cannot retain typed payload bound",
                ));
            }
        }
        seen_cords.push(actual.connection_id.clone());
    }
    Ok(())
}
