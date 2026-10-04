//! Exact chosen-phone profile projection. Timing, sources and commitment remain
//! separate obligations; the retained choice is not rewritten into its intent.
use crate::{
    allophone_selection::IntentAllophoneChoice,
    profile_admission::{prepare_binding, ProfileRefusal},
    semantic::*,
    VoiceEvent,
};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum ChosenProfileRefusal {
    NoChosenPhone,
    MissingDefinition,
    AmbiguousDefinition,
    Identity(NativeBindingRefusal),
    Profile(ProfileRefusal),
}
pub struct ChosenAllophoneProfile<'a, 'source> {
    choice: &'a IntentAllophoneChoice<'source>,
    definition: &'source SpeechPhone,
    identity: SpeechPhoneDefinitionMatch,
    profile: &'a SpeechFormantVoiceProfile,
    binding: &'a SpeechFormantPhoneBinding,
    basis: SpeechFormantProfileBasis,
    event: VoiceEvent,
}
impl<'a, 'source> ChosenAllophoneProfile<'a, 'source> {
    pub fn choice(&self) -> &'a IntentAllophoneChoice<'source> {
        self.choice
    }
    pub fn definition(&self) -> &'source SpeechPhone {
        self.definition
    }
    pub fn checked_identity(&self) -> &SpeechPhoneDefinitionMatch {
        &self.identity
    }
    pub fn profile(&self) -> &'a SpeechFormantVoiceProfile {
        self.profile
    }
    pub fn binding(&self) -> &'a SpeechFormantPhoneBinding {
        self.binding
    }
    pub fn checked_basis(&self) -> &SpeechFormantProfileBasis {
        &self.basis
    }
    pub fn event(&self) -> VoiceEvent {
        self.event
    }
}
pub fn prepare_chosen_allophone_profile<'a, 'source>(
    choice: &'a IntentAllophoneChoice<'source>,
    profile: &'a SpeechFormantVoiceProfile,
) -> Result<ChosenAllophoneProfile<'a, 'source>, ChosenProfileRefusal> {
    let selected = choice
        .selected_phone()
        .ok_or(ChosenProfileRefusal::NoChosenPhone)?;
    let mut matches = choice
        .inventory()
        .phones()
        .as_slice()
        .iter()
        .filter(|definition| definition.identity() == selected);
    let definition = matches
        .next()
        .ok_or(ChosenProfileRefusal::MissingDefinition)?;
    if matches.next().is_some() {
        return Err(ChosenProfileRefusal::AmbiguousDefinition);
    }
    let identity = SpeechPhoneDefinitionMatch::new(selected.clone(), definition.identity().clone())
        .map_err(ChosenProfileRefusal::Identity)?;
    let (binding, basis, event) = prepare_binding(
        choice.inventory(),
        definition,
        profile,
        choice.occurrence().segment().stress(),
        false,
    )
    .map_err(ChosenProfileRefusal::Profile)?;
    Ok(ChosenAllophoneProfile {
        choice,
        definition,
        identity,
        profile,
        binding,
        basis,
        event,
    })
}
