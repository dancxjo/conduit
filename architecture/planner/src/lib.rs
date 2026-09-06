#![no_std]
#![doc = r#"
# Planner architecture

The ordinary v1 path is intentionally small:

1. `default_placements` derives functionally valid placement choices from a
   checked Form and current Host advertisements.
2. `plan` (or `plan_with_options`) validates exact capabilities, resources,
   authority, Lines, queue bounds, and startup order, then seals one immutable
   `Plan`.

Reusable mechanisms surround that path without replacing it:

- `requirements`, `characteristics`, and `policy` keep hard admissibility
  distinct from reviewed preference;
- `observations`, `locality`, `performance_policy`, and `fusion` select from
  current truthful offers using bounded evidence;
- `incremental`, `replanning`, `degradation`, `diversity`,
  `dormant_readmission`, `survival_policy`, and `recursive_recovery` produce or
  justify fresh planning decisions without mutating an existing Plan;
- `realization` and `realization_families` select exact implementation leaves.

Named acceptance compositions live under [`proof`]. They consume the reusable
planner API but are not planner architecture or production extension points.
New policy belongs in a focused reusable module; new end-to-end evidence belongs
under `proof`.

Named acceptance compositions are deliberately unavailable as flat imports.
"#]

#[macro_use]
extern crate alloc;
#[cfg(test)]
extern crate std;

mod prelude {
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
}

use crate::prelude::*;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    mandatory_sign_storage_requirement, seal_plan, AdmittedLine, AuthorityBinding, AuthorityGrant,
    BaseImplementationId, CancellationPolicy, CapabilityId, ConnectionId, ExpectedSign,
    ExpectedTerminal, FragmentId, GearId, HostAdvertisement, HostId, LineAvailability, LineId,
    LineOffer, PlacementId, Plan, PlanFragment, PlanId, PlannedConnection, PlannedGear,
    ResourcePoolId, StartupDependency, TerminalPolicy, DEFAULT_CONNECTION_BYTE_CAPACITY,
    DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_form::{CheckedForm, CheckedGear};
use sha2::{Digest, Sha256};

mod accelerator;
mod advice;
mod body_envelope;
mod canonical;
mod characteristic_policy;
mod characteristic_sealing;
mod characteristics;
mod compute_admission;
mod contract;
mod decision_evidence;
mod degradation;
mod degraded_profile;
mod diagnostic;
mod diversity;
mod dormant_readmission;
mod fact_policy;
mod functional_compatibility;
mod fusion;
mod generic_selection;
mod incremental;
mod locality;
mod observations;
mod performance_policy;
mod policy;
mod policy_composition;
mod profile;
pub mod proof;
mod protected_resources;
mod realization;
mod realization_families;
mod recursive_recovery;
mod replanning;
mod requirements;
mod resource_binding;
mod startup;
pub mod state_delay;
#[cfg(test)]
use startup::startup_order;
mod style;
mod survival_policy;

use functional_compatibility::default_placements_unvalidated;
use protected_resources::validate_protected_resource_grants;

pub use accelerator::{
    select_accelerator_candidate, AcceleratorCandidate, AcceleratorCandidateDisposition,
    AcceleratorCandidateEvidence, AcceleratorDemand, AcceleratorDimension, AcceleratorObservation,
    AcceleratorOffer, AcceleratorPlanningBasis, AcceleratorReservation, AcceleratorSelection,
    ExecutionMechanism, MAXIMUM_ACCELERATOR_CANDIDATES, MAXIMUM_ACCELERATOR_DEMANDS,
    MAXIMUM_ACCELERATOR_DIMENSIONS, MAXIMUM_ACCELERATOR_OFFERS,
};
pub use advice::{
    seed_planning_from_advice, AdvisedPlanningInputs, PlanningAdvice, PlanningAdviceEvidence,
    PlanningAdviceRefusal, SuggestedLine, SuggestedPlacement, MAXIMUM_ADVICE_ID_BYTES,
    MAXIMUM_ADVICE_LINES, MAXIMUM_ADVICE_PLACEMENTS,
};
pub use body_envelope::plan_with_resource_allowances;
pub use canonical::{
    default_expanded_placements, plan_canonical_realization_with_options, plan_expanded_canonical,
    plan_expanded_canonical_with_connection_limits, plan_expanded_canonical_with_options,
    plan_expanded_canonical_with_shared_pools, CanonicalRealizationMode,
    CanonicalRealizationSelectionError, PlannedCanonicalRealization, SharedPoolPlanningRequirement,
};
pub use characteristics::{
    plan_selected_realizations_with_characteristics,
    plan_selected_realizations_with_characteristics_and_authority,
    plan_selected_realizations_with_characteristics_and_options,
    select_realization_with_characteristics, select_realization_with_characteristics_and_signs,
    SelectedRealizationPlanning, MAXIMUM_PLANNER_POLICY_CLAUSES,
};
pub use contract::{
    parse_placements, ConnectionEndpoints, ConnectionQueueLimits, PlacementChoice,
    PlacementChoices, PlannerError, PlanningOptions,
};
pub use decision_evidence::{
    RealizationDecisionDisposition, RealizationDecisionRecord, RealizationRejection,
    RealizationSelection, MAXIMUM_REALIZATION_DECISION_RECORDS,
};
pub use degradation::{
    assess_scoped_degradation, DegradationAssessment, DegradationFragment,
    DegradationFragmentDisposition, DegradationInput, MAXIMUM_DEGRADATION_FRAGMENTS,
    MAXIMUM_DEGRADATION_FRAGMENT_ID_BYTES, MAXIMUM_DEGRADATION_REFUSAL_BYTES,
};
pub use degraded_profile::{
    seal_reviewed_service_profile_plan, select_reviewed_service_profile, DegradationDirection,
    DegradedDimension, DegradedDimensionEvidence, DegradedProfileRefusal, ReviewedServiceProfile,
    ServiceProfileAdmission, ServiceProfileDisposition, SurvivalPolicy,
    MAXIMUM_DEGRADED_PROFILE_DIMENSIONS, MAXIMUM_DEGRADED_PROFILE_ID_BYTES,
    MAXIMUM_DEGRADED_PROFILE_LABEL_BYTES,
};
pub use diagnostic::structured_planner_diagnostic;
pub use diversity::{
    classify_diversity, prove_diverse_replacement, select_surviving_diverse_candidate,
    DiversityCandidate, DiversityRefusal, DiversityRelationship, DiversityReplacementEvidence,
    LinePathHop, MechanismDependency, PreviousPlanDisposition, MAXIMUM_DIVERSITY_CANDIDATES,
    MAXIMUM_DIVERSITY_DEPENDENCIES, MAXIMUM_DIVERSITY_ID_BYTES, MAXIMUM_DIVERSITY_LINE_HOPS,
    MAXIMUM_DIVERSITY_MECHANISMS,
};
pub use dormant_readmission::{
    observe_dormant_candidate, prove_dormant_readmission, CurrentDormantCandidate,
    DormantEquipmentHistory, DormantReadmissionEvidence, DormantReadmissionRefusal,
    RequiredDormantLine, MAXIMUM_DORMANT_ABSENT_GENERATIONS, MAXIMUM_DORMANT_ID_BYTES,
    MAXIMUM_DORMANT_REQUIRED_LINES, MAXIMUM_DORMANT_SIGNS,
};
pub use fact_policy::{PlannerFactRef, PlannerFactValue, PlannerPredicate, PlannerPreference};
pub use fusion::{
    plan_selected_optimization, select_fusion_candidate, FusionBoundary, FusionCandidate,
    FusionCandidateEvidence, FusionDecisionGroup, FusionPlanningInputs, FusionPlanningObservation,
    FusionRealizationOffer, FusionSelection, OptimizedPlan, MAXIMUM_FUSION_CANDIDATES,
    MAXIMUM_FUSION_GROUPS, MAXIMUM_FUSION_MEMBERS, MAXIMUM_FUSION_OFFERS,
};
pub use incremental::{
    plan_cold, CandidateEvaluation, CandidateEvaluationDisposition, CandidateStructure, FactDomain,
    IncrementalCandidateEvidence, IncrementalPlan, IncrementalPlanner, IncrementalPlannerMetrics,
    PlanningFact, PlanningFactKey, StabilityPolicy, MAXIMUM_CACHED_CANDIDATES,
    MAXIMUM_CANDIDATE_DEPENDENCIES, MAXIMUM_INCREMENTAL_CANDIDATES, MAXIMUM_PLANNING_FACTS,
};
pub use locality::{
    select_data_locality_candidate, CandidateCostEvidence, CandidatePlacement,
    CandidatePlacementDisposition, DataFlowObservation, LocalCordObservation, LocalityCandidate,
    LocalityPlanningBasis, LocalitySelection, ObservationProvenance, RealizationWorkObservation,
    ReductionObservation, TransportObservation, MAXIMUM_LOCALITY_CANDIDATES,
    MAXIMUM_LOCALITY_LINE_OFFERS, MAXIMUM_LOCALITY_OBSERVATIONS,
};
pub use observations::select_realization_with_observations;
pub use performance_policy::{
    select_performance_candidate, PerformanceCandidate, PerformanceCandidateDisposition,
    PerformanceCandidateEvidence, PerformanceIntent, PerformancePolicy, PerformancePolicySelection,
    PerformanceProfileObservation, MAXIMUM_PERFORMANCE_CANDIDATES,
};
pub use policy::{select_realization_with_policy, RealizationPolicy, RealizationPreference};
pub use policy_composition::{
    select_realization_with_scoped_policy, ObservationBasis, PlanningPolicyBasis, PolicyLayer,
    PolicyScope, PolicySourceId, PolicySourceRevision, ReviewedObservation,
    ScopedRealizationSelection, MAXIMUM_POLICY_SOURCES, MAXIMUM_RETAINED_POLICY_OBSERVATIONS,
};
pub use profile::{
    plan_with_advertised_profile, BROWSER_PLANNER_PROFILE, FULL_PLANNER_LIMITS,
    FULL_PLANNER_PROFILE,
};
pub use realization::plan_selected_realizations;
pub use realization_families::{
    select_current_family_frontier, CurrentFamilyOffer, FamilyFrontier, FamilyFrontierMetrics,
    RealizationFamily, RealizationFamilyCatalog, MAXIMUM_CURRENT_FAMILY_OFFERS,
    MAXIMUM_REALIZATION_FAMILIES, MAXIMUM_REALIZATION_FAMILY_PREREQUISITES,
};
pub use recursive_recovery::{
    prove_recursive_recovery, RecursiveRecoveryCandidate, RecursiveRecoveryEvidence,
    RecursiveRecoveryLimits, RecursiveRecoveryRefusal,
};
pub use replanning::{replan_selected_realizations_with_characteristics, RealizationReplanOutcome};
pub use requirements::{plan_with_hard_requirements, HardRealizationRequirements};
pub use style::{
    dos_shell_style, presentation_style_characteristics, select_realization_with_style, NamedStyle,
    PresentationStyleFacts, StyleId, StylePreferenceEvidence, StylePreferenceOutcome,
    StyleSelection, DOS_SHELL_STYLE_ID, PRESENTATION_DENSITY, PRESENTATION_FRAMING,
    PRESENTATION_KEYBOARD_VISIBLE, PRESENTATION_PALETTE_CLASS, PRESENTATION_TEXT_LAYOUT,
};
pub use survival_policy::{
    select_plan_with_survival_policy, triage_scarce_resource, ExplicitCriticality,
    ScarceResourceDecision, ScarceResourceDisposition, ScarceResourceTriage, SurvivalCandidate,
    SurvivalCandidateDisposition, SurvivalCandidateEvidence, SurvivalPlanSelection,
    SurvivalPlanningMode, SurvivalPlanningPolicy, SurvivalPolicyRefusal, SurvivalTradeoff,
    WorkloadResourceRequest, MAXIMUM_SCARCE_RESOURCE_REQUESTS, MAXIMUM_SURVIVAL_CANDIDATES,
    MAXIMUM_SURVIVAL_POLICY_ID_BYTES, MAXIMUM_SURVIVAL_TRADEOFFS,
};

pub fn default_placements(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    default_placements_unvalidated(&form.gears, hosts)
}

pub fn plan(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits(
        form,
        hosts,
        placements,
        bases,
        DEFAULT_CONNECTION_ITEM_CAPACITY,
        DEFAULT_CONNECTION_BYTE_CAPACITY,
    )
}

pub fn plan_with_authority_grants(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    authority_grants: &[AuthorityGrant],
) -> Result<Plan, PlannerError> {
    plan_with_options(
        form,
        hosts,
        placements,
        bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
}

pub fn plan_with_line_offers(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
    line_offers: &[LineOffer],
) -> Result<Plan, PlannerError> {
    let mut offered_bases = bases.to_vec();
    for offer in line_offers {
        if !offered_bases.contains(&offer.binding.base) {
            offered_bases.push(offer.binding.base.clone());
        }
    }
    plan_with_options(
        form,
        hosts,
        placements,
        &offered_bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers,
        },
    )
}

pub fn plan_with_connection_limits(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits_and_base_overrides(
        form,
        hosts,
        placements,
        bases,
        &BTreeMap::new(),
        connection_item_capacity,
        connection_byte_capacity,
    )
}

pub fn plan_with_connection_limits_and_base_overrides(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    connection_bases: &BTreeMap<(GearId, GearId), BaseImplementationId>,
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_options(
        form,
        hosts,
        placements,
        bases,
        PlanningOptions {
            connection_bases,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
}

pub fn plan_with_options(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    form.validate_identities()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    plan_validated_form(form, hosts, placements, bases, options)
}

pub(crate) fn plan_validated_form(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    plan_validated_form_with_connection_limits(
        form,
        hosts,
        placements,
        bases,
        options,
        &BTreeMap::new(),
    )
}

pub(crate) fn plan_validated_form_with_connection_limits(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let PlanningOptions {
        connection_bases,
        line_candidates,
        connection_item_capacity,
        connection_byte_capacity,
        authority_grants,
        protected_resource_grants,
        line_offers,
    } = options;
    if connection_item_capacity == 0 || connection_byte_capacity == 0 {
        return Err(PlannerError::InvalidConnectionBudget(
            "item and byte capacity must both be nonzero".to_string(),
        ));
    }
    for (endpoints, limits) in connection_limits {
        if limits.item_capacity == 0 || limits.byte_capacity == 0 {
            return Err(PlannerError::InvalidConnectionBudget(
                "per-connection item and byte capacity must both be nonzero".to_string(),
            ));
        }
        if !form
            .connections
            .iter()
            .any(|connection| connection_endpoints(connection) == *endpoints)
        {
            return Err(PlannerError::InvalidConnectionBudget(
                "per-connection capacity names a Cord absent from the checked Form".to_string(),
            ));
        }
    }
    let host_index = hosts
        .iter()
        .map(|host| (host.host_id.clone(), host))
        .collect::<BTreeMap<_, _>>();

    for host in hosts {
        validate_host_resources(host)?;
    }
    validate_authority_grants(authority_grants)?;
    validate_protected_resource_grants(protected_resource_grants)?;
    validate_line_offers(line_offers)?;

    let mut placement_count = BTreeMap::<(HostId, CapabilityId), u16>::new();
    let mut resource_usage = BTreeMap::<(HostId, ResourcePoolId), u32>::new();
    let mut remaining_compute_minimum =
        compute_admission::admit_minima(form, &host_index, placements)?;
    let mut consumed_protected_handles = BTreeSet::new();
    let mut resource_writers = BTreeSet::new();
    let mut planned_gears = Vec::<PlannedGear>::new();
    let mut placement_lookup = BTreeMap::<GearId, PlacementId>::new();

    for gear in &form.gears {
        let choice = placements
            .by_gear
            .get(&gear.gear_id)
            .ok_or_else(|| PlannerError::MissingPlacement(gear.gear_id.as_str().to_string()))?;
        let host = host_index
            .get(&choice.host_id)
            .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().to_string()))?;
        let capability = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .ok_or_else(|| {
                PlannerError::UnknownCapability(choice.capability_id.as_str().to_string())
            })?;
        validate_operation_capability(gear, capability)?;

        let count = placement_count
            .entry((host.host_id.clone(), capability.capability_id.clone()))
            .or_insert(0);
        *count += 1;
        if *count > capability.limits.max_active_instances {
            return Err(PlannerError::CapabilityInstanceLimitExceeded(format!(
                "capability '{}' exceeds max {}",
                capability.capability_id.as_str(),
                capability.limits.max_active_instances
            )));
        }

        let resource_bindings = resource_binding::bind_resources(
            host,
            capability,
            gear,
            protected_resource_grants,
            resource_binding::ResourcePlanningState {
                writers: &mut resource_writers,
                usage: &mut resource_usage,
                compute_minimum: &mut remaining_compute_minimum,
                protected_handles: &mut consumed_protected_handles,
            },
        )?;

        let mut authority_bindings = Vec::with_capacity(capability.authority_requirements.len());
        for requirement in &capability.authority_requirements {
            let mut matches = authority_grants.iter().filter(|grant| {
                grant.contract_id == requirement.contract_id
                    && grant.host_operation_contract_id == requirement.host_operation_contract_id
                    && grant.subject_kind == requirement.subject_kind
                    && grant.host_id == host.host_id
                    && grant.boot_id == host.boot_id
                    && grant.capability_id == capability.capability_id
            });
            let Some(grant) = matches.next() else {
                return Err(PlannerError::AuthorityGrantMissing(format!(
                    "capability '{}' requires '{}' for subject '{}' on host '{}' boot '{}'",
                    capability.capability_id.as_str(),
                    requirement.contract_id.as_str(),
                    requirement.subject_kind.as_str(),
                    host.host_id.as_str(),
                    host.boot_id.as_str()
                )));
            };
            if matches.next().is_some() {
                return Err(PlannerError::AuthorityGrantAmbiguous(format!(
                    "multiple grants satisfy capability '{}' requirement '{}'",
                    capability.capability_id.as_str(),
                    requirement.contract_id.as_str()
                )));
            }
            authority_bindings.push(AuthorityBinding {
                grant_id: grant.grant_id.clone(),
                contract_id: grant.contract_id.clone(),
                host_operation_contract_id: grant.host_operation_contract_id.clone(),
                subject_kind: grant.subject_kind.clone(),
                host_id: grant.host_id.clone(),
                boot_id: grant.boot_id.clone(),
                capability_id: grant.capability_id.clone(),
            });
        }
        authority_bindings.sort();

        let placement_id = PlacementId::from(hash_string(&format!(
            "placement:{}:{}:{}:{}",
            form.checked_form_id.as_str(),
            gear.gear_id.as_str(),
            host.host_id.as_str(),
            capability.capability_id.as_str()
        )));
        placement_lookup.insert(gear.gear_id.clone(), placement_id.clone());
        planned_gears.push(PlannedGear {
            placement_id,
            gear_id: gear.gear_id.clone(),
            kind_id: capability.kind_id.clone(),
            kind_contract_revision: capability.kind_contract_revision.clone(),
            execution_profile_id: capability.implementation.execution_profile_id.clone(),
            configuration: gear.configuration.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            capability_id: capability.capability_id.clone(),
            implementation_id: capability.implementation.implementation_id.clone(),
            artifact_id: capability.implementation.artifact_id.clone(),
            realization_characteristics: Vec::new(),
            limits: capability.limits.clone(),
            inputs: capability.inputs.clone(),
            outputs: capability.outputs.clone(),
            host_operations: capability.host_operations.clone(),
            resources: resource_bindings,
            authority: authority_bindings,
            pool_references: gear.pool_references.clone(),
        });
    }

    if consumed_protected_handles.len() != protected_resource_grants.len() {
        return Err(PlannerError::InvalidProtectedResourceGrant(
            "every supplied protected-resource grant must be consumed by one exact planned role"
                .to_string(),
        ));
    }

    for gear in placements.by_gear.keys() {
        if !form.gears.iter().any(|item| &item.gear_id == gear) {
            return Err(PlannerError::UnknownGear(gear.as_str().to_string()));
        }
    }

    let mut planned_connections = Vec::<PlannedConnection>::new();
    for connection in &form.connections {
        let limits = connection_limits
            .get(&connection_endpoints(connection))
            .copied()
            .unwrap_or(ConnectionQueueLimits {
                item_capacity: connection_item_capacity,
                byte_capacity: connection_byte_capacity,
            });
        let source_placement = placement_lookup
            .get(&connection.source_gear_id)
            .ok_or_else(|| {
                PlannerError::UnknownGear(connection.source_gear_id.as_str().to_string())
            })?;
        let sink_placement = placement_lookup
            .get(&connection.sink_gear_id)
            .ok_or_else(|| {
                PlannerError::UnknownGear(connection.sink_gear_id.as_str().to_string())
            })?;
        let source_plan = planned_gears
            .iter()
            .find(|item| &item.placement_id == source_placement)
            .expect("source placement must exist");
        let sink_plan = planned_gears
            .iter()
            .find(|item| &item.placement_id == sink_placement)
            .expect("sink placement must exist");
        let (selected_line, admitted_lines) = select_line(LineSelection {
            source: source_plan,
            sink: sink_plan,
            bases,
            requested: connection_bases
                .get(&(
                    connection.source_gear_id.clone(),
                    connection.sink_gear_id.clone(),
                ))
                .cloned(),
            requested_candidates: line_candidates.get(&(
                connection.source_gear_id.clone(),
                connection.sink_gear_id.clone(),
            )),
            line_offers,
            connection_item_capacity: limits.item_capacity,
            connection_byte_capacity: limits.byte_capacity,
        })?;
        let source_capability =
            find_capability(hosts, &source_plan.host_id, &source_plan.capability_id)?;
        let sink_capability = find_capability(hosts, &sink_plan.host_id, &sink_plan.capability_id)?;
        if limits.item_capacity > source_capability.limits.max_queue_items
            || limits.item_capacity > sink_capability.limits.max_queue_items
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires item capacity {}",
                source_plan.gear_id.as_str(),
                sink_plan.gear_id.as_str(),
                limits.item_capacity
            )));
        }
        if limits.byte_capacity > source_capability.limits.max_queue_bytes
            || limits.byte_capacity > sink_capability.limits.max_queue_bytes
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires byte capacity {}",
                source_plan.gear_id.as_str(),
                sink_plan.gear_id.as_str(),
                limits.byte_capacity
            )));
        }
        planned_connections.push(PlannedConnection {
            connection_id: ConnectionId::from(hash_string(&format!(
                "connection:{}:{}:{}:{}:{}:{}:{}",
                form.checked_form_id.as_str(),
                connection.source_gear_id.as_str(),
                connection.source_port_id.as_str(),
                connection.sink_gear_id.as_str(),
                connection.sink_port_id.as_str(),
                connection.value_kind.as_str(),
                connection.temporal.as_str(),
            ))),
            source_placement_id: source_plan.placement_id.clone(),
            source_port_id: connection.source_port_id.clone(),
            sink_placement_id: sink_plan.placement_id.clone(),
            sink_port_id: connection.sink_port_id.clone(),
            value_kind: connection.value_kind.clone(),
            temporal: connection.temporal,
            selected_line,
            admitted_lines,
            item_capacity: limits.item_capacity,
            byte_capacity: limits.byte_capacity,
        });
    }

    let global_startup_order = startup::startup_order(&planned_gears, &planned_connections)
        .ok_or_else(|| PlannerError::CyclicStartupDependencies(form.name.clone()))?;

    let fragments = hosts
        .iter()
        .map(|host| -> Result<Option<PlanFragment>, PlannerError> {
            let placements = planned_gears
                .iter()
                .filter(|item| item.host_id == host.host_id)
                .cloned()
                .collect::<Vec<_>>();
            if placements.is_empty() {
                return Ok(None);
            }
            let connections = planned_connections
                .iter()
                .filter(|connection| {
                    placements
                        .iter()
                        .any(|item| item.placement_id == connection.source_placement_id)
                        || placements
                            .iter()
                            .any(|item| item.placement_id == connection.sink_placement_id)
                })
                .cloned()
                .collect::<Vec<_>>();
            let startup_order = global_startup_order
                .iter()
                .filter(|placement_id| {
                    placements
                        .iter()
                        .any(|placement| &placement.placement_id == *placement_id)
                })
                .cloned()
                .collect();
            let startup_dependencies = connections
                .iter()
                .filter(|connection| connection.source_placement_id != connection.sink_placement_id)
                .map(|connection| StartupDependency {
                    prerequisite_placement_id: connection.sink_placement_id.clone(),
                    dependent_placement_id: connection.source_placement_id.clone(),
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let expected_terminals = placements
                .iter()
                .map(|placement| {
                    ExpectedTerminal::PlacementCompleted(placement.placement_id.clone())
                })
                .chain(connections.iter().map(|connection| {
                    ExpectedTerminal::ConnectionCompleted(connection.connection_id.clone())
                }))
                .chain(core::iter::once(ExpectedTerminal::PlanCompleted))
                .collect();
            let expected_sign = core::iter::once(ExpectedSign::PlanFragmentReceived)
                .chain(placements.iter().map(|placement| {
                    ExpectedSign::PlacementPrepared(placement.placement_id.clone())
                }))
                .chain(placements.iter().map(|placement| {
                    ExpectedSign::PlacementTerminal(placement.placement_id.clone())
                }))
                .chain(connections.iter().map(|connection| {
                    ExpectedSign::ConnectionTerminal(connection.connection_id.clone())
                }))
                .chain(core::iter::once(ExpectedSign::PlanTerminal))
                .collect::<Vec<_>>();
            let sign_storage_budget = mandatory_sign_storage_requirement(&expected_sign)
                .ok_or_else(|| {
                    PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            Ok(Some(PlanFragment {
                plan_id: PlanId::from(""),
                fragment_id: FragmentId::from(""),
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
                realization_backs: Vec::new(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                placements,
                execution_regions: Vec::new(),
                execution_fusions: Vec::new(),
                connections,
                shared_pools: Vec::new(),
                startup_dependencies,
                startup_order,
                cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
                terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
                expected_terminals,
                expected_sign,
                sign_storage_budget,
                plan_fragments: Vec::new(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(seal_plan(form.identity(), fragments))
}

fn connection_endpoints(connection: &conduit_form::CheckedConnection) -> ConnectionEndpoints {
    (
        connection.source_gear_id.clone(),
        connection.source_port_id.clone(),
        connection.sink_gear_id.clone(),
        connection.sink_port_id.clone(),
    )
}

fn validate_operation_capability(
    gear: &CheckedGear,
    capability: &conduit_core::CapabilityOffer,
) -> Result<(), PlannerError> {
    if capability.checked_face() != gear.checked_face() {
        return Err(PlannerError::IncompatibleCheckedFace(format!(
            "gear '{}' face differs from capability '{}' face",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if capability.host_operations.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement
                .target_kind
                .as_ref()
                .is_some_and(|target| target.as_str().is_empty())
            || requirement.maximum_in_flight == 0
    }) || capability
        .host_operations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidHostOperationRequirement(format!(
            "capability '{}' requirements must have non-empty identities, unique canonical ordering, and nonzero in-flight bounds",
            capability.capability_id.as_str()
        )));
    }
    if capability.resource_requirements.iter().any(|requirement| {
        requirement.class_id.as_str().is_empty()
            || requirement.units == 0
            || requirement
                .compute
                .as_ref()
                .is_some_and(|compute| !compute.is_valid_for_units(requirement.units))
            || requirement
                .protected_role
                .as_ref()
                .is_some_and(|role| role.as_str().is_empty())
    }) || capability
        .resource_requirements
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "capability '{}' requirements must have non-empty classes and protected roles, positive units, and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    if capability.authority_requirements.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement.host_operation_contract_id.as_str().is_empty()
            || requirement.subject_kind.as_str().is_empty()
            || !capability.host_operations.iter().any(|host_operation| {
                host_operation.contract_id == requirement.host_operation_contract_id
                    && host_operation.target_kind.as_ref() == Some(&requirement.subject_kind)
            })
    }) || capability
        .authority_requirements
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidAuthorityContract(format!(
            "capability '{}' authority requirements must bind a declared targeted host operation with non-empty identities and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    Ok(())
}

fn validate_authority_grants(grants: &[AuthorityGrant]) -> Result<(), PlannerError> {
    if grants.iter().any(|grant| {
        grant.grant_id.as_str().is_empty()
            || grant.contract_id.as_str().is_empty()
            || grant.host_operation_contract_id.as_str().is_empty()
            || grant.subject_kind.as_str().is_empty()
            || grant.host_id.as_str().is_empty()
            || grant.boot_id.as_str().is_empty()
            || grant.capability_id.as_str().is_empty()
    }) {
        return Err(PlannerError::InvalidAuthorityContract(
            "grants must have non-empty immutable scope identities".to_string(),
        ));
    }
    let unique_ids = grants
        .iter()
        .map(|grant| &grant.grant_id)
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != grants.len() {
        return Err(PlannerError::InvalidAuthorityContract(
            "grant identities must be unique".to_string(),
        ));
    }
    Ok(())
}

fn validate_host_resources(host: &HostAdvertisement) -> Result<(), PlannerError> {
    if host.resources.iter().any(|resource| {
        resource.pool_id.as_str().is_empty()
            || resource.class_id.as_str().is_empty()
            || resource.capacity_units == 0
            || resource
                .compute
                .as_ref()
                .is_some_and(|compute| !compute.is_valid_for_capacity(resource.capacity_units))
    }) || host
        .resources
        .windows(2)
        .any(|pair| pair[0].pool_id >= pair[1].pool_id)
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "host '{}' pools must have non-empty identities, positive capacity, and unique pool-id ordering",
            host.host_id.as_str()
        )));
    }
    Ok(())
}

fn find_capability<'a>(
    hosts: &'a [HostAdvertisement],
    host_id: &HostId,
    capability_id: &CapabilityId,
) -> Result<&'a conduit_core::CapabilityOffer, PlannerError> {
    hosts
        .iter()
        .find(|host| &host.host_id == host_id)
        .and_then(|host| {
            host.capabilities
                .iter()
                .find(|item| &item.capability_id == capability_id)
        })
        .ok_or_else(|| PlannerError::UnknownCapability(capability_id.as_str().to_string()))
}

struct LineSelection<'a> {
    source: &'a PlannedGear,
    sink: &'a PlannedGear,
    bases: &'a [BaseImplementationId],
    requested: Option<BaseImplementationId>,
    requested_candidates: Option<&'a Vec<LineId>>,
    line_offers: &'a [LineOffer],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
}

fn select_line(
    selection: LineSelection<'_>,
) -> Result<(Option<AdmittedLine>, Vec<AdmittedLine>), PlannerError> {
    let LineSelection {
        source,
        sink,
        bases,
        requested,
        requested_candidates,
        line_offers,
        connection_item_capacity,
        connection_byte_capacity,
    } = selection;
    if source.host_id == sink.host_id {
        if requested.is_some_and(|base| base != BaseImplementationId::from("conduit.base/local@1"))
            || !bases.contains(&BaseImplementationId::from("conduit.base/local@1"))
        {
            return Err(PlannerError::UnavailableBaseImplementationId(format!(
                "local base unavailable for '{}' > '{}'",
                source.gear_id.as_str(),
                sink.gear_id.as_str()
            )));
        }
        if requested_candidates.is_some_and(|candidates| !candidates.is_empty()) {
            return Err(PlannerError::InvalidLineOffer(
                "local Cords cannot seal remote Line candidates".to_string(),
            ));
        }
        return Ok((None, Vec::new()));
    }

    if requested == Some(BaseImplementationId::from("conduit.base/local@1")) {
        return Err(PlannerError::UnavailableBaseImplementationId(format!(
            "local base cannot connect '{}' > '{}'",
            source.gear_id.as_str(),
            sink.gear_id.as_str()
        )));
    }
    let endpoint_matches = |offer: &&LineOffer| {
        offer.binding.source.host_id == source.host_id
            && offer.binding.source.boot_id == source.boot_id
            && offer.binding.sink.host_id == sink.host_id
            && offer.binding.sink.boot_id == sink.boot_id
            && requested
                .as_ref()
                .is_none_or(|base| &offer.binding.base == base)
            && bases.contains(&offer.binding.base)
    };
    let exact = line_offers
        .iter()
        .filter(endpoint_matches)
        .collect::<Vec<_>>();
    if exact.is_empty() {
        return Err(PlannerError::LineOfferMissing(format!(
            "no boot-scoped Line offered for '{}' > '{}'",
            source.gear_id.as_str(),
            sink.gear_id.as_str()
        )));
    }
    let ready = exact
        .into_iter()
        .filter(|offer| {
            offer.availability.availability == LineAvailability::Ready
                && offer.binding.limits.maximum_in_flight_items >= connection_item_capacity
                && offer.binding.limits.maximum_payload_bytes >= connection_byte_capacity
                && offer.binding.limits.maximum_buffered_bytes >= connection_byte_capacity
                && offer.binding.limits.maximum_frame_bytes
                    >= offer.binding.limits.maximum_payload_bytes
        })
        .collect::<Vec<_>>();
    if ready.is_empty() {
        return Err(PlannerError::LineOfferUnavailable(format!(
            "offered Line for '{}' > '{}' is unavailable or below item/byte limits",
            source.gear_id.as_str(),
            sink.gear_id.as_str()
        )));
    }
    if let Some(requested_candidates) = requested_candidates {
        let unique_candidates = requested_candidates.iter().collect::<BTreeSet<_>>();
        if requested_candidates.is_empty() || unique_candidates.len() != requested_candidates.len()
        {
            return Err(PlannerError::InvalidLineOffer(
                "Line candidate policy must be non-empty and contain no duplicates".to_string(),
            ));
        }
        let mut selected = Vec::with_capacity(requested_candidates.len());
        for line_id in requested_candidates {
            let matches = ready
                .iter()
                .filter(|offer| &offer.line_id == line_id)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(PlannerError::LineOfferMissing(format!(
                    "requested Line '{}' is not one exact ready bounded offer",
                    line_id.as_str()
                )));
            }
            selected.push(matches[0].admitted_line());
        }
        let first = selected[0].clone();
        return Ok((Some(first), selected));
    }
    if ready.len() != 1 {
        return Err(PlannerError::LineOfferAmbiguous(format!(
            "multiple offered Lines satisfy '{}' > '{}'",
            source.gear_id.as_str(),
            sink.gear_id.as_str()
        )));
    }
    let line = ready[0].admitted_line();
    Ok((Some(line.clone()), vec![line]))
}

fn validate_line_offers(offers: &[LineOffer]) -> Result<(), PlannerError> {
    if offers.iter().any(|offer| {
        let binding = &offer.binding;
        offer.line_id.as_str().is_empty()
            || !offer.validate_sign_identity()
            || binding.binding_id.as_str().is_empty()
            || offer.line_id.as_str() == binding.binding_id.as_str()
            || offer.line_id.as_str() == binding.base_instance_id.as_str()
            || offer.line_id.as_str() == binding.source.endpoint_id.as_str()
            || offer.line_id.as_str() == binding.sink.endpoint_id.as_str()
            || binding.binding_id.as_str() == binding.base_instance_id.as_str()
            || binding.source.host_id.as_str().is_empty()
            || binding.source.boot_id.as_str().is_empty()
            || binding.source.endpoint_id.as_str().is_empty()
            || binding.sink.host_id.as_str().is_empty()
            || binding.sink.boot_id.as_str().is_empty()
            || binding.sink.endpoint_id.as_str().is_empty()
            || binding.source.endpoint_id == binding.sink.endpoint_id
            || binding.source.host_id == binding.sink.host_id
            || binding.base == BaseImplementationId::from("conduit.base/local@1")
            || binding.base_instance_id.as_str().is_empty()
            || binding.limits.maximum_in_flight_items == 0
            || binding.limits.maximum_payload_bytes == 0
            || binding.limits.maximum_buffered_bytes == 0
            || binding.limits.maximum_frame_bytes < binding.limits.maximum_payload_bytes
            || matches!(
                &binding.credential,
                conduit_core::LinkCredentialReference::Opaque(reference)
                    if reference.as_str().is_empty()
            )
            || matches!(
                &binding.authority,
                conduit_core::LinkAuthorityReference::Grant(grant_id)
                    if grant_id.as_str().is_empty()
            )
    }) {
        return Err(PlannerError::InvalidLineOffer(
            "remote Line offers require distinct Line/binding/Base identities, one matching availability Sign, non-empty boot-scoped endpoints, and positive finite limits".to_string(),
        ));
    }
    let unique_lines = offers
        .iter()
        .map(|offer| &offer.line_id)
        .collect::<BTreeSet<_>>();
    let unique_bindings = offers
        .iter()
        .map(|offer| &offer.binding.binding_id)
        .collect::<BTreeSet<_>>();
    if unique_lines.len() != offers.len() || unique_bindings.len() != offers.len() {
        return Err(PlannerError::InvalidLineOffer(
            "Line and lower binding identities must each be unique".to_string(),
        ));
    }
    Ok(())
}

fn hash_string(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble out of range"),
    }
}

#[cfg(test)]
mod tests;
