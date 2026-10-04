//! Explicit phoneme-default completion of native standalone-rule selection.
use crate::{
    generated,
    global_rule_selection::GlobalRuleChoice,
    intent_phoneme_inventory::{
        resolve_intent_inventory_phoneme, IntentPhonemeRefusal, ResolvedIntentPhoneme,
    },
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum GlobalDefaultRefusal<'a> {
    Phoneme(IntentPhonemeRefusal<'a>),
    CompiledPlot,
    State(NativeBindingRefusal),
}
pub struct GlobalDefaultChoice<'choice, 'source> {
    choice: &'choice GlobalRuleChoice<'source>,
    phoneme: ResolvedIntentPhoneme<'source>,
    default_identity: Option<SpeechPhonePatternIdentity>,
    state: SpeechAllophoneChoiceState,
}
impl<'choice, 'source> GlobalDefaultChoice<'choice, 'source> {
    pub fn choice(&self) -> &'choice GlobalRuleChoice<'source> {
        self.choice
    }
    pub fn phoneme(&self) -> &ResolvedIntentPhoneme<'source> {
        &self.phoneme
    }
    pub fn state(&self) -> &SpeechAllophoneChoiceState {
        &self.state
    }
    pub fn checked_default_identity(&self) -> Option<&SpeechPhonePatternIdentity> {
        self.default_identity.as_ref()
    }
    pub fn selected_default(&self) -> Option<&'source PhoneId> {
        if matches!(
            self.state.outcome(),
            SpeechAllophoneChoiceOutcome::SelectedDefault
        ) {
            self.phoneme.definition().default_phone().as_ref()
        } else {
            None
        }
    }
}
/// A default comes only from the exact original phoneme definition. It remains
/// a phone-ID declaration, not phone membership, acoustic admission or commitment.
/// The native finish law preserves both winners and earlier deferral.
pub fn finish_global_default_choice<'choice, 'source>(
    choice: &'choice GlobalRuleChoice<'source>,
    inventory: &'source SpeechInventory,
) -> Result<GlobalDefaultChoice<'choice, 'source>, GlobalDefaultRefusal<'source>> {
    let phoneme = resolve_intent_inventory_phoneme(
        choice.occurrence().intent(),
        choice.occurrence().event(),
        inventory,
    )
    .map_err(GlobalDefaultRefusal::Phoneme)?;
    let default = phoneme.definition().default_phone().as_ref();
    let requirement = choice.occurrence().segment().phone();
    let default_identity = match (requirement, default) {
        (PhoneSpecification::Known(requested), Some(default)) => {
            SpeechPhonePatternIdentity::new(default.clone(), requested.clone()).ok()
        }
        _ => None,
    };
    let compatible = generated::speech_identity_pattern_compare(
        generated::SpeechIdentityPatternComparisonInput {
            requirement: if matches!(requirement, PhoneSpecification::Known(_)) {
                generated::SpeechSpecificationState::known
            } else {
                generated::SpeechSpecificationState::unspecified
            },
            observation: if default.is_some() {
                generated::SpeechSpecificationState::known
            } else {
                generated::SpeechSpecificationState::unspecified
            },
            identical: default_identity.is_some(),
        },
    )
    .ok_or(GlobalDefaultRefusal::CompiledPlot)?;
    use generated::SpeechAllophoneChoiceFold as F;
    let index = generated::SpeechAllophoneCandidateIndex {
        index: *choice.state().index(),
    };
    let state = match choice.state().outcome() {
        SpeechAllophoneChoiceOutcome::None => F::none,
        SpeechAllophoneChoiceOutcome::SelectedAllophone => F::selected_allophone(index),
        SpeechAllophoneChoiceOutcome::Deferred => match choice.state().reason() {
            SpeechContextDecision::RequirementUnresolved => F::requirement_deferred(index),
            SpeechContextDecision::ObservationUnresolved => F::observation_deferred(index),
            _ => return Err(GlobalDefaultRefusal::CompiledPlot),
        },
        SpeechAllophoneChoiceOutcome::SelectedDefault => {
            return Err(GlobalDefaultRefusal::CompiledPlot)
        }
    };
    let result =
        generated::speech_allophone_choice_finish(generated::SpeechAllophoneChoiceFinishInput {
            state,
            allow_default: *choice.policy().allow_default(),
            default_available: default.is_some()
                && matches!(compatible, generated::SpeechContextDecision::matched),
        })
        .ok_or(GlobalDefaultRefusal::CompiledPlot)?;
    let state = if matches!(result, F::selected_default) {
        SpeechAllophoneChoiceState::new(
            0,
            SpeechAllophoneChoiceOutcome::SelectedDefault,
            SpeechContextDecision::Matched,
        )
        .map_err(GlobalDefaultRefusal::State)?
    } else {
        choice.state().clone()
    };
    Ok(GlobalDefaultChoice {
        choice,
        phoneme,
        default_identity,
        state,
    })
}
