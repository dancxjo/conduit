//! Admit a reviewed protocol artifact through the exact sealed Plan Fore.
//! This preparation entrance neither discovers hardware nor issues possession.
use crate::protocol_host_calls::ProtocolCallRefusal;
use alloc::{format, vec, vec::Vec};
use conduit_composite::{
    KernelCompositeBoundary, KernelCompositeDefinition, KernelCompositeFrontBinding,
};
use conduit_core::*;
use conduit_plot::CompositeFrontTerminal;

/// Packaging identities remain distinct from Host/Boot realization and Plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolArtifactIdentity {
    pub source: SourceDocumentId,
    pub checked: CheckedPlotId,
    pub expanded: ExpandedPlotId,
    pub artifact: ArtifactId,
}

pub struct AdmittedProtocolArtifact {
    identity: ProtocolArtifactIdentity,
    definition: KernelCompositeDefinition,
}

impl AdmittedProtocolArtifact {
    /// The packaging caller retains the reviewed artifact and its identities.
    /// This checks exact Plan correspondence, not review, trust, or authority.
    /// Actual hardware possession is required separately by native Play admission.
    pub fn admit(
        identity: ProtocolArtifactIdentity,
        plan: Plan,
    ) -> Result<Self, ProtocolCallRefusal> {
        if identity.artifact.as_str().is_empty()
            || identity.source != plan.source_document_id
            || identity.checked != plan.checked_plot_id
            || identity.expanded != plan.expanded_plot_id
            || !verify_plan(&plan)
            || plan.fragments.len() != 1
        {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        let fragment = &plan.fragments[0];
        let mut boundary = KernelCompositeBoundary {
            input_fronts: Vec::new(),
            output_fronts: Vec::new(),
        };
        let mut contracts = Vec::new();
        let mut maximum_bytes = 1;
        for fore in &fragment.fore_ports {
            if fore.track != ConnectionTrack::Payload {
                return Err(ProtocolCallRefusal::InvalidPlan);
            }
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == fore.placement_id)
                .ok_or(ProtocolCallRefusal::InvalidPlan)?;
            let ports = match fore.direction {
                PortDirection::Input => &placement.inputs,
                PortDirection::Output => &placement.outputs,
            };
            let port = ports
                .iter()
                .find(|port| port.port_id == fore.gear_port_id)
                .ok_or(ProtocolCallRefusal::InvalidPlan)?;
            if port.value_kind != fore.value_kind
                || port.temporal != fore.temporal
                || port.abnormal_kind != fore.abnormal_kind
            {
                return Err(ProtocolCallRefusal::InvalidPlan);
            }
            let mut external_port = port.clone();
            external_port.port_id = fore.front_port_id.clone();
            if let Some(contract) = &fore.value_contract {
                contract
                    .validate_definition()
                    .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
                maximum_bytes = maximum_bytes.max(contract.maximum_bytes);
                contracts.push(FrontValueContract {
                    location: match fore.direction {
                        PortDirection::Input => {
                            FrontValueLocation::Input(fore.front_port_id.clone())
                        }
                        PortDirection::Output => {
                            FrontValueLocation::Output(fore.front_port_id.clone())
                        }
                    },
                    contract: contract.clone(),
                });
            }
            if let Some(previous) = boundary
                .input_fronts
                .iter()
                .chain(&boundary.output_fronts)
                .find(|front| front.external_port.port_id == fore.front_port_id)
            {
                // One external input routes through the planner's already
                // lowered atomic fan-out. The binding retains one exact representative.
                if fore.direction != PortDirection::Input
                    || previous.external_port != external_port
                    || fragment
                        .fore_ports
                        .iter()
                        .filter(|candidate| candidate.front_port_id == fore.front_port_id)
                        .any(|candidate| {
                            candidate.direction != fore.direction
                                || candidate.value_contract != fore.value_contract
                        })
                {
                    return Err(ProtocolCallRefusal::InvalidPlan);
                }
                // Value contracts describe external ports, not fan-out targets.
                if fore.value_contract.is_some() {
                    contracts.pop();
                }
                continue;
            }
            let binding = KernelCompositeFrontBinding {
                external_port,
                internal_child: fragment.host_id.clone(),
                internal_placement_id: fore.placement_id.clone(),
                internal_port_id: fore.gear_port_id.clone(),
                terminal: CompositeFrontTerminal::Independent,
            };
            match fore.direction {
                PortDirection::Input => boundary.input_fronts.push(binding),
                PortDirection::Output => boundary.output_fronts.push(binding),
            }
        }
        let definition = KernelCompositeDefinition {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
            profile: HostProfileId::from("conduitos/protocol-play@1"),
            external_capability: conduit_core::capability_offer_from_parts! {
                semantic_contract: KindSemanticContract { configuration: vec![], laws: vec![KindSemanticLaw::ValueContracts(contracts)] },
                startup_parameters: vec![], shorthand: None,
                capability_id: CapabilityId::from(format!("conduitos/protocol-artifact/{}", identity.artifact.as_str())),
                kind_id: KindId::from(format!("plot/{}", identity.checked.as_str())),
                kind_contract_revision: KindIdentity::from(identity.checked.as_str()),
                implementation: ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from("conduitos/protocol-play@1"),
                    implementation_id: ImplementationId::from("conduitos/protocol-play@1"),
                    artifact_id: identity.artifact.clone(),
                },
                inputs: boundary.input_fronts.iter().map(|front| front.external_port.clone()).collect(),
                outputs: boundary.output_fronts.iter().map(|front| front.external_port.clone()).collect(),
                host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
                limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: maximum_bytes },
            },
            internal_plan: plan,
            boundary,
            failure_translation: FailureReason::CompositeCapabilityFailed,
        };
        crate::protocol_play::validate_fore(&definition)?;
        Ok(Self {
            identity,
            definition,
        })
    }
    pub fn identity(&self) -> &ProtocolArtifactIdentity {
        &self.identity
    }
    pub fn definition(&self) -> &KernelCompositeDefinition {
        &self.definition
    }
    pub fn into_definition(self) -> KernelCompositeDefinition {
        self.definition
    }
}
