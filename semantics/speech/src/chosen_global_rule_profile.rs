//! Exact selected standalone-rule output, inheritance and compact voice binding.
//! Timing, source resolution and commitment remain separate obligations.
use crate::{
    feature_realization::{realize_aspiration, AspirationRealization, FeatureRealizationRefusal},
    global_rule_selection::GlobalRuleChoice,
    output_features::{prepare_rule_output_features, OutputFeatureRefusal, RuleOutputFeatures},
    profile_admission::{prepare_binding, ProfileRefusal},
    semantic::*,
    VoiceEvent,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum ChosenGlobalProfileRefusal<'a> {
    Unchosen(SpeechAllophoneChoiceState),
    UnresolvedOutput(&'a PhoneSpecification),
    InventoryBasis(NativeBindingRefusal),
    DefaultOccurrence(NativeBindingRefusal),
    MissingDefinition,
    AmbiguousDefinition,
    Identity(NativeBindingRefusal),
    Features(OutputFeatureRefusal),
    Profile(ProfileRefusal),
    AcousticFeature(FeatureRealizationRefusal<'a>),
}
pub struct ChosenGlobalRuleProfile<'choice, 'source, 'profile> {
    choice: &'choice GlobalRuleChoice<'source>,
    inventory: &'source SpeechInventory,
    inventory_basis: SpeechIntentInventoryBasis,
    default: &'source SpeechOccurrenceFeatureObservation,
    default_occurrence: SpeechOccurrenceObservationMatch,
    definition: &'source SpeechPhone,
    identity: SpeechPhoneDefinitionMatch,
    features: RuleOutputFeatures<'source>,
    profile: &'profile SpeechFormantVoiceProfile,
    binding: &'profile SpeechFormantPhoneBinding,
    profile_basis: SpeechFormantProfileBasis,
    event: VoiceEvent,
    aspiration: Option<AspirationRealization<'source, 'profile>>,
}
impl<'choice, 'source, 'profile> ChosenGlobalRuleProfile<'choice, 'source, 'profile> {
    pub fn choice(&self) -> &'choice GlobalRuleChoice<'source> {
        self.choice
    }
    pub fn inventory(&self) -> &'source SpeechInventory {
        self.inventory
    }
    pub fn checked_inventory_basis(&self) -> &SpeechIntentInventoryBasis {
        &self.inventory_basis
    }
    pub fn default_features(&self) -> &'source SpeechOccurrenceFeatureObservation {
        self.default
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
    pub fn features(&self) -> &RuleOutputFeatures<'source> {
        &self.features
    }
    pub fn profile(&self) -> &'profile SpeechFormantVoiceProfile {
        self.profile
    }
    pub fn binding(&self) -> &'profile SpeechFormantPhoneBinding {
        self.binding
    }
    pub fn checked_profile_basis(&self) -> &SpeechFormantProfileBasis {
        &self.profile_basis
    }
    pub fn aspiration(&self) -> Option<&AspirationRealization<'source, 'profile>> {
        self.aspiration.as_ref()
    }
    pub fn into_realization(
        self,
    ) -> (
        RuleOutputFeatures<'source>,
        Option<AspirationRealization<'source, 'profile>>,
    ) {
        (self.features, self.aspiration)
    }
    pub fn event(&self) -> VoiceEvent {
        self.event
    }
    /// Move the borrowed feature receipt into an owning preparation aggregate.
    pub fn into_features(self) -> RuleOutputFeatures<'source> {
        self.features
    }
}
/// Default features are explicitly supplied occurrence evidence, not a guess
/// from phoneme definitions. Their provenance is retained, not authenticated.
/// The compact voice currently refuses every nonempty inherited feature bundle.
pub fn prepare_chosen_global_rule_profile<'choice, 'source, 'profile>(
    choice: &'choice GlobalRuleChoice<'source>,
    inventory: &'source SpeechInventory,
    profile: &'profile SpeechFormantVoiceProfile,
    default: &'source SpeechOccurrenceFeatureObservation,
) -> Result<ChosenGlobalRuleProfile<'choice, 'source, 'profile>, ChosenGlobalProfileRefusal<'source>>
{
    prepare_inner(choice, inventory, profile, default, None)
}
/// Realize inherited aspiration using the enclosed exact base voice profile.
pub fn prepare_aspirated_global_rule_profile<'choice, 'source, 'profile>(
    choice: &'choice GlobalRuleChoice<'source>,
    inventory: &'source SpeechInventory,
    profile: &'profile SpeechFormantAspirationProfile,
    default: &'source SpeechOccurrenceFeatureObservation,
) -> Result<ChosenGlobalRuleProfile<'choice, 'source, 'profile>, ChosenGlobalProfileRefusal<'source>>
{
    prepare_inner(choice, inventory, profile.voice(), default, Some(profile))
}
fn prepare_inner<'choice, 'source, 'profile>(
    choice: &'choice GlobalRuleChoice<'source>,
    inventory: &'source SpeechInventory,
    profile: &'profile SpeechFormantVoiceProfile,
    default: &'source SpeechOccurrenceFeatureObservation,
    aspiration_profile: Option<&'profile SpeechFormantAspirationProfile>,
) -> Result<ChosenGlobalRuleProfile<'choice, 'source, 'profile>, ChosenGlobalProfileRefusal<'source>>
{
    let rule = choice
        .selected_rule()
        .ok_or_else(|| ChosenGlobalProfileRefusal::Unchosen(choice.state().clone()))?;
    let PhoneSpecification::Known(selected) = rule.phone() else {
        return Err(ChosenGlobalProfileRefusal::UnresolvedOutput(rule.phone()));
    };
    let inventory_basis = SpeechIntentInventoryBasis::new(
        choice.profile().inventory_id().clone(),
        choice.profile().language().clone(),
        inventory.identity().clone(),
        inventory.language().clone(),
    )
    .map_err(ChosenGlobalProfileRefusal::InventoryBasis)?;
    let default_occurrence = SpeechOccurrenceObservationMatch::new(
        choice.occurrence().segment().occurrence().clone(),
        default.occurrence().clone(),
    )
    .map_err(ChosenGlobalProfileRefusal::DefaultOccurrence)?;
    let mut matches = inventory
        .phones()
        .as_slice()
        .iter()
        .filter(|definition| definition.identity() == selected);
    let definition = matches
        .next()
        .ok_or(ChosenGlobalProfileRefusal::MissingDefinition)?;
    if matches.next().is_some() {
        return Err(ChosenGlobalProfileRefusal::AmbiguousDefinition);
    }
    let identity = SpeechPhoneDefinitionMatch::new(selected.clone(), definition.identity().clone())
        .map_err(ChosenGlobalProfileRefusal::Identity)?;
    let features = prepare_rule_output_features(rule, Some(default.features()), Some(definition))
        .map_err(ChosenGlobalProfileRefusal::Features)?;
    let (binding, profile_basis, event) = prepare_binding(
        inventory,
        definition,
        profile,
        choice.occurrence().segment().stress(),
        aspiration_profile.is_none() && features.features().next().is_some(),
    )
    .map_err(ChosenGlobalProfileRefusal::Profile)?;
    let aspiration = aspiration_profile
        .map(|profile| realize_aspiration(&features, profile, event))
        .transpose()
        .map_err(ChosenGlobalProfileRefusal::AcousticFeature)?;
    let event = aspiration
        .as_ref()
        .map_or(event, AspirationRealization::event);
    Ok(ChosenGlobalRuleProfile {
        choice,
        inventory,
        inventory_basis,
        default,
        default_occurrence,
        definition,
        identity,
        features,
        profile,
        binding,
        profile_basis,
        event,
        aspiration,
    })
}
