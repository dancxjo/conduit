//! Exact voice admission of an already selected declared phoneme default.
use crate::{
    admission::{validate_feature_bundle, LocalSemanticRefusal},
    default_output_features::{prepare_default_output_features, DefaultOutputFeatures},
    feature_realization::{
        realize_aspiration_features, AspirationRealization, FeatureRealizationRefusal,
    },
    global_default_choice::GlobalDefaultChoice,
    output_features::OutputFeatureRefusal,
    profile_admission::{prepare_binding, prepare_feature_binding, ProfileRefusal},
    semantic::*,
    VoiceEvent,
};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum GlobalDefaultProfileRefusal<'a> {
    Unchosen(SpeechAllophoneChoiceState),
    DefaultOccurrence(NativeBindingRefusal),
    Features(LocalSemanticRefusal),
    MissingDefinition,
    AmbiguousDefinition,
    Identity(NativeBindingRefusal),
    Profile(ProfileRefusal),
    OutputFeatures(OutputFeatureRefusal),
    AcousticFeature(FeatureRealizationRefusal<'a>),
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
    output_features: Option<DefaultOutputFeatures<'source>>,
    aspiration: Option<AspirationRealization<'source, 'profile>>,
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
    pub fn output_features(&self) -> Option<&DefaultOutputFeatures<'source>> {
        self.output_features.as_ref()
    }
    pub fn aspiration(&self) -> Option<&AspirationRealization<'source, 'profile>> {
        self.aspiration.as_ref()
    }
    pub fn into_realization(
        self,
    ) -> (
        Option<DefaultOutputFeatures<'source>>,
        Option<AspirationRealization<'source, 'profile>>,
    ) {
        (self.output_features, self.aspiration)
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
    GlobalDefaultProfileRefusal<'source>,
> {
    prepare_inner(choice, profile, default_features, None)
}
/// Lower definition and explicit occurrence features after native default choice.
pub fn prepare_aspirated_global_default_profile<'receipt, 'choice, 'source, 'profile>(
    choice: &'receipt GlobalDefaultChoice<'choice, 'source>,
    profile: &'profile SpeechFormantAspirationProfile,
    default_features: &'source SpeechOccurrenceFeatureObservation,
) -> Result<
    PreparedGlobalDefaultProfile<'receipt, 'choice, 'source, 'profile>,
    GlobalDefaultProfileRefusal<'source>,
> {
    prepare_inner(choice, profile.voice(), default_features, Some(profile))
}
fn prepare_inner<'receipt, 'choice, 'source, 'profile>(
    choice: &'receipt GlobalDefaultChoice<'choice, 'source>,
    profile: &'profile SpeechFormantVoiceProfile,
    default_features: &'source SpeechOccurrenceFeatureObservation,
    aspiration_profile: Option<&'profile SpeechFormantAspirationProfile>,
) -> Result<
    PreparedGlobalDefaultProfile<'receipt, 'choice, 'source, 'profile>,
    GlobalDefaultProfileRefusal<'source>,
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
    let output_features = aspiration_profile
        .map(|_| prepare_default_output_features(default_features.features(), definition))
        .transpose()
        .map_err(GlobalDefaultProfileRefusal::OutputFeatures)?;
    let (binding, basis, event) = match aspiration_profile {
        Some(_) => prepare_feature_binding(inventory, definition, profile, occurrence.stress()),
        None => prepare_binding(
            inventory,
            definition,
            profile,
            occurrence.stress(),
            !default_features.features().get().as_slice().is_empty(),
        ),
    }
    .map_err(GlobalDefaultProfileRefusal::Profile)?;
    let aspiration = match (aspiration_profile, output_features.as_ref()) {
        (Some(profile), Some(features)) => Some(
            realize_aspiration_features(
                features.features().map(|entry| entry.feature()),
                profile,
                event,
            )
            .map_err(GlobalDefaultProfileRefusal::AcousticFeature)?,
        ),
        _ => None,
    };
    let event = aspiration
        .as_ref()
        .map_or(event, AspirationRealization::event);
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
        output_features,
        aspiration,
    })
}
