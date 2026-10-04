//! Finite ordered standalone-rule choice over one original intent occurrence.
use crate::{
    admission::{validate_feature_bundle, LocalSemanticRefusal},
    allophone_selection::{decision_projection, policy_projection, status_projection},
    declared_context::ExplicitAllophoneContext,
    generated,
    occurrence_context::{
        resolve_intent_occurrence_context, IntentOccurrenceContext, OccurrenceContextRefusal,
    },
    rule_evaluation::{evaluate_allophone_rule, AllophoneRuleEvaluation, RuleEvaluationRefusal},
    rule_input::{compare_phone_pattern, IdentityPatternComparison, IdentityPatternRefusal},
    semantic::*,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;
pub type GlobalPhoneConstraint<'a> =
    IdentityPatternComparison<'a, PhoneSpecification, SpeechPhonePatternIdentity>;
#[derive(Debug)]
pub enum GlobalRuleSelectionRefusal<'a> {
    Occurrence(OccurrenceContextRefusal),
    Basis(NativeBindingRefusal),
    FeatureOccurrence(NativeBindingRefusal),
    FeatureEvidence(LocalSemanticRefusal),
    UnresolvedPhone(&'a PhoneSpecification),
    Evaluation {
        index: usize,
        reason: RuleEvaluationRefusal,
    },
    PhoneConstraint {
        index: usize,
        reason: IdentityPatternRefusal,
    },
    CompiledPlot,
    State(NativeBindingRefusal),
}
pub struct GlobalRuleCandidate<'a> {
    rule: &'a SpeechAllophoneRule,
    status_allowed: bool,
    evaluation: Option<AllophoneRuleEvaluation<'a>>,
    phone_constraint: Option<GlobalPhoneConstraint<'a>>,
    decision: Option<SpeechContextDecision>,
}
impl<'a> GlobalRuleCandidate<'a> {
    pub fn rule(&self) -> &'a SpeechAllophoneRule {
        self.rule
    }
    pub fn status_allowed(&self) -> bool {
        self.status_allowed
    }
    pub fn evaluation(&self) -> Option<&AllophoneRuleEvaluation<'a>> {
        self.evaluation.as_ref()
    }
    pub fn phone_constraint(&self) -> Option<&GlobalPhoneConstraint<'a>> {
        self.phone_constraint.as_ref()
    }
    pub fn decision(&self) -> Option<&SpeechContextDecision> {
        self.decision.as_ref()
    }
}
pub struct GlobalRuleChoice<'a> {
    occurrence: IntentOccurrenceContext<'a>,
    profile: &'a SpeechAllophoneRuleProfile,
    policy: &'a SpeechAllophoneChoicePolicy,
    basis: SpeechIntentInventoryBasis,
    observed_features: Option<&'a SpeechOccurrenceFeatureObservation>,
    checked_features: Option<SpeechOccurrenceObservationMatch>,
    candidates: Vec<GlobalRuleCandidate<'a>>,
    state: SpeechAllophoneChoiceState,
}
impl<'a> GlobalRuleChoice<'a> {
    pub fn occurrence(&self) -> &IntentOccurrenceContext<'a> {
        &self.occurrence
    }
    pub fn profile(&self) -> &'a SpeechAllophoneRuleProfile {
        self.profile
    }
    pub fn policy(&self) -> &'a SpeechAllophoneChoicePolicy {
        self.policy
    }
    pub fn checked_basis(&self) -> &SpeechIntentInventoryBasis {
        &self.basis
    }
    pub fn observed_features(&self) -> Option<&'a SpeechOccurrenceFeatureObservation> {
        self.observed_features
    }
    pub fn checked_features(&self) -> Option<&SpeechOccurrenceObservationMatch> {
        self.checked_features.as_ref()
    }
    pub fn candidates(&self) -> &[GlobalRuleCandidate<'a>] {
        &self.candidates
    }
    pub fn state(&self) -> &SpeechAllophoneChoiceState {
        &self.state
    }
    pub fn selected_rule(&self) -> Option<&'a SpeechAllophoneRule> {
        match self.state.outcome() {
            SpeechAllophoneChoiceOutcome::SelectedAllophone => self
                .profile
                .rules()
                .as_slice()
                .get(*self.state.index() as usize),
            _ => None,
        }
    }
}
/// Native laws choose the first permitted match, preserving earlier unresolved
/// evidence as deferral. Every permitted rule is evaluated before returning a
/// receipt, so unsupported later obligations still refuse the profile. This
/// stage has no default candidate and does not realize or commit a phone.
pub fn select_global_allophone_rule<'a>(
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    profile: &'a SpeechAllophoneRuleProfile,
    policy: &'a SpeechAllophoneChoicePolicy,
    observed_features: Option<&'a SpeechOccurrenceFeatureObservation>,
    explicit: ExplicitAllophoneContext<'a>,
) -> Result<GlobalRuleChoice<'a>, GlobalRuleSelectionRefusal<'a>> {
    let occurrence = resolve_intent_occurrence_context(intent, event)
        .map_err(GlobalRuleSelectionRefusal::Occurrence)?;
    let requirement = occurrence.segment().phone();
    let request_state = match requirement {
        PhoneSpecification::Known(_) => generated::SpeechSpecificationState::known,
        PhoneSpecification::Unspecified => generated::SpeechSpecificationState::unspecified,
        PhoneSpecification::Unknown => generated::SpeechSpecificationState::unknown,
        PhoneSpecification::NotApplicable => generated::SpeechSpecificationState::not_applicable,
        PhoneSpecification::Variable(_) => generated::SpeechSpecificationState::variable,
        PhoneSpecification::Gradient(_) => generated::SpeechSpecificationState::gradient,
    };
    if !generated::speech_phone_choice_requirement_supported(request_state)
        .ok_or(GlobalRuleSelectionRefusal::CompiledPlot)?
    {
        return Err(GlobalRuleSelectionRefusal::UnresolvedPhone(requirement));
    }
    let basis = SpeechIntentInventoryBasis::new(
        intent.inventory_id().clone(),
        intent.language().clone(),
        profile.inventory_id().clone(),
        profile.language().clone(),
    )
    .map_err(GlobalRuleSelectionRefusal::Basis)?;
    let checked_features = observed_features
        .map(|value| {
            SpeechOccurrenceObservationMatch::new(
                occurrence.segment().occurrence().clone(),
                value.occurrence().clone(),
            )
        })
        .transpose()
        .map_err(GlobalRuleSelectionRefusal::FeatureOccurrence)?;
    if let Some(value) = observed_features {
        validate_feature_bundle(value.features())
            .map_err(GlobalRuleSelectionRefusal::FeatureEvidence)?;
    }
    let mut candidates = Vec::with_capacity(profile.rules().as_slice().len());
    let mut state = generated::SpeechAllophoneChoiceFold::none;
    for (index, rule) in profile.rules().as_slice().iter().enumerate() {
        let status_allowed =
            generated::speech_rule_status_choice(generated::SpeechRuleStatusChoiceInput {
                status: status_projection(rule.status()),
                policy: policy_projection(policy),
            })
            .ok_or(GlobalRuleSelectionRefusal::CompiledPlot)?;
        let (evaluation, phone_constraint, decision) = if status_allowed {
            let evaluation =
                evaluate_allophone_rule(rule, intent, event, observed_features, explicit)
                    .map_err(|reason| GlobalRuleSelectionRefusal::Evaluation { index, reason })?;
            let phone_constraint = compare_phone_pattern(requirement, rule.phone())
                .map_err(|reason| GlobalRuleSelectionRefusal::PhoneConstraint { index, reason })?;
            let decision =
                generated::speech_context_conjunction(generated::SpeechContextConjunction {
                    left: decision_projection(evaluation.decision()),
                    right: decision_projection(phone_constraint.decision()),
                })
                .ok_or(GlobalRuleSelectionRefusal::CompiledPlot)?;
            let retained = match decision {
                generated::SpeechContextDecision::matched => SpeechContextDecision::Matched,
                generated::SpeechContextDecision::mismatched => SpeechContextDecision::Mismatched,
                generated::SpeechContextDecision::requirement_unresolved => {
                    SpeechContextDecision::RequirementUnresolved
                }
                generated::SpeechContextDecision::observation_unresolved => {
                    SpeechContextDecision::ObservationUnresolved
                }
            };
            (Some(evaluation), Some(phone_constraint), Some(retained))
        } else {
            (None, None, None)
        };
        state = generated::speech_allophone_choice_step(generated::SpeechAllophoneChoiceInput {
            state,
            index: index as u32,
            admitted: status_allowed,
            decision: decision
                .as_ref()
                .map(decision_projection)
                .unwrap_or(generated::SpeechContextDecision::mismatched),
        })
        .ok_or(GlobalRuleSelectionRefusal::CompiledPlot)?;
        candidates.push(GlobalRuleCandidate {
            rule,
            status_allowed,
            evaluation,
            phone_constraint,
            decision,
        });
    }
    use generated::SpeechAllophoneChoiceFold as F;
    let (index, outcome, reason) = match state {
        F::none => (
            0,
            SpeechAllophoneChoiceOutcome::None,
            SpeechContextDecision::Mismatched,
        ),
        F::selected_allophone(value) => (
            value.index,
            SpeechAllophoneChoiceOutcome::SelectedAllophone,
            SpeechContextDecision::Matched,
        ),
        F::requirement_deferred(value) => (
            value.index,
            SpeechAllophoneChoiceOutcome::Deferred,
            SpeechContextDecision::RequirementUnresolved,
        ),
        F::observation_deferred(value) => (
            value.index,
            SpeechAllophoneChoiceOutcome::Deferred,
            SpeechContextDecision::ObservationUnresolved,
        ),
        F::selected_default => return Err(GlobalRuleSelectionRefusal::CompiledPlot),
    };
    let state = SpeechAllophoneChoiceState::new(index, outcome, reason)
        .map_err(GlobalRuleSelectionRefusal::State)?;
    Ok(GlobalRuleChoice {
        occurrence,
        profile,
        policy,
        basis,
        observed_features,
        checked_features,
        candidates,
        state,
    })
}
