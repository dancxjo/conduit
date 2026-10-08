//! Composition with the existing opaque contextual allophone choice. This owns
//! no new inventory, utterance, context selection or execution authority.
use crate::{
    allophone_selection::IntentAllophoneChoice, contextual_gestures::ContextualGestureRefusal,
    gesture_lowering::PreparedDeclaredPhoneGestures, semantic::*,
};
use conduit_plot::rust_binding::NativeRustBinding;
pub struct PreparedContextualGreetingPhoneGestures<'choice, 'source> {
    choice: &'choice IntentAllophoneChoice<'source>,
    profile: crate::PreparedGreetingPhoneGestures,
}
impl<'choice, 'source> PreparedContextualGreetingPhoneGestures<'choice, 'source> {
    pub fn choice(&self) -> &'choice IntentAllophoneChoice<'source> {
        self.choice
    }
    pub fn profile(&self) -> &crate::PreparedGreetingPhoneGestures {
        &self.profile
    }
    pub fn lowered(&self) -> &PreparedDeclaredPhoneGestures {
        self.profile.lowered()
    }
}
/// Retains the original opaque choice, including exact original inventory,
/// occurrence/revision, candidates and context decisions. The selected definition
/// must be unique in that exact inventory; no private renderer code selects it.
pub fn prepare_contextual_greeting_phone_gestures<'choice, 'source>(
    choice: &'choice IntentAllophoneChoice<'source>,
    timing: &SpeechGestureTiming,
    policy: SpeechGreetingLossPolicy,
) -> Result<PreparedContextualGreetingPhoneGestures<'choice, 'source>, ContextualGestureRefusal> {
    let selected = choice
        .selected_phone()
        .ok_or(ContextualGestureRefusal::NoSelectedPhone)?;
    let mut definitions = choice
        .inventory()
        .phones()
        .as_slice()
        .iter()
        .filter(|phone| phone.identity() == selected);
    let definition = definitions
        .next()
        .ok_or(ContextualGestureRefusal::MissingDefinition)?;
    if definitions.next().is_some() {
        return Err(ContextualGestureRefusal::AmbiguousDefinition);
    }
    let event = &choice.occurrence().intent().events().as_slice()[choice.occurrence().event()];
    let encode = |result: Result<
        alloc::vec::Vec<u8>,
        conduit_plot::rust_binding::NativeBindingRefusal,
    >| result.map_err(ContextualGestureRefusal::Native);
    let profile = crate::prepare_greeting_phone_gestures(
        &encode(event.clone().encode())?,
        &encode(choice.occurrence().membership().clone().encode())?,
        &encode(definition.clone().encode())?,
        &encode(choice.state().clone().encode())?,
        &encode(timing.clone().encode())?,
        policy,
    )
    .map_err(ContextualGestureRefusal::Gesture)?;
    Ok(PreparedContextualGreetingPhoneGestures { choice, profile })
}
