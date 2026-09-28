//! Exact admission of one replacement after selected-realization loss.
//!
//! This module does not observe availability, choose a candidate, dispatch a
//! Play, or retry work. It binds an ordinary replanning result to the exact
//! invalidated realization and refuses to continue unless moving the semantic
//! work to the newly selected Back is permitted by the reviewed Kind law.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::{
    derive_planned_transformation_eligibility, verify_plan, ActivePlayIdentity, BootId, GearId,
    HostId, ImplementationId, LineId, OfferGeneration, PlacementId, Plan, PlanId, SignIdentity,
    TransformationRefusal, WorkTransformation,
};

use crate::RealizationReplanOutcome;

pub const MAXIMUM_INVALIDATED_REALIZATION_LINES: usize = 32;
pub const MAXIMUM_RECOVERY_REFUSAL_BYTES: usize = 512;

/// Exact Plan-selected truth withdrawn by one observed loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationInvalidation {
    pub placement_id: PlacementId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: ImplementationId,
    /// Every selected Line incident to the invalidated placement, sorted by ID.
    pub selected_line_ids: Vec<LineId>,
    pub supporting_sign: SignIdentity,
}

/// The ordinary planner either produced a checked result or found no admitted
/// alternative. Invalid input and policy errors must not be converted into the
/// latter terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryPlanningOutcome {
    Planned(RealizationReplanOutcome),
    NoAdmittedAlternative { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationReplacementEvidence {
    pub previous_plan_id: PlanId,
    pub previous_play: ActivePlayIdentity,
    pub invalidation: RealizationInvalidation,
    pub replacement_plan_id: Option<PlanId>,
    pub replacement_play: Option<ActivePlayIdentity>,
    pub replacement_placement_id: Option<PlacementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationRecoveryOutcome {
    Replacement {
        plan: Plan,
        evidence: RealizationReplacementEvidence,
    },
    NoAdmittedAlternative {
        reason: String,
        evidence: RealizationReplacementEvidence,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationRecoveryRefusal {
    InvalidPreviousPlay,
    InvalidationDoesNotMatchPlan,
    InvalidInvalidationSign,
    InvalidLineEvidence,
    Transformation(TransformationRefusal),
    UnchangedInvalidatedPlan,
    ReplacementDoesNotMatchPreviousPlan,
    ReplacementNotSemanticallySubstitutable,
    InvalidReplacementPlay,
    InvalidNoAlternativeReason,
    StaleCompletion,
}

/// Admit one already-planned replacement or one explicit no-alternative
/// terminal. No work is scheduled by this function.
#[allow(clippy::too_many_arguments)]
pub fn admit_realization_recovery(
    previous: &Plan,
    previous_play: &ActivePlayIdentity,
    invalidation: RealizationInvalidation,
    planning: RecoveryPlanningOutcome,
    replacement_play: Option<ActivePlayIdentity>,
) -> Result<RealizationRecoveryOutcome, RealizationRecoveryRefusal> {
    let old = previous
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.placement_id == invalidation.placement_id)
        .ok_or(RealizationRecoveryRefusal::InvalidationDoesNotMatchPlan)?;
    if previous_play.plan_id != previous.plan_id
        || previous_play.host_id != old.host_id
        || previous_play.boot_id != old.boot_id
    {
        return Err(RealizationRecoveryRefusal::InvalidPreviousPlay);
    }
    if old.host_id != invalidation.host_id
        || old.boot_id != invalidation.boot_id
        || old.offer_generation != invalidation.offer_generation
        || old.implementation_id != invalidation.implementation_id
    {
        return Err(RealizationRecoveryRefusal::InvalidationDoesNotMatchPlan);
    }
    if invalidation.supporting_sign.host_id != previous_play.host_id
        || invalidation.supporting_sign.boot_id != previous_play.boot_id
        || invalidation.supporting_sign.active_play_id.as_ref()
            != Some(&previous_play.active_play_id)
    {
        return Err(RealizationRecoveryRefusal::InvalidInvalidationSign);
    }
    validate_line_evidence(previous, &invalidation)?;

    match planning {
        RecoveryPlanningOutcome::Planned(RealizationReplanOutcome::Unchanged { .. }) => {
            Err(RealizationRecoveryRefusal::UnchangedInvalidatedPlan)
        }
        RecoveryPlanningOutcome::Planned(RealizationReplanOutcome::Replacement {
            previous_plan_id,
            plan,
        }) => {
            if !verify_plan(&plan)
                || previous_plan_id != previous.plan_id
                || plan.source_document_id != previous.source_document_id
                || plan.checked_form_id != previous.checked_form_id
                || plan.expanded_form_id != previous.expanded_form_id
                || plan.plan_id == previous.plan_id
            {
                return Err(RealizationRecoveryRefusal::ReplacementDoesNotMatchPreviousPlan);
            }
            let replacement = replacement_for_gear(&plan, &old.gear_id)?;
            if replacement.kind_id != old.kind_id
                || replacement.kind_contract_revision != old.kind_contract_revision
            {
                return Err(RealizationRecoveryRefusal::ReplacementNotSemanticallySubstitutable);
            }
            derive_planned_transformation_eligibility(replacement)
                .map_err(RealizationRecoveryRefusal::Transformation)?
                .require(WorkTransformation::Move)
                .map_err(RealizationRecoveryRefusal::Transformation)?;
            let replacement_play =
                replacement_play.ok_or(RealizationRecoveryRefusal::InvalidReplacementPlay)?;
            if replacement_play.plan_id != plan.plan_id
                || replacement_play.host_id != replacement.host_id
                || replacement_play.boot_id != replacement.boot_id
                || replacement_play.active_play_id == previous_play.active_play_id
            {
                return Err(RealizationRecoveryRefusal::InvalidReplacementPlay);
            }
            let evidence = RealizationReplacementEvidence {
                previous_plan_id: previous.plan_id.clone(),
                previous_play: previous_play.clone(),
                invalidation,
                replacement_plan_id: Some(plan.plan_id.clone()),
                replacement_play: Some(replacement_play),
                replacement_placement_id: Some(replacement.placement_id.clone()),
            };
            Ok(RealizationRecoveryOutcome::Replacement { plan, evidence })
        }
        RecoveryPlanningOutcome::NoAdmittedAlternative { reason } => {
            if replacement_play.is_some()
                || reason.is_empty()
                || reason.len() > MAXIMUM_RECOVERY_REFUSAL_BYTES
            {
                return Err(RealizationRecoveryRefusal::InvalidNoAlternativeReason);
            }
            Ok(RealizationRecoveryOutcome::NoAdmittedAlternative {
                reason,
                evidence: RealizationReplacementEvidence {
                    previous_plan_id: previous.plan_id.clone(),
                    previous_play: previous_play.clone(),
                    invalidation,
                    replacement_plan_id: None,
                    replacement_play: None,
                    replacement_placement_id: None,
                },
            })
        }
    }
}

impl RealizationReplacementEvidence {
    /// Accept completion only from the exact replacement Play. The retired
    /// Play remains evidence, never current execution truth.
    pub fn accept_completion(
        &self,
        completion: &ActivePlayIdentity,
    ) -> Result<(), RealizationRecoveryRefusal> {
        match &self.replacement_play {
            Some(current) if current == completion => Ok(()),
            _ => Err(RealizationRecoveryRefusal::StaleCompletion),
        }
    }
}

fn replacement_for_gear<'a>(
    plan: &'a Plan,
    gear_id: &GearId,
) -> Result<&'a conduit_core::PlannedGear, RealizationRecoveryRefusal> {
    let mut matches = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .filter(|placement| &placement.gear_id == gear_id);
    let replacement = matches
        .next()
        .ok_or(RealizationRecoveryRefusal::ReplacementNotSemanticallySubstitutable)?;
    if matches.next().is_some() {
        return Err(RealizationRecoveryRefusal::ReplacementNotSemanticallySubstitutable);
    }
    Ok(replacement)
}

fn validate_line_evidence(
    previous: &Plan,
    invalidation: &RealizationInvalidation,
) -> Result<(), RealizationRecoveryRefusal> {
    if invalidation.selected_line_ids.len() > MAXIMUM_INVALIDATED_REALIZATION_LINES
        || invalidation
            .selected_line_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(RealizationRecoveryRefusal::InvalidLineEvidence);
    }
    let expected = previous
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter(|connection| {
            connection.source_placement_id == invalidation.placement_id
                || connection.sink_placement_id == invalidation.placement_id
        })
        .filter_map(|connection| connection.selected_line.as_ref())
        .map(|line| line.line_id.clone())
        .collect::<BTreeSet<_>>();
    if expected.len() > MAXIMUM_INVALIDATED_REALIZATION_LINES
        || expected.into_iter().collect::<Vec<_>>() != invalidation.selected_line_ids
    {
        return Err(RealizationRecoveryRefusal::InvalidLineEvidence);
    }
    Ok(())
}
