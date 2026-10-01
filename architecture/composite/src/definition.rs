use conduit_core::{
    verify_plan, verify_prepared_plan, ArtifactId, BootId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FailureReason, HostId, HostProfileId, ImplementationId, OfferGeneration,
    Plan, PlannedActivationEntry, PlannedActivationFront, PortDescriptor, PortDirection,
    PreparedPlan,
};
use conduit_form::{CheckedForm, CompositeFrontTerminal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeDefinitionError {
    InvalidInternalPlan(String),
}

impl core::fmt::Display for KernelCompositeDefinitionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidInternalPlan(reason) => write!(f, "invalid internal plan: {reason}"),
        }
    }
}

impl std::error::Error for KernelCompositeDefinitionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositeBoundary {
    pub input_fronts: Vec<KernelCompositeFrontBinding>,
    pub output_fronts: Vec<KernelCompositeFrontBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositeFrontBinding {
    pub external_port: PortDescriptor,
    pub internal_child: HostId,
    pub internal_placement_id: conduit_core::PlacementId,
    pub internal_port_id: conduit_core::PortId,
    pub terminal: CompositeFrontTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositeDefinition {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub profile: HostProfileId,
    pub external_capability: CapabilityOffer,
    pub internal_plan: Plan,
    pub boundary: KernelCompositeBoundary,
    pub failure_translation: FailureReason,
}

impl KernelCompositeDefinition {
    /// Build the executable child composite from sealed Plan and preparation
    /// truth. This entrance deliberately cannot see CheckedForm or a Kind
    /// registry: every boundary route is an exact selected-Plan Fore.
    pub fn from_planned_activation(
        outer: &Plan,
        prepared: &PreparedPlan,
        activation_id: &str,
    ) -> Result<Self, KernelCompositeDefinitionError> {
        if !verify_plan(outer) || !verify_prepared_plan(prepared, outer) {
            return Err(invalid("outer Plan or its exact preparation is stale"));
        }
        let entry = outer
            .activations
            .iter()
            .find(|entry| activation_identity(entry) == activation_id)
            .ok_or_else(|| invalid("planned activation is absent"))?;
        let binding = outer
            .activation_preparations
            .iter()
            .find(|binding| binding.activation_id == activation_id)
            .ok_or_else(|| invalid("planned activation preparation binding is absent"))?;
        let (owner, selected, inputs, output, limits) = match entry {
            PlannedActivationEntry::Unary(value) => (
                &value.owner_placement_id,
                value.selected_plan.as_ref(),
                vec![&value.input],
                &value.output,
                value.limits,
            ),
            PlannedActivationEntry::Fold(value) => (
                &value.owner_placement_id,
                value.selected_plan.as_ref(),
                vec![&value.accumulator_input, &value.item_input],
                &value.output,
                value.limits,
            ),
            PlannedActivationEntry::Scan(value) => (
                &value.owner_placement_id,
                value.selected_plan.as_ref(),
                vec![&value.accumulator_input, &value.item_input],
                &value.output,
                value.limits,
            ),
        };
        if binding.owner_placement_id != *owner || binding.selected_plan_id != selected.plan_id {
            return Err(invalid(
                "activation binding differs from selected Plan truth",
            ));
        }
        let owner = outer
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.placement_id == *owner)
            .ok_or_else(|| invalid("activation owner placement is absent"))?;
        let bind = |front: &PlannedActivationFront, direction| {
            bind_planned_front(selected, front, direction)
        };
        let input_fronts = inputs
            .into_iter()
            .map(|front| bind(front, PortDirection::Input))
            .collect::<Result<Vec<_>, _>>()?;
        let output_fronts = vec![bind(output, PortDirection::Output)?];
        let inputs = input_fronts
            .iter()
            .map(|front| front.external_port.clone())
            .collect();
        let outputs = output_fronts
            .iter()
            .map(|front| front.external_port.clone())
            .collect();
        Ok(Self {
            host_id: binding.owner_host_id.clone(),
            boot_id: binding.owner_boot_id.clone(),
            offer_generation: binding.owner_offer_generation,
            profile: HostProfileId::from("conduit/planned-activation-composite@1"),
            external_capability: conduit_core::capability_offer_from_parts! {
                semantic_contract: owner.semantic_contract.clone(),
                startup_parameters: vec![],
                shorthand: None,
                capability_id: owner.capability_id.clone(),
                kind_id: owner.kind_id.clone(),
                kind_contract_revision: owner.kind_contract_revision.clone(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: owner.execution_profile_id.clone(),
                    implementation_id: owner.implementation_id.clone(),
                    artifact_id: owner.artifact_id.clone(),
                },
                inputs,
                outputs,
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: limits.maximum_active,
                    max_queue_items: limits.maximum_queue_items,
                    max_queue_bytes: limits.maximum_queue_bytes,
                },
            },
            internal_plan: selected.clone(),
            boundary: KernelCompositeBoundary {
                input_fronts,
                output_fronts,
            },
            failure_translation: FailureReason::CompositeCapabilityFailed,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_authored_export(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile: HostProfileId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
        form: &CheckedForm,
        export_capability_id: &conduit_core::CapabilityId,
        internal_plan: Plan,
        failure_translation: FailureReason,
    ) -> Result<Self, KernelCompositeDefinitionError> {
        if internal_plan.source_document_id != form.source_document_id
            || internal_plan.checked_form_id != form.checked_form_id
            || internal_plan.expanded_form_id != form.expanded_form_id
            || !verify_plan(&internal_plan)
        {
            return Err(KernelCompositeDefinitionError::InvalidInternalPlan(
                "authored form and exact internal plan do not agree".into(),
            ));
        }
        let exported = form
            .export_boundary(export_capability_id)
            .map_err(|error| {
                KernelCompositeDefinitionError::InvalidInternalPlan(error.to_string())
            })?;
        let bind_fronts = |fronts: &[conduit_form::CheckedCompositeFront]| {
            fronts
                .iter()
                .map(|front| {
                    let placement = internal_plan
                        .fragments
                        .iter()
                        .flat_map(|fragment| &fragment.placements)
                        .find(|placement| placement.gear_id == front.internal_gear_id)
                        .ok_or_else(|| {
                            KernelCompositeDefinitionError::InvalidInternalPlan(format!(
                                "front '{}' internal operation is absent from the exact plan",
                                front.external_port.port_id.as_str()
                            ))
                        })?;
                    let planned_port = match front.external_port.direction {
                        PortDirection::Input => &placement.inputs,
                        PortDirection::Output => &placement.outputs,
                    }
                    .iter()
                    .find(|port| port.port_id == front.internal_port_id)
                    .ok_or_else(|| {
                        KernelCompositeDefinitionError::InvalidInternalPlan(format!(
                            "front '{}' internal endpoint is absent from the exact plan",
                            front.external_port.port_id.as_str()
                        ))
                    })?;
                    if planned_port.value_kind != front.external_port.value_kind
                        || planned_port.direction != front.external_port.direction
                        || planned_port.temporal != front.external_port.temporal
                        || planned_port.abnormal_kind != front.external_port.abnormal_kind
                    {
                        return Err(KernelCompositeDefinitionError::InvalidInternalPlan(
                            format!(
                                "front '{}' differs from its exact planned endpoint",
                                front.external_port.port_id.as_str()
                            ),
                        ));
                    }
                    Ok(KernelCompositeFrontBinding {
                        external_port: front.external_port.clone(),
                        internal_child: placement.host_id.clone(),
                        internal_placement_id: placement.placement_id.clone(),
                        internal_port_id: front.internal_port_id.clone(),
                        terminal: front.terminal,
                    })
                })
                .collect::<Result<Vec<_>, KernelCompositeDefinitionError>>()
        };
        let input_fronts = bind_fronts(&exported.input_fronts)?;
        let output_fronts = bind_fronts(&exported.output_fronts)?;
        if input_fronts
            .iter()
            .chain(&output_fronts)
            .any(|front| front.terminal != CompositeFrontTerminal::Independent)
        {
            return Err(KernelCompositeDefinitionError::InvalidInternalPlan(
                "the kernel composite profile currently requires independent fronts".into(),
            ));
        }
        let queue_items = internal_plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .map(|connection| connection.item_capacity)
            .min()
            .unwrap_or(conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY);
        let queue_bytes = internal_plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .map(|connection| connection.byte_capacity)
            .min()
            .unwrap_or(conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY);
        Ok(Self {
            host_id,
            boot_id,
            offer_generation,
            profile,
            external_capability: conduit_core::capability_offer_from_parts! {
                semantic_contract: Default::default(),
                startup_parameters: vec![],
                shorthand: None,
                capability_id: exported.capability_id,
                kind_id: exported.kind_id,
                kind_contract_revision: exported.kind_contract_revision,
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from(format!(
                        "composite:{}@1",
                        implementation_id.as_str()
                    )),
                    implementation_id,
                    artifact_id,
                },
                inputs: exported.inputs,
                outputs: exported.outputs,
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: queue_items,
                    max_queue_bytes: queue_bytes,
                },
            },
            internal_plan,
            boundary: KernelCompositeBoundary {
                input_fronts,
                output_fronts,
            },
            failure_translation,
        })
    }
}

fn invalid(reason: impl Into<String>) -> KernelCompositeDefinitionError {
    KernelCompositeDefinitionError::InvalidInternalPlan(reason.into())
}

fn activation_identity(entry: &PlannedActivationEntry) -> &str {
    match entry {
        PlannedActivationEntry::Unary(value) => &value.activation_id,
        PlannedActivationEntry::Fold(value) => &value.activation_id,
        PlannedActivationEntry::Scan(value) => &value.activation_id,
    }
}

fn bind_planned_front(
    plan: &Plan,
    expected: &PlannedActivationFront,
    direction: PortDirection,
) -> Result<KernelCompositeFrontBinding, KernelCompositeDefinitionError> {
    let mut matches = plan.fragments.iter().flat_map(|fragment| {
        fragment
            .fore_ports
            .iter()
            .filter(move |front| {
                front.front_port_id == expected.front_port_id && front.direction == direction
            })
            .map(move |front| (fragment, front))
    });
    let (fragment, front) = matches.next().ok_or_else(|| {
        invalid(format!(
            "selected Fore '{}' is absent",
            expected.front_port_id.as_str()
        ))
    })?;
    if matches.next().is_some()
        || front.value_kind != expected.value_kind
        || front.abnormal_kind != expected.abnormal_kind
    {
        return Err(invalid(format!(
            "selected Fore '{}' is ambiguous or differs from activation truth",
            expected.front_port_id.as_str()
        )));
    }
    let descriptor = fragment
        .placements
        .iter()
        .find(|placement| placement.placement_id == front.placement_id)
        .and_then(|placement| {
            match direction {
                PortDirection::Input => placement.inputs.iter(),
                PortDirection::Output => placement.outputs.iter(),
            }
            .find(|port| port.port_id == front.gear_port_id)
        })
        .ok_or_else(|| invalid("selected Fore endpoint is absent"))?;
    let mut external = descriptor.clone();
    external.port_id = front.front_port_id.clone();
    Ok(KernelCompositeFrontBinding {
        external_port: external,
        internal_child: fragment.host_id.clone(),
        internal_placement_id: front.placement_id.clone(),
        internal_port_id: front.gear_port_id.clone(),
        terminal: CompositeFrontTerminal::Independent,
    })
}
