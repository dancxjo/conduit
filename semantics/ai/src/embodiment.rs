//! Bounded evidence that typed wiring, not prose, defines a model's situation.

use crate::{EffectAuthority, ProposalDecisionOutcome, ProposalRefusal};
use alloc::{collections::BTreeSet, string::String, vec::Vec};
use conduit_core::{
    verify_plan, ActivePlayId, CheckedFormId, ExpandedFormId, KindId, PlacementId, Plan, PlanId,
    SignId,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_EMBODIMENT_PORTS: usize = 16;
pub const MAXIMUM_EMBODIMENT_SIGNS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbodimentStage {
    PerceptionOnly,
    Expressive,
    AuthorizedEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbodiedModelView {
    pub stage: EmbodimentStage,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub model_gear_identity: String,
    pub model_implementation_identity: String,
    pub wired_inputs: Vec<KindId>,
    pub wired_outputs: Vec<KindId>,
    pub expressive_output_wired: bool,
    pub protected_effect_wired: bool,
    pub authority_id: Option<String>,
    pub proposal_id: String,
    pub decision: ProposalDecisionOutcome,
    pub resulting_signs: Vec<SignId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbodiedModelReceipt {
    pub schema: &'static str,
    pub proof_class: &'static str,
    pub body_id: String,
    pub perception_value_kind: KindId,
    pub state_value_kind: KindId,
    pub expressive_value_kind: KindId,
    pub protected_effect_kind: KindId,
    pub views: Vec<EmbodiedModelView>,
    pub ambient_host_access: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbodimentReceiptError {
    InvalidPlan,
    ModelPlacementMissing,
    ModelPlacementAmbiguous,
    ConnectionPlacementMissing,
    ConnectionPortMismatch,
    EffectWiringAmbiguous,
    EffectAuthorityInvalid,
    InvalidIdentity,
    InvalidStageCount,
    InvalidStageOrder,
    InvalidBound,
    ProviderChanged,
    GearChanged,
    IdentityReused,
    MissingPerception,
    MissingState,
    UnexpectedExpression,
    MissingExpression,
    UnexpectedEffectWiring,
    MissingEffectWiring,
    InvalidDecision,
    InvalidEffectEvidence,
    AmbientHostAccess,
    AmbientPort,
}

impl EmbodiedModelView {
    /// Observes the model's situation exclusively from an immutable sealed Plan.
    #[allow(clippy::too_many_arguments)]
    pub fn from_plan(
        stage: EmbodimentStage,
        plan: &Plan,
        active_play_id: ActivePlayId,
        model_placement_id: &PlacementId,
        expressive_value_kind: &KindId,
        protected_effect_kind: &KindId,
        proposal_id: String,
        decision: ProposalDecisionOutcome,
        resulting_signs: Vec<SignId>,
    ) -> Result<Self, EmbodimentReceiptError> {
        if !verify_plan(plan) {
            return Err(EmbodimentReceiptError::InvalidPlan);
        }
        let mut models = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|placement| &placement.placement_id == model_placement_id);
        let model = models
            .next()
            .ok_or(EmbodimentReceiptError::ModelPlacementMissing)?;
        if models.next().is_some() {
            return Err(EmbodimentReceiptError::ModelPlacementAmbiguous);
        }

        let placements = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .collect::<Vec<_>>();
        let connections = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .collect::<Vec<_>>();
        let mut wired_inputs = connections
            .iter()
            .filter(|connection| &connection.sink_placement_id == model_placement_id)
            .map(|connection| connection.value_kind.clone())
            .collect::<Vec<_>>();
        let model_outputs = connections
            .iter()
            .filter(|connection| &connection.source_placement_id == model_placement_id)
            .collect::<Vec<_>>();
        if connections.iter().any(|connection| {
            (&connection.sink_placement_id == model_placement_id
                && !model.inputs.iter().any(|port| {
                    port.port_id == connection.sink_port_id
                        && port.value_kind == connection.value_kind
                }))
                || (&connection.source_placement_id == model_placement_id
                    && !model.outputs.iter().any(|port| {
                        port.port_id == connection.source_port_id
                            && port.value_kind == connection.value_kind
                    }))
        }) {
            return Err(EmbodimentReceiptError::ConnectionPortMismatch);
        }
        let mut wired_outputs = model_outputs
            .iter()
            .map(|connection| connection.value_kind.clone())
            .collect::<Vec<_>>();
        wired_inputs.sort();
        wired_inputs.dedup();
        wired_outputs.sort();
        wired_outputs.dedup();

        let expressive_output_wired = model_outputs
            .iter()
            .any(|connection| &connection.value_kind == expressive_value_kind);
        let mut effect_sinks = model_outputs
            .iter()
            .filter_map(|connection| {
                placements
                    .iter()
                    .find(|placement| placement.placement_id == connection.sink_placement_id)
                    .copied()
            })
            .filter(|placement| &placement.kind_id == protected_effect_kind);
        let effect = effect_sinks.next();
        if effect_sinks.next().is_some() {
            return Err(EmbodimentReceiptError::EffectWiringAmbiguous);
        }
        if model_outputs.iter().any(|connection| {
            !placements
                .iter()
                .any(|placement| placement.placement_id == connection.sink_placement_id)
        }) {
            return Err(EmbodimentReceiptError::ConnectionPlacementMissing);
        }
        let authority_id = effect
            .map(|_| {
                EffectAuthority::from_plan(plan, model_placement_id, protected_effect_kind)
                    .map(|authority| authority.authority_id)
                    .map_err(|_| EmbodimentReceiptError::EffectAuthorityInvalid)
            })
            .transpose()?;

        Ok(Self {
            stage,
            checked_form_id: plan.checked_form_id.clone(),
            expanded_form_id: plan.expanded_form_id.clone(),
            plan_id: plan.plan_id.clone(),
            active_play_id,
            model_gear_identity: model.gear_id.as_str().into(),
            model_implementation_identity: model.implementation_id.as_str().into(),
            wired_inputs,
            wired_outputs,
            expressive_output_wired,
            protected_effect_wired: effect.is_some(),
            authority_id,
            proposal_id,
            decision,
            resulting_signs,
        })
    }
}

impl EmbodiedModelReceipt {
    pub fn validate(&self) -> Result<(), EmbodimentReceiptError> {
        if self.schema != "conduit.llm/embodied-body-receipt@1"
            || self.proof_class.is_empty()
            || self.body_id.is_empty()
            || self.views.iter().any(|view| {
                view.model_gear_identity.is_empty()
                    || view.model_implementation_identity.is_empty()
                    || view.proposal_id.is_empty()
            })
        {
            return Err(EmbodimentReceiptError::InvalidIdentity);
        }
        if self.views.len() != 3 {
            return Err(EmbodimentReceiptError::InvalidStageCount);
        }
        if self.views.iter().map(|view| view.stage).ne([
            EmbodimentStage::PerceptionOnly,
            EmbodimentStage::Expressive,
            EmbodimentStage::AuthorizedEffect,
        ]) {
            return Err(EmbodimentReceiptError::InvalidStageOrder);
        }
        if self.ambient_host_access {
            return Err(EmbodimentReceiptError::AmbientHostAccess);
        }
        if self.views.iter().any(|view| {
            view.wired_inputs.len() > MAXIMUM_EMBODIMENT_PORTS
                || view.wired_outputs.len() > MAXIMUM_EMBODIMENT_PORTS
                || view.resulting_signs.len() > MAXIMUM_EMBODIMENT_SIGNS
        }) {
            return Err(EmbodimentReceiptError::InvalidBound);
        }
        let first = &self.views[0];
        if self
            .views
            .iter()
            .any(|view| view.model_implementation_identity != first.model_implementation_identity)
        {
            return Err(EmbodimentReceiptError::ProviderChanged);
        }
        if self
            .views
            .iter()
            .any(|view| view.model_gear_identity != first.model_gear_identity)
        {
            return Err(EmbodimentReceiptError::GearChanged);
        }
        unique_identities(&self.views)?;
        for view in &self.views {
            if !view.wired_inputs.contains(&self.perception_value_kind) {
                return Err(EmbodimentReceiptError::MissingPerception);
            }
            if !view.wired_inputs.contains(&self.state_value_kind) {
                return Err(EmbodimentReceiptError::MissingState);
            }
            if view
                .wired_inputs
                .iter()
                .chain(&view.wired_outputs)
                .any(|kind| ambient_kind(kind.as_str()))
            {
                return Err(EmbodimentReceiptError::AmbientPort);
            }
        }
        validate_perception_only(&self.views[0])?;
        validate_expressive(&self.views[1], &self.expressive_value_kind)?;
        validate_authorized(&self.views[2], &self.expressive_value_kind)?;
        Ok(())
    }
}

fn validate_perception_only(view: &EmbodiedModelView) -> Result<(), EmbodimentReceiptError> {
    if view.expressive_output_wired {
        return Err(EmbodimentReceiptError::UnexpectedExpression);
    }
    if view.protected_effect_wired || view.authority_id.is_some() {
        return Err(EmbodimentReceiptError::UnexpectedEffectWiring);
    }
    if !matches!(
        view.decision,
        ProposalDecisionOutcome::Refused(ProposalRefusal::UnwiredOperation)
    ) || !view.resulting_signs.is_empty()
    {
        return Err(EmbodimentReceiptError::InvalidDecision);
    }
    Ok(())
}

fn validate_expressive(
    view: &EmbodiedModelView,
    expressive_kind: &KindId,
) -> Result<(), EmbodimentReceiptError> {
    if !view.expressive_output_wired || !view.wired_outputs.contains(expressive_kind) {
        return Err(EmbodimentReceiptError::MissingExpression);
    }
    if view.protected_effect_wired || view.authority_id.is_some() {
        return Err(EmbodimentReceiptError::UnexpectedEffectWiring);
    }
    if !matches!(
        view.decision,
        ProposalDecisionOutcome::Refused(ProposalRefusal::UnwiredOperation)
    ) || !view.resulting_signs.is_empty()
    {
        return Err(EmbodimentReceiptError::InvalidDecision);
    }
    Ok(())
}

fn validate_authorized(
    view: &EmbodiedModelView,
    expressive_kind: &KindId,
) -> Result<(), EmbodimentReceiptError> {
    if !view.expressive_output_wired || !view.wired_outputs.contains(expressive_kind) {
        return Err(EmbodimentReceiptError::MissingExpression);
    }
    if !view.protected_effect_wired || view.authority_id.as_deref().is_none_or(str::is_empty) {
        return Err(EmbodimentReceiptError::MissingEffectWiring);
    }
    if !matches!(view.decision, ProposalDecisionOutcome::Authorized { .. }) {
        return Err(EmbodimentReceiptError::InvalidDecision);
    }
    if view.resulting_signs.is_empty() {
        return Err(EmbodimentReceiptError::InvalidEffectEvidence);
    }
    Ok(())
}

fn unique_identities(views: &[EmbodiedModelView]) -> Result<(), EmbodimentReceiptError> {
    let mut checked = BTreeSet::new();
    let mut expanded = BTreeSet::new();
    let mut plans = BTreeSet::new();
    let mut plays = BTreeSet::new();
    for view in views {
        if !checked.insert(view.checked_form_id.as_str())
            || !expanded.insert(view.expanded_form_id.as_str())
            || !plans.insert(view.plan_id.as_str())
            || !plays.insert(view.active_play_id.as_str())
        {
            return Err(EmbodimentReceiptError::IdentityReused);
        }
    }
    Ok(())
}

fn ambient_kind(kind: &str) -> bool {
    [
        "filesystem",
        "shell",
        "environment",
        "ambient-network",
        "tool",
    ]
    .iter()
    .any(|forbidden| kind.contains(forbidden))
}
