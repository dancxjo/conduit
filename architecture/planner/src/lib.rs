#![no_std]
#![doc = r#"
# Planner architecture

The ordinary v1 path is intentionally small:

1. `default_placements` derives functionally valid placement choices from a
   checked plot and current host advertisements.
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
use completion::plan_completion_policy;
use conduit_core::{
    mandatory_sign_storage_requirement, seal_plan_with_completion, AdmittedLine, AuthorityBinding,
    AuthorityGrant, BaseImplementationId, CancellationPolicy, CapabilityId, ConnectionId,
    DeliveryPressurePolicy, ExpectedSign, ExpectedTerminal, FragmentId, GearId, HostAdvertisement,
    HostId, LineAvailability, LineId, LineOffer, PlacementId, Plan, PlanFragment, PlanId,
    PlannedConnection, PlannedGear, PlannedStateBoundary, ResourcePoolId, StateContinuation,
    StateId, StateLifetime, TerminalPolicy, DEFAULT_CONNECTION_BYTE_CAPACITY,
    DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_plot::{CheckedGear, CheckedPlot};
use sha2::{Digest, Sha256};

mod accelerator;
mod advice;
mod body_envelope;
mod canonical;
mod characteristic_policy;
mod characteristic_sealing;
mod characteristics;
mod completion;
mod compute_admission;
mod contract;
mod decision_evidence;
mod degradation;
#[cfg(test)]
mod degradation_explanation_tests;
mod degraded_profile;
mod degraded_profile_explanation;
#[cfg(test)]
mod degraded_profile_explanation_tests;
mod diagnostic;
mod diversity;
mod dormant_readmission;
mod dormant_readmission_explanation;
#[cfg(test)]
mod dormant_readmission_explanation_tests;
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
mod policy_explanation;
mod profile;
pub mod proof;
mod protected_resources;
mod realization;
mod realization_families;
mod realization_recovery;
mod recursive_recovery;
mod recursive_recovery_explanation;
mod replanning;
mod requirements;
mod resource_binding;
mod resource_port;
mod retry;
mod selected_plan_sealing;
mod startup;
pub mod state_delay;
pub mod wcet;
#[cfg(test)]
use startup::startup_order;
mod style;
mod survival_policy;
mod survival_policy_explanation;
#[cfg(test)]
mod survival_policy_explanation_tests;

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
    default_expanded_placements, plan_canonical_realization_with_options,
    plan_expanded_authoring_with_options, plan_expanded_canonical,
    plan_expanded_canonical_with_activations, plan_expanded_canonical_with_connection_limits,
    plan_expanded_canonical_with_options, plan_expanded_canonical_with_shared_pools,
    CanonicalRealizationMode, CanonicalRealizationSelectionError, ForeBoundaryKey,
    PlannedCanonicalRealization, SharedPoolPlanningRequirement,
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
    assess_scoped_degradation, DegradationAssessment, DegradationExplanation, DegradationFragment,
    DegradationFragmentDisposition, DegradationInput, MAXIMUM_DEGRADATION_EXPLANATION_BYTES,
    MAXIMUM_DEGRADATION_FRAGMENTS, MAXIMUM_DEGRADATION_FRAGMENT_ID_BYTES,
    MAXIMUM_DEGRADATION_REFUSAL_BYTES,
};
pub use degraded_profile::{
    seal_reviewed_service_profile_plan, select_reviewed_service_profile, DegradationDirection,
    DegradedDimension, DegradedDimensionEvidence, DegradedProfileRefusal, ReviewedServiceProfile,
    ServiceProfileAdmission, ServiceProfileDisposition, SurvivalPolicy,
    MAXIMUM_DEGRADED_PROFILE_DIMENSIONS, MAXIMUM_DEGRADED_PROFILE_ID_BYTES,
    MAXIMUM_DEGRADED_PROFILE_LABEL_BYTES,
};
pub use degraded_profile_explanation::{
    explain_degraded_profile, explain_degraded_profile_refusal, DegradedProfileExplanation,
    DegradedProfileExplanationError, DegradedProfileState, ProfileDimensionExplanation,
    MAXIMUM_DEGRADED_PROFILE_EXPLANATION_BYTES,
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
pub use dormant_readmission_explanation::{
    explain_dormant_readmission, DormantReadmissionExplanation, DormantReadmissionExplanationError,
    MAXIMUM_DORMANT_READMISSION_EXPLANATION_BYTES,
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
pub use policy_explanation::{
    PolicyChoiceDetails, PolicyChoiceDomain, PolicyChoiceExplanation, PolicyChoiceSummary,
    PolicyExplanationError, PolicyReplanRequest, MAX_POLICY_EXPLANATIONS,
    MAX_STYLE_EXPLANATION_CLAUSES,
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
pub use realization_recovery::{
    admit_realization_recovery, RealizationInvalidation, RealizationRecoveryOutcome,
    RealizationRecoveryRefusal, RealizationReplacementEvidence, RecoveryPlanningOutcome,
    MAXIMUM_INVALIDATED_REALIZATION_LINES, MAXIMUM_RECOVERY_REFUSAL_BYTES,
};
pub use recursive_recovery::{
    prove_recursive_recovery, RecursiveRecoveryCandidate, RecursiveRecoveryEvidence,
    RecursiveRecoveryLimits, RecursiveRecoveryRefusal,
};
pub use recursive_recovery_explanation::{
    explain_recursive_recovery, RecursiveRecoveryExplanation, RecursiveRecoveryExplanationError,
    MAXIMUM_RECURSIVE_RECOVERY_EXPLANATION_BYTES,
};
pub use replanning::{replan_selected_realizations_with_characteristics, RealizationReplanOutcome};
pub use requirements::{plan_with_hard_requirements, HardRealizationRequirements};
pub use retry::{admit_explicit_retry, RetryAdmission, RetryAdmissionBasis};
pub use selected_plan_sealing::seal_exact_plan_with_selected_realizations;
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
pub use survival_policy_explanation::{
    explain_survival_plan_selection, SurvivalPolicyExplanation, SurvivalPolicyExplanationError,
    MAXIMUM_SURVIVAL_POLICY_EXPLANATION_BYTES,
};
pub use wcet::{
    admit_deadline_region, validate_replan, DeadlineAdmission, DeadlineRegion, TimingDependency,
    TimingFacts, WcetRefusal,
};

pub fn default_placements(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    default_placements_unvalidated(&plot.gears, hosts)
}

/// Plans semantic work under an explicit closed-world set of Cord mechanisms.
///
/// `allowed_line_bases` governs only local Cord and remote Line realization.
/// Capability/resource Base providers are selected exclusively from each
/// current `HostAdvertisement::bases` entry and do not belong in this list.
pub fn plan(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    allowed_line_bases: &[BaseImplementationId],
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits(
        plot,
        hosts,
        placements,
        allowed_line_bases,
        DEFAULT_CONNECTION_ITEM_CAPACITY,
        DEFAULT_CONNECTION_BYTE_CAPACITY,
    )
}

pub fn plan_with_authority_grants(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    allowed_line_bases: &[BaseImplementationId],
    authority_grants: &[AuthorityGrant],
) -> Result<Plan, PlannerError> {
    plan_with_options(
        plot,
        hosts,
        placements,
        allowed_line_bases,
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
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    allowed_line_bases: &[BaseImplementationId],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
    line_offers: &[LineOffer],
) -> Result<Plan, PlannerError> {
    let mut offered_bases = allowed_line_bases.to_vec();
    for offer in line_offers {
        if !offered_bases.contains(&offer.binding.base) {
            offered_bases.push(offer.binding.base.clone());
        }
    }
    plan_with_options(
        plot,
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
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    allowed_line_bases: &[BaseImplementationId],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits_and_base_overrides(
        plot,
        hosts,
        placements,
        allowed_line_bases,
        &BTreeMap::new(),
        connection_item_capacity,
        connection_byte_capacity,
    )
}

pub fn plan_with_connection_limits_and_base_overrides(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    allowed_line_bases: &[BaseImplementationId],
    connection_bases: &BTreeMap<(GearId, GearId), BaseImplementationId>,
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_options(
        plot,
        hosts,
        placements,
        allowed_line_bases,
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
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    allowed_line_bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    plot.validate_identities()
        .map_err(|error| PlannerError::InvalidPlotIdentity(error.to_string()))?;
    plan_validated_plot(plot, hosts, placements, allowed_line_bases, options)
}

pub(crate) fn plan_validated_plot(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    plan_validated_plot_with_connection_limits(
        plot,
        hosts,
        placements,
        bases,
        options,
        &BTreeMap::new(),
    )
}

pub(crate) fn plan_validated_plot_with_connection_limits(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let line_policy = contract::LineMechanismPolicy::new(bases);
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
        if !plot
            .connections
            .iter()
            .any(|connection| connection_endpoints(connection) == *endpoints)
        {
            return Err(PlannerError::InvalidConnectionBudget(
                "per-connection capacity names a Cord absent from the checked plot".to_string(),
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
        compute_admission::admit_minima(plot, &host_index, placements)?;
    let mut consumed_protected_handles = BTreeSet::new();
    let mut resource_writers = BTreeSet::new();
    let mut planned_gears = Vec::<PlannedGear>::new();
    let mut placement_lookup = BTreeMap::<GearId, PlacementId>::new();

    for gear in &plot.gears {
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
        validate_keep_retention(gear, capability)?;

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
        let base = selected_base_provider(host, capability, &resource_bindings)?;

        let mut authority_bindings = Vec::with_capacity(capability.authority_requirements.len());
        for requirement in &capability.authority_requirements {
            let mut matches = authority_grants.iter().filter(|grant| {
                grant.contract_id == requirement.contract_id
                    && grant.host_call_contract_id == requirement.host_call_contract_id
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
                host_call_contract_id: grant.host_call_contract_id.clone(),
                subject_kind: grant.subject_kind.clone(),
                host_id: grant.host_id.clone(),
                boot_id: grant.boot_id.clone(),
                capability_id: grant.capability_id.clone(),
            });
        }
        authority_bindings.sort();

        let placement_id = PlacementId::from(hash_string(&format!(
            "placement:{}:{}:{}:{}",
            plot.checked_plot_id.as_str(),
            gear.gear_id.as_str(),
            host.host_id.as_str(),
            capability.capability_id.as_str()
        )));
        placement_lookup.insert(gear.gear_id.clone(), placement_id.clone());
        planned_gears.push(conduit_core::planned_gear_from_parts! {
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
            base,
            realization_characteristics: Vec::new(),
            limits: capability.limits.clone(),
            inputs: capability.inputs.clone(),
            outputs: capability.outputs.clone(),
            semantic_contract: capability.semantic_contract.clone(),
            terminal_transductions: gear.terminal_transductions.clone(),
            host_calls: capability.host_calls.clone(),
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
        if !plot.gears.iter().any(|item| &item.gear_id == gear) {
            return Err(PlannerError::UnknownGear(gear.as_str().to_string()));
        }
    }

    let mut planned_connections = Vec::<PlannedConnection>::new();
    for connection in &plot.connections {
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
        let source_gear = plot
            .gears
            .iter()
            .find(|gear| gear.gear_id == connection.source_gear_id)
            .expect("checked source gear must exist");
        let sink_gear = plot
            .gears
            .iter()
            .find(|gear| gear.gear_id == connection.sink_gear_id)
            .expect("checked sink gear must exist");
        let resource = resource_port::plan_resource_connection(
            connection,
            source_gear,
            sink_gear,
            source_plan,
            sink_plan,
        )?;
        let (selected_line, admitted_lines) = select_line(LineSelection {
            source: source_plan,
            sink: sink_plan,
            policy: line_policy,
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
                "connection:{}:{}:{}:{}:{}:{}:{}:{}",
                plot.checked_plot_id.as_str(),
                connection.source_gear_id.as_str(),
                connection.source_port_id.as_str(),
                connection.sink_gear_id.as_str(),
                connection.sink_port_id.as_str(),
                connection.value_kind.as_str(),
                connection.track.as_str(),
                connection.temporal.as_str(),
            ))),
            source_placement_id: source_plan.placement_id.clone(),
            source_port_id: connection.source_port_id.clone(),
            sink_placement_id: sink_plan.placement_id.clone(),
            sink_port_id: connection.sink_port_id.clone(),
            value_kind: connection.value_kind.clone(),
            resource,
            abnormal_kind: source_capability
                .outputs
                .iter()
                .find(|port| port.port_id == connection.source_port_id)
                .and_then(|port| port.abnormal_kind.clone()),
            track: connection.track,
            temporal: connection.temporal,
            pressure_policy: if source_plan.kind_id.as_str() == "flow/coalesce-latest" {
                DeliveryPressurePolicy::CoalesceLatest
            } else {
                DeliveryPressurePolicy::PreserveOrder
            },
            selected_line,
            admitted_lines,
            item_capacity: limits.item_capacity,
            byte_capacity: limits.byte_capacity,
        });
    }

    let global_startup_order = startup::startup_order(&planned_gears, &planned_connections)?
        .ok_or_else(|| PlannerError::CyclicStartupDependencies(plot.name.clone()))?;

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
            let startup_dependencies = startup::startup_dependencies(&placements, &connections)?;
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
            let states = placements
                .iter()
                .map(planned_keep_state)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
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
            let mut sign_storage_budget = mandatory_sign_storage_requirement(&expected_sign)
                .ok_or_else(|| {
                    PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            let state_budget = conduit_core::state_resource_budget(&states).map_err(|error| {
                PlannerError::InvalidStateContract(format!(
                    "host '{}' retained State admission: {error:?}",
                    host.host_id.as_str()
                ))
            })?;
            sign_storage_budget.item_capacity = sign_storage_budget
                .item_capacity
                .checked_add(state_budget.sign_storage.item_capacity)
                .ok_or_else(|| {
                    PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            sign_storage_budget.byte_capacity = sign_storage_budget
                .byte_capacity
                .checked_add(state_budget.sign_storage.byte_capacity)
                .ok_or_else(|| {
                    PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            Ok(Some(PlanFragment {
                plan_id: PlanId::from(""),
                fragment_id: FragmentId::from(""),
                source_document_id: plot.source_document_id.clone(),
                checked_plot_id: plot.checked_plot_id.clone(),
                expanded_plot_id: plot.expanded_plot_id.clone(),
                completion_policy: plan_completion_policy(plot.completion),
                realization_backs: Vec::new(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                placements,
                execution_regions: Vec::new(),
                execution_fusions: Vec::new(),
                states,
                connections,
                fore_ports: Vec::new(),
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

    Ok(seal_plan_with_completion(
        plot.identity(),
        plan_completion_policy(plot.completion),
        fragments,
    ))
}

fn planned_keep_state(
    placement: &PlannedGear,
) -> Result<Option<PlannedStateBoundary>, PlannerError> {
    let Some(duration) = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "retained-duration")
    else {
        return Ok(None);
    };
    if !matches!(
        placement.kind_id.as_str(),
        "state/latest" | conduit_core::STATE_VALUE_KIND
    ) {
        return Err(PlannerError::InvalidStateContract(format!(
            "gear '{}' attaches retained duration to non-State Kind '{}'",
            placement.gear_id.as_str(),
            placement.kind_id.as_str()
        )));
    }
    let conduit_core::ConfigurationValue::Text(duration) = &duration.value else {
        return Err(PlannerError::InvalidStateContract(format!(
            "gear '{}' retained duration is not canonical text",
            placement.gear_id.as_str()
        )));
    };
    let lifetime = state_lifetime(duration, &placement.gear_id)?;
    let maximum = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "maximum-bytes")
        .and_then(|entry| match &entry.value {
            conduit_core::ConfigurationValue::U64(value) => u32::try_from(*value).ok(),
            _ => None,
        })
        .ok_or_else(|| {
            PlannerError::InvalidStateContract(format!(
                "gear '{}' has no exact retained value bound",
                placement.gear_id.as_str()
            ))
        })?;
    if maximum > placement.limits.max_queue_bytes {
        return Err(PlannerError::InvalidStateContract(format!(
            "gear '{}' requires {} retained value bytes, but its selected Back admits {}",
            placement.gear_id.as_str(),
            maximum,
            placement.limits.max_queue_bytes
        )));
    }
    let [input] = placement.inputs.as_slice() else {
        return Err(PlannerError::InvalidStateContract(format!(
            "gear '{}' retained State must have one input",
            placement.gear_id.as_str()
        )));
    };
    let [output] = placement.outputs.as_slice() else {
        return Err(PlannerError::InvalidStateContract(format!(
            "gear '{}' retained State must have one output",
            placement.gear_id.as_str()
        )));
    };
    if input.value_kind != output.value_kind
        || output.temporal != conduit_core::PortTemporal::Current
    {
        return Err(PlannerError::InvalidStateContract(format!(
            "gear '{}' retained State Fore is inconsistent",
            placement.gear_id.as_str()
        )));
    }
    let initial_value = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "initial")
        .map(|entry| match &entry.value {
            conduit_core::ConfigurationValue::Quantity(value) => Ok(value.encode().to_vec()),
            conduit_core::ConfigurationValue::Structured(value) => {
                let structured = conduit_core::StructuredInfoValue::from_canonical_bytes(
                    value.canonical_value(),
                )
                .map_err(|_| {
                    PlannerError::InvalidStateContract(format!(
                        "gear '{}' retained initializer is malformed",
                        placement.gear_id.as_str()
                    ))
                })?;
                match (structured.value_type().shape(), structured.shape()) {
                    (
                        conduit_core::StructuredInfoTypeShape::Leaf(kind),
                        conduit_core::StructuredInfoValueShape::Leaf(bytes),
                    ) if output.value_kind == *kind => Ok(bytes.to_vec()),
                    _ => Ok(value.canonical_value().to_vec()),
                }
            }
            _ => Err(PlannerError::InvalidStateContract(format!(
                "gear '{}' retained initializer has no canonical encoding",
                placement.gear_id.as_str()
            ))),
        })
        .transpose()?;
    Ok(Some(PlannedStateBoundary {
        state_id: StateId::from(placement.gear_id.as_str()),
        gear_id: placement.gear_id.clone(),
        value_kind: output.value_kind.clone(),
        initial_value,
        lifetime,
        retained: None,
        maximum_value_bytes: maximum,
        continuation: StateContinuation::ExternallyBounded,
    }))
}

fn validate_keep_retention(
    gear: &CheckedGear,
    capability: &conduit_core::CapabilityOffer,
) -> Result<(), PlannerError> {
    capability.validate_state_retention().map_err(|_| {
        PlannerError::InvalidStateContract(format!(
            "capability '{}' attaches State retention to non-State Kind '{}'",
            capability.capability_id.as_str(),
            capability.kind_id.as_str()
        ))
    })?;
    let Some(duration) = gear
        .configuration
        .iter()
        .find(|entry| entry.key == "retained-duration")
    else {
        return Ok(());
    };
    let conduit_core::ConfigurationValue::Text(duration) = &duration.value else {
        return Err(PlannerError::InvalidStateContract(format!(
            "gear '{}' retained duration is not canonical text",
            gear.gear_id.as_str()
        )));
    };
    let required = state_lifetime(duration, &gear.gear_id)?;
    if capability
        .state_retention
        .is_none_or(|support| !support.supports(required))
    {
        return Err(PlannerError::StateRetentionUnsupported(format!(
            "gear '{}' requires {:?}, but capability '{}' supports at most {}",
            gear.gear_id.as_str(),
            required,
            capability.capability_id.as_str(),
            capability
                .state_retention
                .map(|support| format!("{:?}", support.maximum_lifetime))
                .unwrap_or_else(|| "no keep duration".to_string())
        )));
    }
    Ok(())
}

fn state_lifetime(duration: &str, gear_id: &GearId) -> Result<StateLifetime, PlannerError> {
    match duration {
        "step" => Ok(StateLifetime::Step),
        "play" => Ok(StateLifetime::Play),
        "wake" => Ok(StateLifetime::Wake),
        "boot" => Ok(StateLifetime::Boot),
        "body" => Ok(StateLifetime::Body),
        _ => Err(PlannerError::InvalidStateContract(format!(
            "gear '{}' has unknown retained duration '{}'",
            gear_id.as_str(),
            duration
        ))),
    }
}

fn connection_endpoints(connection: &conduit_plot::CheckedConnection) -> ConnectionEndpoints {
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
    if capability.kind_id != gear.kind_id {
        return Err(PlannerError::WrongSemanticKind(format!(
            "gear '{}' Kind differs from capability '{}' Kind",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if capability.checked_front() != gear.checked_front() {
        return Err(PlannerError::IncompatibleCheckedFront(format!(
            "gear '{}' front differs from capability '{}' front",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if !gear.accepts_semantic_contract(capability) {
        return Err(PlannerError::WrongKindContractRevision(format!(
            "gear '{}' semantic laws or configuration contract differ from capability '{}'",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if gear.kind_contract_revision.as_str() != conduit_core::STRUCTURAL_POLYMORPHIC_CONTRACT
        && capability.kind_contract_revision != gear.kind_contract_revision
    {
        return Err(PlannerError::WrongKindContractRevision(format!(
            "gear '{}' semantic contract differs from capability '{}'",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if capability.host_calls.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement
                .target_kind
                .as_ref()
                .is_some_and(|target| target.as_str().is_empty())
            || requirement.maximum_in_flight == 0
    }) || capability
        .host_calls
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidHostCallRequirement(format!(
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
            || requirement.host_call_contract_id.as_str().is_empty()
            || requirement.subject_kind.as_str().is_empty()
            || !capability.host_calls.iter().any(|host_call| {
                host_call.contract_id == requirement.host_call_contract_id
                    && host_call.target_kind.as_ref() == Some(&requirement.subject_kind)
            })
    }) || capability
        .authority_requirements
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidAuthorityContract(format!(
            "capability '{}' authority requirements must bind a declared targeted Host Call with non-empty identities and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    Ok(())
}

fn validate_authority_grants(grants: &[AuthorityGrant]) -> Result<(), PlannerError> {
    if grants.iter().any(|grant| {
        grant.grant_id.as_str().is_empty()
            || grant.contract_id.as_str().is_empty()
            || grant.host_call_contract_id.as_str().is_empty()
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
    for base in &host.bases {
        if base.lifecycle != conduit_core::BaseLifecycle::Ready
            || base.base_id.as_str().is_empty()
            || base.provider_instance_id.as_str().is_empty()
            || base.provider_generation == 0
            || base.implementation_id.as_str().is_empty()
            || base.mechanism_family.as_str().is_empty()
            || base.capability_ids.is_empty() && base.resource_pool_ids.is_empty()
            || base.capability_ids.iter().any(|id| {
                !host
                    .capabilities
                    .iter()
                    .any(|offer| offer.capability_id == *id)
            })
            || base
                .resource_pool_ids
                .iter()
                .any(|id| !host.resources.iter().any(|offer| offer.pool_id == *id))
        {
            return Err(PlannerError::InvalidResourceContract(format!(
                "host '{}' has invalid current Base provenance",
                host.host_id.as_str()
            )));
        }
    }
    Ok(())
}

fn selected_base_provider(
    host: &HostAdvertisement,
    capability: &conduit_core::CapabilityOffer,
    _resources: &[conduit_core::ResourceBinding],
) -> Result<Option<conduit_core::BaseProviderBinding>, PlannerError> {
    let mut owners = host
        .bases
        .iter()
        .filter(|base| base.capability_ids.contains(&capability.capability_id));
    let Some(owner) = owners.next() else {
        return Ok(None);
    };
    if owners.next().is_some() {
        return Err(PlannerError::InvalidResourceContract(format!(
            "capability '{}' has ambiguous Base provenance on host '{}'",
            capability.capability_id.as_str(),
            host.host_id.as_str()
        )));
    }
    Ok(Some(owner.binding()))
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
    policy: contract::LineMechanismPolicy<'a>,
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
        policy,
        requested,
        requested_candidates,
        line_offers,
        connection_item_capacity,
        connection_byte_capacity,
    } = selection;
    if source.host_id == sink.host_id {
        if requested.is_some_and(|base| {
            base != BaseImplementationId::from(conduit_core::LOCAL_BASE_IMPLEMENTATION_ID)
        }) || !policy.permits_local()
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

    if requested
        == Some(BaseImplementationId::from(
            conduit_core::LOCAL_BASE_IMPLEMENTATION_ID,
        ))
    {
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
            && policy.permits_remote(&offer.binding.base)
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
