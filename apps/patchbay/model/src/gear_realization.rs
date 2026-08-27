//! Bounded Gear-reverse truth derived from canonical Form, Plan, and Host offers.

use conduit_core::{
    verify_plan, ArtifactId, BootId, CapabilityId, HostAdvertisement, HostId, ImplementationId,
    Plan, PlannedGear,
};
use conduit_form::ExpandedCanonicalForm;
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical, PlacementChoice, PlannerError,
};

use crate::{PatchbayGraph, PatchbaySubjectRef};

pub const MAX_GEAR_REALIZATION_ALTERNATIVES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealizationDisposition {
    Selected,
    Compatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GearRealizationAlternative {
    pub disposition: RealizationDisposition,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub execution_profile_id: conduit_core::ExecutionProfileId,
    pub host_operation_contracts: Vec<String>,
    pub resource_classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GearRealizationInspection {
    pub gear_identity: String,
    pub gear_id: conduit_core::GearId,
    pub selected: Option<PlannedGear>,
    pub alternatives: Vec<GearRealizationAlternative>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GearRealizationError {
    StaleSubject,
    UnknownGear,
    InvalidPlan,
    StalePlanBasis,
    AmbiguousSelectedPlacement,
    SelectedOfferUnavailable,
    TooManyAlternatives,
    UnknownAlternative,
    SameRealization,
    Planning(PlannerError),
}

impl core::fmt::Display for GearRealizationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "Gear realization inspection failed: {self:?}")
    }
}

impl std::error::Error for GearRealizationError {}

impl GearRealizationInspection {
    pub fn inspect(
        graph: &PatchbayGraph,
        subject: &PatchbaySubjectRef,
        plan: Option<&Plan>,
        hosts: &[HostAdvertisement],
    ) -> Result<Self, GearRealizationError> {
        if subject.expanded_form_id != graph.expanded_form_id {
            return Err(GearRealizationError::StaleSubject);
        }
        let gear = graph
            .gears
            .iter()
            .find(|gear| gear.identity == subject.subject_identity)
            .ok_or(GearRealizationError::UnknownGear)?;
        let selected = plan
            .map(|plan| selected_placement(graph, &gear.gear_id, plan))
            .transpose()?
            .flatten();
        let inputs = gear
            .inputs
            .iter()
            .map(|port| port.descriptor.clone())
            .collect::<Vec<_>>();
        let outputs = gear
            .outputs
            .iter()
            .map(|port| port.descriptor.clone())
            .collect::<Vec<_>>();
        let mut alternatives = Vec::new();
        for host in hosts {
            for offer in &host.capabilities {
                if offer.kind_id != gear.kind_id
                    || offer.kind_contract_revision != gear.kind_contract_revision
                    || offer.inputs != inputs
                    || offer.outputs != outputs
                {
                    continue;
                }
                if alternatives.len() == MAX_GEAR_REALIZATION_ALTERNATIVES {
                    return Err(GearRealizationError::TooManyAlternatives);
                }
                let disposition = if selected.as_ref().is_some_and(|placement| {
                    placement.host_id == host.host_id
                        && placement.boot_id == host.boot_id
                        && placement.capability_id == offer.capability_id
                        && placement.implementation_id == offer.implementation.implementation_id
                }) {
                    RealizationDisposition::Selected
                } else {
                    RealizationDisposition::Compatible
                };
                alternatives.push(GearRealizationAlternative {
                    disposition,
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    capability_id: offer.capability_id.clone(),
                    implementation_id: offer.implementation.implementation_id.clone(),
                    artifact_id: offer.implementation.artifact_id.clone(),
                    execution_profile_id: offer.implementation.execution_profile_id.clone(),
                    host_operation_contracts: offer
                        .host_operations
                        .iter()
                        .map(|operation| operation.contract_id.as_str().to_owned())
                        .collect(),
                    resource_classes: offer
                        .resource_requirements
                        .iter()
                        .map(|resource| resource.class_id.as_str().to_owned())
                        .collect(),
                });
            }
        }
        alternatives.sort_by(|left, right| {
            (
                left.host_id.as_str(),
                left.boot_id.as_str(),
                left.capability_id.as_str(),
            )
                .cmp(&(
                    right.host_id.as_str(),
                    right.boot_id.as_str(),
                    right.capability_id.as_str(),
                ))
        });
        if selected.is_some()
            && !alternatives
                .iter()
                .any(|alternative| alternative.disposition == RealizationDisposition::Selected)
        {
            return Err(GearRealizationError::SelectedOfferUnavailable);
        }
        Ok(Self {
            gear_identity: gear.identity.clone(),
            gear_id: gear.gear_id.clone(),
            selected,
            alternatives,
        })
    }
}

pub fn replan_with_implementation(
    form: &ExpandedCanonicalForm,
    current_plan: &Plan,
    hosts: &[HostAdvertisement],
    subject: &PatchbaySubjectRef,
    host_id: &HostId,
    capability_id: &CapabilityId,
) -> Result<Plan, GearRealizationError> {
    let graph =
        PatchbayGraph::from_expanded(form).map_err(|_| GearRealizationError::UnknownGear)?;
    let inspection =
        GearRealizationInspection::inspect(&graph, subject, Some(current_plan), hosts)?;
    let requested = inspection
        .alternatives
        .iter()
        .find(|candidate| {
            &candidate.host_id == host_id && &candidate.capability_id == capability_id
        })
        .ok_or(GearRealizationError::UnknownAlternative)?;
    if requested.disposition == RealizationDisposition::Selected {
        return Err(GearRealizationError::SameRealization);
    }
    let mut placements =
        default_expanded_placements(form, hosts).map_err(GearRealizationError::Planning)?;
    placements.by_gear.insert(
        inspection.gear_id,
        PlacementChoice {
            host_id: host_id.clone(),
            capability_id: capability_id.clone(),
        },
    );
    let replacement = plan_expanded_canonical(
        form,
        hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
    )
    .map_err(GearRealizationError::Planning)?;
    if replacement.plan_id == current_plan.plan_id {
        return Err(GearRealizationError::SameRealization);
    }
    Ok(replacement)
}

fn selected_placement(
    graph: &PatchbayGraph,
    gear_id: &conduit_core::GearId,
    plan: &Plan,
) -> Result<Option<PlannedGear>, GearRealizationError> {
    if !verify_plan(plan) {
        return Err(GearRealizationError::InvalidPlan);
    }
    if plan.source_document_id != graph.source_document_id
        || plan.checked_form_id != graph.checked_form_id
        || plan.expanded_form_id != graph.expanded_form_id
    {
        return Err(GearRealizationError::StalePlanBasis);
    }
    let mut placements = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .filter(|placement| &placement.gear_id == gear_id);
    let selected = placements.next().cloned();
    if placements.next().is_some() {
        return Err(GearRealizationError::AmbiguousSelectedPlacement);
    }
    Ok(selected)
}
