//! Exact voice admission of an already selected declared phoneme default.
use crate::{
    admission::{validate_feature_bundle, LocalSemanticRefusal},
    global_default_choice::GlobalDefaultChoice,
    profile_admission::{prepare_binding, ProfileRefusal},
    semantic::*,
    VoiceEvent,
};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum GlobalDefaultProfileRefusal {
    Unchosen(SpeechAllophoneChoiceState),
    DefaultOccurrence(NativeBindingRefusal),
    Features(LocalSemanticRefusal),
    MissingDefinition,
    AmbiguousDefinition,
    Identity(NativeBindingRefusal),
    Profile(ProfileRefusal),
}
pub struct PreparedGlobalDefaultProfile<'receipt, 'choice, 'source, 'profile> {
    choice: &'receipt GlobalDefaultChoice<'choice, 'source>,
    default_features: &'source SpeechOccurrenceFeatureObservation,
    default_occurrence: SpeechOccurrenceObservationMatch,
    definition: &'source SpeechPhone,
    identity: SpeechPhoneDefinitionMatch,
    profile: &'profile SpeechFormantVoiceProfile,
    binding: &'profile SpeechFormantPhoneBinding,
    basis: SpeechFormantProfileBasis,
    event: VoiceEvent,
}
impl<'receipt, 'choice, 'source, 'profile>
    PreparedGlobalDefaultProfile<'receipt, 'choice, 'source, 'profile>
{
    pub fn choice(&self) -> &'receipt GlobalDefaultChoice<'choice, 'source> {
        self.choice
    }
    pub fn default_features(&self) -> &'source SpeechOccurrenceFeatureObservation {
        self.default_features
    }
    pub fn checked_default_occurrence(&self) -> &SpeechOccurrenceObservationMatch {
        &self.default_occurrence
    }
    pub fn definition(&self) -> &'source SpeechPhone {
        self.definition
    }
    pub fn checked_identity(&self) -> &SpeechPhoneDefinitionMatch {
        &self.identity
    }
    pub fn profile(&self) -> &'profile SpeechFormantVoiceProfile {
        self.profile
    }
    pub fn binding(&self) -> &'profile SpeechFormantPhoneBinding {
        self.binding
    }
    pub fn checked_basis(&self) -> &SpeechFormantProfileBasis {
        &self.basis
    }
    pub fn event(&self) -> VoiceEvent {
        self.event
    }
}
/// Defaults are explicit occurrence evidence, never inferred from phoneme features.
/// Their provenance is retained, not authenticated. Nonempty default or phone
/// definition features remain unsupported by this exact compact voice.
pub fn prepare_global_default_profile<'receipt, 'choice, 'source, 'profile>(
    choice: &'receipt GlobalDefaultChoice<'choice, 'source>,
    profile: &'profile SpeechFormantVoiceProfile,
    default_features: &'source SpeechOccurrenceFeatureObservation,
) -> Result<
    PreparedGlobalDefaultProfile<'receipt, 'choice, 'source, 'profile>,
    GlobalDefaultProfileRefusal,
> {
    let selected = choice
        .selected_default()
        .ok_or_else(|| GlobalDefaultProfileRefusal::Unchosen(choice.state().clone()))?;
    let occurrence = choice.choice().occurrence().segment();
    let default_occurrence = SpeechOccurrenceObservationMatch::new(
        occurrence.occurrence().clone(),
        default_features.occurrence().clone(),
    )
    .map_err(GlobalDefaultProfileRefusal::DefaultOccurrence)?;
    validate_feature_bundle(default_features.features())
        .map_err(GlobalDefaultProfileRefusal::Features)?;
    let inventory = choice.phoneme().inventory();
    let mut matches = inventory
        .phones()
        .as_slice()
        .iter()
        .filter(|value| value.identity() == selected);
    let definition = matches
        .next()
        .ok_or(GlobalDefaultProfileRefusal::MissingDefinition)?;
    if matches.next().is_some() {
        return Err(GlobalDefaultProfileRefusal::AmbiguousDefinition);
    }
    let identity = SpeechPhoneDefinitionMatch::new(selected.clone(), definition.identity().clone())
        .map_err(GlobalDefaultProfileRefusal::Identity)?;
    let (binding, basis, event) = prepare_binding(
        inventory,
        definition,
        profile,
        occurrence.stress(),
        !default_features.features().get().as_slice().is_empty(),
    )
    .map_err(GlobalDefaultProfileRefusal::Profile)?;
    Ok(PreparedGlobalDefaultProfile {
        choice,
        default_features,
        default_occurrence,
        definition,
        identity,
        profile,
        binding,
        basis,
        event,
    })
}
