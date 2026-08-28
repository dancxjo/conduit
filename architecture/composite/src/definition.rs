use conduit_core::{
    verify_plan, ArtifactId, BootId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FailureReason, HostId, HostProfileId, ImplementationId, OfferGeneration, Plan, PortDescriptor,
    PortDirection,
};
use conduit_form::{CheckedForm, CompositeFaceTerminal};

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
    pub input_faces: Vec<KernelCompositeFaceBinding>,
    pub output_faces: Vec<KernelCompositeFaceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositeFaceBinding {
    pub external_port: PortDescriptor,
    pub internal_child: HostId,
    pub internal_placement_id: conduit_core::PlacementId,
    pub internal_port_id: conduit_core::PortId,
    pub terminal: CompositeFaceTerminal,
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
        let bind_faces = |faces: &[conduit_form::CheckedCompositeFace]| {
            faces
                .iter()
                .map(|face| {
                    let placement = internal_plan
                        .fragments
                        .iter()
                        .flat_map(|fragment| &fragment.placements)
                        .find(|placement| placement.gear_id == face.internal_gear_id)
                        .ok_or_else(|| {
                            KernelCompositeDefinitionError::InvalidInternalPlan(format!(
                                "face '{}' internal operation is absent from the exact plan",
                                face.external_port.port_id.as_str()
                            ))
                        })?;
                    let planned_port = match face.external_port.direction {
                        PortDirection::Input => &placement.inputs,
                        PortDirection::Output => &placement.outputs,
                    }
                    .iter()
                    .find(|port| port.port_id == face.internal_port_id)
                    .ok_or_else(|| {
                        KernelCompositeDefinitionError::InvalidInternalPlan(format!(
                            "face '{}' internal endpoint is absent from the exact plan",
                            face.external_port.port_id.as_str()
                        ))
                    })?;
                    if planned_port.value_kind != face.external_port.value_kind
                        || planned_port.direction != face.external_port.direction
                        || planned_port.temporal != face.external_port.temporal
                    {
                        return Err(KernelCompositeDefinitionError::InvalidInternalPlan(
                            format!(
                                "face '{}' differs from its exact planned endpoint",
                                face.external_port.port_id.as_str()
                            ),
                        ));
                    }
                    Ok(KernelCompositeFaceBinding {
                        external_port: face.external_port.clone(),
                        internal_child: placement.host_id.clone(),
                        internal_placement_id: placement.placement_id.clone(),
                        internal_port_id: face.internal_port_id.clone(),
                        terminal: face.terminal,
                    })
                })
                .collect::<Result<Vec<_>, KernelCompositeDefinitionError>>()
        };
        let input_faces = bind_faces(&exported.input_faces)?;
        let output_faces = bind_faces(&exported.output_faces)?;
        if input_faces
            .iter()
            .chain(&output_faces)
            .any(|face| face.terminal != CompositeFaceTerminal::Independent)
        {
            return Err(KernelCompositeDefinitionError::InvalidInternalPlan(
                "the kernel composite profile currently requires independent faces".into(),
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
            external_capability: CapabilityOffer {
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
                host_operations: vec![],
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
                input_faces,
                output_faces,
            },
            failure_translation,
        })
    }
}
