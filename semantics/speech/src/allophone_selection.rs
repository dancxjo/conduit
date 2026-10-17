//! Explicit, preparation-time choice among at most eight original declarations.
//! Native Plots own status eligibility, priority, deferral and default fallback.
use crate::{
    declared_context::{
        compare_allophone_requirements, AllophoneRequirements, DeclaredContextRefusal,
        ExplicitAllophoneContext,
    },
    generated,
    occurrence_context::{
        resolve_intent_occurrence_context, IntentOccurrenceContext, OccurrenceContextRefusal,
    },
    semantic::*,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;
pub type PhoneConstraintReceipt = Result<SpeechPhoneDefinitionMatch, NativeBindingRefusal>;
#[derive(Debug)]
pub enum AllophoneSelectionRefusal<'a> {
    Occurrence(OccurrenceContextRefusal),
    Basis(NativeBindingRefusal),
    UnresolvedPhoneme(&'a PhonemeSpecification),
    UnresolvedPhone(&'a PhoneSpecification),
    MissingPhoneme,
    AmbiguousPhoneme,
    Phoneme(NativeBindingRefusal),
    PhoneConstraint(NativeBindingRefusal),
    Context {
        index: usize,
        reason: DeclaredContextRefusal,
    },
    CompiledPlot,
    State(NativeBindingRefusal),
}
pub struct AllophoneCandidate<'a> {
    declaration: &'a SpeechPhonemeAllophone,
    status_allowed: bool,
    phone_constraint: Option<PhoneConstraintReceipt>,
    eligible: bool,
    context: Option<AllophoneRequirements<'a>>,
}
impl<'a> AllophoneCandidate<'a> {
    pub fn declaration(&self) -> &'a SpeechPhonemeAllophone {
        self.declaration
    }
    pub fn status_allowed(&self) -> bool {
        self.status_allowed
    }
    pub fn phone_constraint(
        &self,
    ) -> Option<&Result<SpeechPhoneDefinitionMatch, NativeBindingRefusal>> {
        self.phone_constraint.as_ref()
    }
    pub fn eligible(&self) -> bool {
        self.eligible
    }
    pub fn context_decision(&self) -> Option<&SpeechContextDecision> {
        self.context.as_ref().map(|value| &value.decision)
    }
    pub fn conditions(&self) -> Option<&crate::rule_conditions::ConditionsComparison<'a>> {
        self.context.as_ref().map(|value| &value.conditions)
    }
    pub fn scalar(&self) -> Option<&crate::allophone_context::AllophoneScalarContext<'a>> {
        self.context.as_ref().map(|value| &value.scalar)
    }
    pub fn neighbors(&self) -> Option<&crate::allophone_context::AllophoneNeighborContext<'a>> {
        self.context.as_ref().map(|value| &value.neighbors)
    }
}
pub struct IntentAllophoneChoice<'a> {
    occurrence: IntentOccurrenceContext<'a>,
    inventory: &'a SpeechInventory,
    phoneme: &'a SpeechPhoneme,
    basis: SpeechIntentInventoryBasis,
    identity: SpeechPhonemeDefinitionMatch,
    policy: &'a SpeechAllophoneChoicePolicy,
    candidates: Vec<AllophoneCandidate<'a>>,
    default_constraint: Option<PhoneConstraintReceipt>,
    state: SpeechAllophoneChoiceState,
}
impl<'a> IntentAllophoneChoice<'a> {
    pub fn occurrence(&self) -> &IntentOccurrenceContext<'a> {
        &self.occurrence
    }
    pub fn inventory(&self) -> &'a SpeechInventory {
        self.inventory
    }
    pub fn phoneme(&self) -> &'a SpeechPhoneme {
        self.phoneme
    }
    pub fn basis(&self) -> &SpeechIntentInventoryBasis {
        &self.basis
    }
    pub fn checked_phoneme(&self) -> &SpeechPhonemeDefinitionMatch {
        &self.identity
    }
    pub fn policy(&self) -> &'a SpeechAllophoneChoicePolicy {
        self.policy
    }
    pub fn candidates(&self) -> impl Iterator<Item = &AllophoneCandidate<'a>> {
        self.candidates.iter()
    }
    pub fn default_constraint(
        &self,
    ) -> Option<&Result<SpeechPhoneDefinitionMatch, NativeBindingRefusal>> {
        self.default_constraint.as_ref()
    }
    pub fn state(&self) -> &SpeechAllophoneChoiceState {
        &self.state
    }
    pub fn selected_phone(&self) -> Option<&'a PhoneId> {
        match self.state.outcome() {
            SpeechAllophoneChoiceOutcome::SelectedAllophone => self
                .phoneme
                .allophones()
                .as_slice()
                .get(*self.state.index() as usize)
                .map(SpeechPhonemeAllophone::phone),
            SpeechAllophoneChoiceOutcome::SelectedDefault => self.phoneme.default_phone().as_ref(),
            SpeechAllophoneChoiceOutcome::None | SpeechAllophoneChoiceOutcome::Deferred => None,
        }
    }
}
pub(crate) fn policy_projection(
    value: &SpeechAllophoneChoicePolicy,
) -> generated::SpeechAllophoneChoicePolicy {
    generated::SpeechAllophoneChoicePolicy {
        productive: *value.productive(),
        lexicalized: *value.lexicalized(),
        optional: *value.optional(),
        style_dependent: *value.style_dependent(),
        experimental: *value.experimental(),
        allow_default: *value.allow_default(),
    }
}
pub(crate) fn status_projection(value: &SpeechRuleStatus) -> generated::SpeechRuleStatus {
    match value {
        SpeechRuleStatus::Productive => generated::SpeechRuleStatus::productive,
        SpeechRuleStatus::Lexicalized => generated::SpeechRuleStatus::lexicalized,
        SpeechRuleStatus::Optional => generated::SpeechRuleStatus::optional,
        SpeechRuleStatus::StyleDependent => generated::SpeechRuleStatus::style_dependent,
        SpeechRuleStatus::Experimental => generated::SpeechRuleStatus::experimental,
    }
}
pub(crate) fn decision_projection(
    value: &SpeechContextDecision,
) -> generated::SpeechContextDecision {
    match value {
        SpeechContextDecision::Matched => generated::SpeechContextDecision::matched,
        SpeechContextDecision::Mismatched => generated::SpeechContextDecision::mismatched,
        SpeechContextDecision::RequirementUnresolved => {
            generated::SpeechContextDecision::requirement_unresolved
        }
        SpeechContextDecision::ObservationUnresolved => {
            generated::SpeechContextDecision::observation_unresolved
        }
    }
}
fn phone_constraint<'a>(
    requirement: &'a PhoneSpecification,
    phone: &PhoneId,
) -> Result<(bool, Option<PhoneConstraintReceipt>), AllophoneSelectionRefusal<'a>> {
    let PhoneSpecification::Known(required) = requirement else {
        return Ok((true, None));
    };
    match SpeechPhoneDefinitionMatch::new(required.clone(), phone.clone()) {
        Ok(receipt) => Ok((true, Some(Ok(receipt)))),
        Err(reason @ NativeBindingRefusal::ViolatedInvariant { index: 0 }) => {
            Ok((false, Some(Err(reason))))
        }
        Err(reason) => Err(AllophoneSelectionRefusal::PhoneConstraint(reason)),
    }
}
pub fn select_intent_allophone<'a>(
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    inventory: &'a SpeechInventory,
    policy: &'a SpeechAllophoneChoicePolicy,
    explicit: ExplicitAllophoneContext<'a>,
) -> Result<IntentAllophoneChoice<'a>, AllophoneSelectionRefusal<'a>> {
    let occurrence = resolve_intent_occurrence_context(intent, event)
        .map_err(AllophoneSelectionRefusal::Occurrence)?;
    let requirement = occurrence.segment().phone();
    use generated::SpeechSpecificationState as S;
    let state = match requirement {
        PhoneSpecification::Known(_) => S::known,
        PhoneSpecification::Unspecified => S::unspecified,
        PhoneSpecification::Unknown => S::unknown,
        PhoneSpecification::NotApplicable => S::not_applicable,
        PhoneSpecification::Variable(_) => S::variable,
        PhoneSpecification::Gradient(_) => S::gradient,
    };
    if !generated::speech_phone_choice_requirement_supported(state)
        .ok_or(AllophoneSelectionRefusal::CompiledPlot)?
    {
        return Err(AllophoneSelectionRefusal::UnresolvedPhone(requirement));
    }
    let basis = SpeechIntentInventoryBasis::new(
        intent.inventory_id().clone(),
        intent.language().clone(),
        inventory.identity().clone(),
        inventory.language().clone(),
    )
    .map_err(AllophoneSelectionRefusal::Basis)?;
    let requested = match occurrence.segment().phoneme() {
        PhonemeSpecification::Known(value) => value,
        other => return Err(AllophoneSelectionRefusal::UnresolvedPhoneme(other)),
    };
    let mut matches = inventory
        .phonemes()
        .as_slice()
        .iter()
        .filter(|value| value.identity() == requested);
    let phoneme = matches
        .next()
        .ok_or(AllophoneSelectionRefusal::MissingPhoneme)?;
    if matches.next().is_some() {
        return Err(AllophoneSelectionRefusal::AmbiguousPhoneme);
    }
    let identity = SpeechPhonemeDefinitionMatch::new(phoneme.identity().clone(), requested.clone())
        .map_err(AllophoneSelectionRefusal::Phoneme)?;
    // Native collection bound is eight; reserve its exact size during preparation.
    let mut candidates = Vec::with_capacity(phoneme.allophones().as_slice().len());
    let mut state = generated::SpeechAllophoneChoiceFold::none;
    for (index, declaration) in phoneme.allophones().as_slice().iter().enumerate() {
        let status_allowed =
            generated::speech_rule_status_choice(generated::SpeechRuleStatusChoiceInput {
                status: status_projection(declaration.status()),
                policy: policy_projection(policy),
            })
            .ok_or(AllophoneSelectionRefusal::CompiledPlot)?;
        let (phone_compatible, phone_constraint) =
            phone_constraint(requirement, declaration.phone())?;
        let eligible = generated::speech_allophone_candidate_eligible(
            generated::SpeechAllophoneCandidateEligibility {
                status_allowed,
                phone_compatible,
            },
        )
        .ok_or(AllophoneSelectionRefusal::CompiledPlot)?;
        let context = if eligible {
            Some(
                compare_allophone_requirements(declaration, &occurrence, explicit)
                    .map_err(|reason| AllophoneSelectionRefusal::Context { index, reason })?,
            )
        } else {
            None
        };
        // This filler is ignored by the native step for excluded candidates.
        let decision = context
            .as_ref()
            .map(|value| decision_projection(&value.decision))
            .unwrap_or(generated::SpeechContextDecision::mismatched);
        state = generated::speech_allophone_choice_step(generated::SpeechAllophoneChoiceInput {
            state,
            index: index as u32,
            admitted: eligible,
            decision,
        })
        .ok_or(AllophoneSelectionRefusal::CompiledPlot)?;
        candidates.push(AllophoneCandidate {
            declaration,
            status_allowed,
            phone_constraint,
            eligible,
            context,
        });
    }
    let (default_available, default_constraint) = match phoneme.default_phone() {
        Some(phone) => phone_constraint(requirement, phone)?,
        None => (false, None),
    };
    state =
        generated::speech_allophone_choice_finish(generated::SpeechAllophoneChoiceFinishInput {
            state,
            allow_default: *policy.allow_default(),
            default_available,
        })
        .ok_or(AllophoneSelectionRefusal::CompiledPlot)?;
    use generated::SpeechAllophoneChoiceFold as Fold;
    let (index, outcome, reason) = match state {
        Fold::none => (
            0,
            SpeechAllophoneChoiceOutcome::None,
            SpeechContextDecision::Mismatched,
        ),
        Fold::selected_default => (
            0,
            SpeechAllophoneChoiceOutcome::SelectedDefault,
            SpeechContextDecision::Matched,
        ),
        Fold::selected_allophone(candidate) => (
            candidate.index,
            SpeechAllophoneChoiceOutcome::SelectedAllophone,
            SpeechContextDecision::Matched,
        ),
        Fold::requirement_deferred(candidate) => (
            candidate.index,
            SpeechAllophoneChoiceOutcome::Deferred,
            SpeechContextDecision::RequirementUnresolved,
        ),
        Fold::observation_deferred(candidate) => (
            candidate.index,
            SpeechAllophoneChoiceOutcome::Deferred,
            SpeechContextDecision::ObservationUnresolved,
        ),
    };
    let state = SpeechAllophoneChoiceState::new(index, outcome, reason)
        .map_err(AllophoneSelectionRefusal::State)?;
    Ok(IntentAllophoneChoice {
        occurrence,
        inventory,
        phoneme,
        basis,
        identity,
        policy,
        candidates,
        default_constraint,
        state,
    })
}
