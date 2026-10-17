//! Immutable contextual choice plus quantitative preparation; no commitment.
use crate::{
    allophone_selection::{
        select_intent_allophone, AllophoneSelectionRefusal, IntentAllophoneChoice,
    },
    chosen_allophone_profile::{prepare_chosen_allophone_profile, ChosenProfileRefusal},
    declared_context::ExplicitAllophoneContext,
    semantic::*,
    utterance_timing::{
        prepare_utterance_timing, EventTimingReceipt, PreparedUtteranceTiming,
        UtteranceTimingRefusal, UtteranceTimingRenderRefusal,
    },
    Renderer, VoiceEvent, SOURCE_ID,
};
use alloc::vec::Vec;

#[derive(Debug)]
pub enum ContextualIntentRefusal<'a> {
    ContextCount,
    MissingContext {
        event: usize,
    },
    BoundaryContext {
        event: usize,
    },
    Timing(UtteranceTimingRefusal),
    Unchosen {
        event: usize,
        state: SpeechAllophoneChoiceState,
    },
    Choice {
        event: usize,
        reason: AllophoneSelectionRefusal<'a>,
    },
    Profile {
        event: usize,
        reason: ChosenProfileRefusal,
    },
    Renderer(UtteranceTimingRenderRefusal),
}

pub struct ContextualPhoneReceipt<'a> {
    choice: IntentAllophoneChoice<'a>,
    definition: &'a SpeechPhone,
    identity: SpeechPhoneDefinitionMatch,
    binding: &'a SpeechFormantPhoneBinding,
    basis: SpeechFormantProfileBasis,
}
impl<'a> ContextualPhoneReceipt<'a> {
    pub fn choice(&self) -> &IntentAllophoneChoice<'a> {
        &self.choice
    }
    pub fn definition(&self) -> &'a SpeechPhone {
        self.definition
    }
    pub fn checked_identity(&self) -> &SpeechPhoneDefinitionMatch {
        &self.identity
    }
    pub fn binding(&self) -> &'a SpeechFormantPhoneBinding {
        self.binding
    }
    pub fn checked_profile_basis(&self) -> &SpeechFormantProfileBasis {
        &self.basis
    }
}

/// All bounded growth occurs before rendering; callers cannot substitute events.
pub struct PreparedContextualIntent<'a> {
    timing: PreparedUtteranceTiming<'a>,
    profile: &'a SpeechFormantVoiceProfile,
    phones: Vec<ContextualPhoneReceipt<'a>>,
    events: Vec<VoiceEvent>,
}
impl<'a> PreparedContextualIntent<'a> {
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.timing.source()
    }
    pub fn timing(&self) -> &PreparedUtteranceTiming<'a> {
        &self.timing
    }
    pub fn profile(&self) -> &'a SpeechFormantVoiceProfile {
        self.profile
    }
    pub fn phones(&self) -> &[ContextualPhoneReceipt<'a>] {
        &self.phones
    }
    pub fn events(&self) -> &[VoiceEvent] {
        &self.events
    }
    pub fn compiled_source_id(&self) -> &'static str {
        SOURCE_ID
    }
    pub fn renderer(&self) -> Result<Renderer<'_>, UtteranceTimingRenderRefusal> {
        self.timing.renderer(&self.events)
    }
}

/// One explicit context slot per original event: Some for segments, None for
/// boundaries. Neither context nor quantitative facts are inferred here.
pub fn prepare_contextual_intent<'a>(
    source: &'a SpeechUtteranceIntent,
    inventory: &'a SpeechInventory,
    profile: &'a SpeechFormantVoiceProfile,
    boundaries: &'a SpeechFormantBoundaryProfile,
    policy: &'a SpeechAllophoneChoicePolicy,
    contexts: &[Option<ExplicitAllophoneContext<'a>>],
) -> Result<PreparedContextualIntent<'a>, ContextualIntentRefusal<'a>> {
    if contexts.len() != source.events().as_slice().len() {
        return Err(ContextualIntentRefusal::ContextCount);
    }
    // Validate alignment before any comparison, including boundary-only inputs.
    for (event, (value, context)) in source.events().as_slice().iter().zip(contexts).enumerate() {
        match (value, context) {
            (SpeechUtteranceIntentEvent::Segment(_), None) => {
                return Err(ContextualIntentRefusal::MissingContext { event })
            }
            (SpeechUtteranceIntentEvent::Boundary(_), Some(_)) => {
                return Err(ContextualIntentRefusal::BoundaryContext { event })
            }
            _ => {}
        }
    }
    let timing =
        prepare_utterance_timing(source, boundaries).map_err(ContextualIntentRefusal::Timing)?;
    let mut phones = Vec::with_capacity(timing.receipts().len());
    let mut events = Vec::with_capacity(timing.receipts().len());
    for (event, (receipt, context)) in timing.receipts().iter().zip(contexts).enumerate() {
        match receipt {
            EventTimingReceipt::Segment { .. } => {
                let explicit = context.ok_or(ContextualIntentRefusal::MissingContext { event })?;
                let choice = select_intent_allophone(source, event, inventory, policy, explicit)
                    .map_err(|reason| ContextualIntentRefusal::Choice { event, reason })?;
                if choice.selected_phone().is_none() {
                    return Err(ContextualIntentRefusal::Unchosen {
                        event,
                        state: choice.state().clone(),
                    });
                }
                let projected = prepare_chosen_allophone_profile(&choice, profile)
                    .map_err(|reason| ContextualIntentRefusal::Profile { event, reason })?;
                let definition = projected.definition();
                let identity = projected.checked_identity().clone();
                let binding = projected.binding();
                let basis = projected.checked_basis().clone();
                events.push(projected.event());
                phones.push(ContextualPhoneReceipt {
                    choice,
                    definition,
                    identity,
                    binding,
                    basis,
                });
            }
            EventTimingReceipt::Boundary { event, .. } => events.push(*event),
        }
    }
    timing
        .renderer(&events)
        .map_err(ContextualIntentRefusal::Renderer)?;
    Ok(PreparedContextualIntent {
        timing,
        profile,
        phones,
        events,
    })
}

/// Complete source coverage and contextual rendering preparation from the same
/// immutable intent. This does not establish causality, authority or commitment.
pub struct PreparedSourcedContextualIntent<'a> {
    sources: crate::intent_sources::PreparedIntentSources<'a>,
    realization: PreparedContextualIntent<'a>,
}
impl<'a> PreparedSourcedContextualIntent<'a> {
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.sources.intent()
    }
    pub fn sources(&self) -> &crate::intent_sources::PreparedIntentSources<'a> {
        &self.sources
    }
    pub fn realization(&self) -> &PreparedContextualIntent<'a> {
        &self.realization
    }
    pub fn renderer(&self) -> Result<Renderer<'_>, UtteranceTimingRenderRefusal> {
        self.realization.renderer()
    }
}
#[derive(Debug)]
pub enum SourcedContextualRefusal<'a> {
    Sources(crate::intent_sources::IntentSourcesRefusal),
    Contextual(ContextualIntentRefusal<'a>),
}
/// Materials are supplied in exact event/source order. All source and realization
/// obligations must succeed before a playable result escapes preparation.
#[allow(clippy::too_many_arguments)] // Explicit immutable bases, without ambient lookup.
pub fn prepare_sourced_contextual_intent<'a>(
    source: &'a SpeechUtteranceIntent,
    materials: &[crate::intent_sources::IntentSourceMaterial<'a>],
    inventory: &'a SpeechInventory,
    profile: &'a SpeechFormantVoiceProfile,
    boundaries: &'a SpeechFormantBoundaryProfile,
    policy: &'a SpeechAllophoneChoicePolicy,
    contexts: &[Option<ExplicitAllophoneContext<'a>>],
) -> Result<PreparedSourcedContextualIntent<'a>, SourcedContextualRefusal<'a>> {
    let sources = crate::intent_sources::resolve_intent_sources(source, materials)
        .map_err(SourcedContextualRefusal::Sources)?;
    let realization =
        prepare_contextual_intent(source, inventory, profile, boundaries, policy, contexts)
            .map_err(SourcedContextualRefusal::Contextual)?;
    Ok(PreparedSourcedContextualIntent {
        sources,
        realization,
    })
}
