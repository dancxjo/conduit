//! Exact selected standalone-rule output, inheritance and compact voice binding.
//! Timing, source resolution and commitment remain separate obligations.
use crate::{
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
    pub fn event(&self) -> VoiceEvent {
        self.event
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
        features.features().next().is_some(),
    )
    .map_err(ChosenGlobalProfileRefusal::Profile)?;
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
    })
}
