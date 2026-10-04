//! Whole-intent global-rule preparation with private immutable event storage.
use crate::{
    chosen_global_rule_profile::{prepare_chosen_global_rule_profile, ChosenGlobalProfileRefusal},
    declared_context::ExplicitAllophoneContext,
    global_default_choice::{finish_global_default_choice, GlobalDefaultRefusal},
    global_default_profile::{prepare_global_default_profile, GlobalDefaultProfileRefusal},
    global_rule_selection::{
        select_global_allophone_rule, GlobalRuleChoice, GlobalRuleSelectionRefusal,
    },
    intent_phoneme_inventory::{
        resolve_intent_inventory_phoneme, IntentPhonemeRefusal, ResolvedIntentPhoneme,
    },
    output_features::RuleOutputFeatures,
    semantic::*,
    utterance_timing::{
        prepare_utterance_timing, EventTimingReceipt, PreparedUtteranceTiming,
        UtteranceTimingRefusal, UtteranceTimingRenderRefusal,
    },
    Renderer, VoiceEvent,
};
use alloc::{boxed::Box, vec::Vec};
#[derive(Clone, Copy)]
pub struct GlobalSegmentPreparation<'a> {
    pub context: ExplicitAllophoneContext<'a>,
    pub observed_features: Option<&'a SpeechOccurrenceFeatureObservation>,
    pub default_features: &'a SpeechOccurrenceFeatureObservation,
}
#[derive(Debug)]
pub enum GlobalIntentRefusal<'a> {
    EvidenceCount,
    MissingSegmentEvidence {
        event: usize,
    },
    BoundaryEvidence {
        event: usize,
    },
    Timing(UtteranceTimingRefusal),
    Phoneme {
        event: usize,
        reason: Box<IntentPhonemeRefusal<'a>>,
    },
    Choice {
        event: usize,
        reason: Box<GlobalRuleSelectionRefusal<'a>>,
    },
    Profile {
        event: usize,
        reason: ChosenGlobalProfileRefusal<'a>,
    },
    DefaultChoice {
        event: usize,
        reason: Box<GlobalDefaultRefusal<'a>>,
    },
    DefaultProfile {
        event: usize,
        reason: GlobalDefaultProfileRefusal,
    },
    Renderer(UtteranceTimingRenderRefusal),
}
pub enum GlobalIntentRealization<'a> {
    Rule(RuleOutputFeatures<'a>),
    Default {
        identity: Option<SpeechPhonePatternIdentity>,
    },
}
pub struct GlobalIntentPhoneReceipt<'a> {
    event: usize,
    choice: GlobalRuleChoice<'a>,
    phoneme: ResolvedIntentPhoneme<'a>,
    default_features: &'a SpeechOccurrenceFeatureObservation,
    default_occurrence: SpeechOccurrenceObservationMatch,
    definition: &'a SpeechPhone,
    identity: SpeechPhoneDefinitionMatch,
    inventory_basis: SpeechIntentInventoryBasis,
    realization: GlobalIntentRealization<'a>,
    completed_choice: SpeechAllophoneChoiceState,
    binding: &'a SpeechFormantPhoneBinding,
    profile_basis: SpeechFormantProfileBasis,
}
impl<'a> GlobalIntentPhoneReceipt<'a> {
    pub fn event_index(&self) -> usize {
        self.event
    }
    pub fn phoneme(&self) -> &ResolvedIntentPhoneme<'a> {
        &self.phoneme
    }
    pub fn choice(&self) -> &GlobalRuleChoice<'a> {
        &self.choice
    }
    pub fn default_features(&self) -> &'a SpeechOccurrenceFeatureObservation {
        self.default_features
    }
    pub fn checked_default_occurrence(&self) -> &SpeechOccurrenceObservationMatch {
        &self.default_occurrence
    }
    pub fn definition(&self) -> &'a SpeechPhone {
        self.definition
    }
    pub fn checked_identity(&self) -> &SpeechPhoneDefinitionMatch {
        &self.identity
    }
    pub fn checked_inventory_basis(&self) -> &SpeechIntentInventoryBasis {
        &self.inventory_basis
    }
    pub fn features(&self) -> Option<&RuleOutputFeatures<'a>> {
        match &self.realization {
            GlobalIntentRealization::Rule(features) => Some(features),
            GlobalIntentRealization::Default { .. } => None,
        }
    }
    pub fn realization(&self) -> &GlobalIntentRealization<'a> {
        &self.realization
    }
    pub fn completed_choice(&self) -> &SpeechAllophoneChoiceState {
        &self.completed_choice
    }
    pub fn binding(&self) -> &'a SpeechFormantPhoneBinding {
        self.binding
    }
    pub fn checked_profile_basis(&self) -> &SpeechFormantProfileBasis {
        &self.profile_basis
    }
}
pub struct PreparedGlobalIntent<'a> {
    timing: PreparedUtteranceTiming<'a>,
    inventory: &'a SpeechInventory,
    profile: &'a SpeechFormantVoiceProfile,
    rules: &'a SpeechAllophoneRuleProfile,
    phones: Vec<GlobalIntentPhoneReceipt<'a>>,
    events: Vec<VoiceEvent>,
}
impl<'a> PreparedGlobalIntent<'a> {
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.timing.source()
    }
    pub fn timing(&self) -> &PreparedUtteranceTiming<'a> {
        &self.timing
    }
    pub fn inventory(&self) -> &'a SpeechInventory {
        self.inventory
    }
    pub fn profile(&self) -> &'a SpeechFormantVoiceProfile {
        self.profile
    }
    pub fn rules(&self) -> &'a SpeechAllophoneRuleProfile {
        self.rules
    }
    pub fn phones(&self) -> &[GlobalIntentPhoneReceipt<'a>] {
        &self.phones
    }
    pub fn events(&self) -> &[VoiceEvent] {
        &self.events
    }
    pub fn renderer(&self) -> Result<Renderer<'_>, UtteranceTimingRenderRefusal> {
        self.timing.renderer(&self.events)
    }
}
/// Exactly one evidence slot per original event: Some for segments, None for
/// boundaries. All choices and profile obligations pass before a renderer can
/// escape. This does not resolve source materials or establish commitment.
pub fn prepare_global_intent<'a>(
    source: &'a SpeechUtteranceIntent,
    inventory: &'a SpeechInventory,
    profile: &'a SpeechFormantVoiceProfile,
    boundaries: &'a SpeechFormantBoundaryProfile,
    rules: &'a SpeechAllophoneRuleProfile,
    policy: &'a SpeechAllophoneChoicePolicy,
    evidence: &[Option<GlobalSegmentPreparation<'a>>],
) -> Result<PreparedGlobalIntent<'a>, GlobalIntentRefusal<'a>> {
    if evidence.len() != source.events().as_slice().len() {
        return Err(GlobalIntentRefusal::EvidenceCount);
    }
    for (event, (value, supplied)) in source.events().as_slice().iter().zip(evidence).enumerate() {
        match (value, supplied) {
            (SpeechUtteranceIntentEvent::Segment(_), None) => {
                return Err(GlobalIntentRefusal::MissingSegmentEvidence { event })
            }
            (SpeechUtteranceIntentEvent::Boundary(_), Some(_)) => {
                return Err(GlobalIntentRefusal::BoundaryEvidence { event })
            }
            _ => {}
        }
    }
    let timing =
        prepare_utterance_timing(source, boundaries).map_err(GlobalIntentRefusal::Timing)?;
    let mut phones = Vec::with_capacity(timing.receipts().len());
    let mut events = Vec::with_capacity(timing.receipts().len());
    for (event, (receipt, supplied)) in timing.receipts().iter().zip(evidence).enumerate() {
        match receipt {
            EventTimingReceipt::Segment { .. } => {
                let supplied =
                    supplied.ok_or(GlobalIntentRefusal::MissingSegmentEvidence { event })?;
                let phoneme = resolve_intent_inventory_phoneme(source, event, inventory).map_err(
                    |reason| GlobalIntentRefusal::Phoneme {
                        event,
                        reason: Box::new(reason),
                    },
                )?;
                let choice = select_global_allophone_rule(
                    source,
                    event,
                    rules,
                    policy,
                    supplied.observed_features,
                    supplied.context,
                )
                .map_err(|reason| GlobalIntentRefusal::Choice {
                    event,
                    reason: Box::new(reason),
                })?;
                let completed =
                    finish_global_default_choice(&choice, inventory).map_err(|reason| {
                        GlobalIntentRefusal::DefaultChoice {
                            event,
                            reason: Box::new(reason),
                        }
                    })?;
                let completed_choice = completed.state().clone();
                let (
                    definition,
                    identity,
                    inventory_basis,
                    default_occurrence,
                    binding,
                    profile_basis,
                    realization,
                    voice_event,
                ) = if completed.selected_default().is_some() {
                    let projected = prepare_global_default_profile(
                        &completed,
                        profile,
                        supplied.default_features,
                    )
                    .map_err(|reason| GlobalIntentRefusal::DefaultProfile { event, reason })?;
                    (
                        projected.definition(),
                        projected.checked_identity().clone(),
                        phoneme.checked_basis().clone(),
                        projected.checked_default_occurrence().clone(),
                        projected.binding(),
                        projected.checked_basis().clone(),
                        GlobalIntentRealization::Default {
                            identity: completed.checked_default_identity().cloned(),
                        },
                        projected.event(),
                    )
                } else {
                    let projected = prepare_chosen_global_rule_profile(
                        &choice,
                        inventory,
                        profile,
                        supplied.default_features,
                    )
                    .map_err(|reason| GlobalIntentRefusal::Profile { event, reason })?;
                    let definition = projected.definition();
                    let identity = projected.checked_identity().clone();
                    let inventory_basis = projected.checked_inventory_basis().clone();
                    let default_occurrence = projected.checked_default_occurrence().clone();
                    let binding = projected.binding();
                    let profile_basis = projected.checked_profile_basis().clone();
                    let voice_event = projected.event();
                    (
                        definition,
                        identity,
                        inventory_basis,
                        default_occurrence,
                        binding,
                        profile_basis,
                        GlobalIntentRealization::Rule(projected.into_features()),
                        voice_event,
                    )
                };
                events.push(voice_event);
                phones.push(GlobalIntentPhoneReceipt {
                    event,
                    choice,
                    phoneme,
                    default_features: supplied.default_features,
                    default_occurrence,
                    definition,
                    identity,
                    inventory_basis,
                    realization,
                    completed_choice,
                    binding,
                    profile_basis,
                });
            }
            EventTimingReceipt::Boundary { event, .. } => events.push(*event),
        }
    }
    timing
        .renderer(&events)
        .map_err(GlobalIntentRefusal::Renderer)?;
    Ok(PreparedGlobalIntent {
        timing,
        inventory,
        profile,
        rules,
        phones,
        events,
    })
}
