//! Human-first projection of canonical Body membership and admission candidates.

use conduit_body::{
    Body, BodyMembership, CandidateId, CandidateInventory, CandidateState, MembershipEventKind,
    MembershipState, PartId,
};
use conduit_core::{ActivePlayIdentity, BootId, HostId, OfferGeneration, PlacementId, Plan};
use serde::{Deserialize, Serialize};

pub const MAX_PARTS_VIEW_ROWS: usize = conduit_body::MAX_BODY_PARTS;
pub const MAX_WANTS_TO_JOIN_ROWS: usize = conduit_body::MAX_CANDIDATES;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartPresentationState {
    Here,
    Attached,
    Offline,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartsAction {
    Inspect,
    Admit,
    Refuse,
    Revoke,
    SpawnBrowserPart,
    Replan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartDetails {
    pub part_id: PartId,
    pub host_id: Option<HostId>,
    pub boot_id: Option<BootId>,
    pub offer_generation: Option<OfferGeneration>,
    pub proof_reference: Option<String>,
    pub planned_placements: Vec<PlacementId>,
    pub planned_authority_bindings: usize,
    pub expected_signs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartRow {
    pub label: String,
    pub state: PartPresentationState,
    pub available: bool,
    pub in_plan: bool,
    pub playing: bool,
    pub details: PartDetails,
    pub actions: Vec<PartsAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateRow {
    pub candidate_id: CandidateId,
    pub label: String,
    pub state: CandidateState,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capabilities: usize,
    pub actions: Vec<PartsAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartsView {
    pub body_id: conduit_body::BodyId,
    pub awake: bool,
    pub parts: Vec<PartRow>,
    pub wants_to_join: Vec<CandidateRow>,
    pub actions: Vec<PartsAction>,
    pub new_realization_possibilities: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PartsViewError {
    WrongBody,
    UnknownHerePart,
    InvalidPlan,
    InvalidPlay,
    CapacityExceeded,
}

impl PartsView {
    #[allow(clippy::too_many_arguments)]
    pub fn project(
        body: &Body,
        membership: &BodyMembership,
        candidates: &CandidateInventory,
        here: &PartId,
        plan: Option<&Plan>,
        play: Option<&ActivePlayIdentity>,
        awake: bool,
    ) -> Result<Self, PartsViewError> {
        if membership.body_id != body.body_id || candidates.body_id != body.body_id {
            return Err(PartsViewError::WrongBody);
        }
        membership
            .parts
            .iter()
            .any(|part| &part.part_id == here && part.state == MembershipState::Admitted)
            .then_some(())
            .ok_or(PartsViewError::UnknownHerePart)?;
        if membership.parts.len() > MAX_PARTS_VIEW_ROWS
            || candidates.candidates.len() > MAX_WANTS_TO_JOIN_ROWS
        {
            return Err(PartsViewError::CapacityExceeded);
        }
        if let Some(plan) = plan {
            conduit_core::verify_plan(plan)
                .then_some(())
                .ok_or(PartsViewError::InvalidPlan)?;
        }
        if let Some(play) = play {
            let plan = plan.ok_or(PartsViewError::InvalidPlay)?;
            if play.plan_id != plan.plan_id {
                return Err(PartsViewError::InvalidPlay);
            }
        }

        let mut rows = Vec::with_capacity(MAX_PARTS_VIEW_ROWS);
        for part in membership
            .parts
            .iter()
            .filter(|part| part.state == MembershipState::Admitted)
        {
            let planned_fragment = part.current.as_ref().and_then(|current| {
                plan.and_then(|plan| {
                    plan.fragments.iter().find(|fragment| {
                        fragment.host_id == current.host_id
                            && fragment.boot_id == current.boot_id
                            && fragment.offer_generation == current.offer_generation
                    })
                })
            });
            let in_plan = planned_fragment.is_some();
            let playing = in_plan && play.is_some();
            let state = if &part.part_id == here {
                PartPresentationState::Here
            } else if part.current.is_some() {
                PartPresentationState::Attached
            } else {
                PartPresentationState::Offline
            };
            let label = if state == PartPresentationState::Here {
                "This computer".into()
            } else {
                part.current
                    .as_ref()
                    .map(|current| friendly_host_label(&current.host_id))
                    .unwrap_or_else(|| "Offline Part".into())
            };
            rows.push(PartRow {
                label,
                state,
                available: part.current.is_some(),
                in_plan,
                playing,
                details: PartDetails {
                    part_id: part.part_id.clone(),
                    host_id: part.current.as_ref().map(|current| current.host_id.clone()),
                    boot_id: part.current.as_ref().map(|current| current.boot_id.clone()),
                    offer_generation: part
                        .current
                        .as_ref()
                        .map(|current| current.offer_generation),
                    proof_reference: part.current.as_ref().map_or_else(
                        || admission_proof(membership, &part.part_id),
                        |current| Some(current.proof_id.as_str().into()),
                    ),
                    planned_placements: planned_fragment
                        .map(|fragment| {
                            fragment
                                .placements
                                .iter()
                                .map(|placement| placement.placement_id.clone())
                                .collect()
                        })
                        .unwrap_or_default(),
                    planned_authority_bindings: planned_fragment
                        .map(|fragment| {
                            fragment
                                .placements
                                .iter()
                                .map(|placement| placement.authority.len())
                                .sum()
                        })
                        .unwrap_or(0),
                    expected_signs: planned_fragment
                        .map(|fragment| fragment.expected_sign.len())
                        .unwrap_or(0),
                },
                actions: vec![PartsAction::Inspect, PartsAction::Revoke],
            });
        }

        let wants_to_join = candidates
            .candidates
            .iter()
            .filter(|candidate| {
                matches!(
                    candidate.state,
                    CandidateState::Discovered | CandidateState::RequestingAdmission
                )
            })
            .map(|candidate| CandidateRow {
                candidate_id: candidate.candidate_id.clone(),
                label: candidate.observation.friendly_label.clone(),
                state: candidate.state,
                host_id: candidate.observation.advertisement.host_id.clone(),
                boot_id: candidate.observation.advertisement.boot_id.clone(),
                offer_generation: candidate.observation.advertisement.offer_generation,
                capabilities: candidate.observation.advertisement.capabilities.len(),
                actions: if candidate.state == CandidateState::Discovered {
                    vec![
                        PartsAction::Inspect,
                        PartsAction::Admit,
                        PartsAction::Refuse,
                    ]
                } else {
                    vec![PartsAction::Inspect]
                },
            })
            .collect::<Vec<_>>();
        let new_realization_possibilities = plan.is_some()
            && (rows.iter().any(|row| row.available && !row.in_plan) || !wants_to_join.is_empty());
        let mut actions = vec![PartsAction::SpawnBrowserPart];
        if new_realization_possibilities {
            actions.push(PartsAction::Replan);
        }
        Ok(Self {
            body_id: body.body_id.clone(),
            awake,
            parts: rows,
            wants_to_join,
            actions,
            new_realization_possibilities,
        })
    }
}

fn admission_proof(membership: &BodyMembership, part_id: &PartId) -> Option<String> {
    membership.events.iter().rev().find_map(|event| {
        (&event.part_id == part_id)
            .then_some(&event.kind)
            .and_then(|kind| match kind {
                MembershipEventKind::Admitted { proof_id } => Some(proof_id.as_str().into()),
                _ => None,
            })
    })
}

fn friendly_host_label(host_id: &HostId) -> String {
    if host_id.as_str().contains("browser") {
        "Browser".into()
    } else if host_id.as_str().contains("pico") {
        "Pico W".into()
    } else {
        "Attached Part".into()
    }
}
