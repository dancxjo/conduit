//! Explicit, bounded aspiration lowering. Original inherited values stay borrowed.
use crate::{generated, output_features::RuleOutputFeatures, semantic::*, VoiceEvent};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum FeatureRealizationRefusal<'a> {
    UnsupportedFeature(&'a SpeechFeature),
    Identity(NativeBindingRefusal),
    Unresolved(&'a FeatureSpecification),
    UnsupportedValue(&'a FeatureSpecification),
    UnsupportedPhone,
    CompiledPlot,
}
pub struct AspirationRealization<'source, 'profile> {
    profile: &'profile SpeechFormantAspirationProfile,
    feature: Option<&'source SpeechFeature>,
    identity: Option<SpeechFeatureIdentityMatch>,
    event: VoiceEvent,
}
impl<'source, 'profile> AspirationRealization<'source, 'profile> {
    pub fn profile(&self) -> &'profile SpeechFormantAspirationProfile {
        self.profile
    }
    pub fn feature(&self) -> Option<&'source SpeechFeature> {
        self.feature
    }
    pub fn checked_identity(&self) -> Option<&SpeechFeatureIdentityMatch> {
        self.identity.as_ref()
    }
    pub fn event(&self) -> VoiceEvent {
        self.event
    }
}
pub(crate) fn realize_aspiration<'source, 'profile>(
    features: &RuleOutputFeatures<'source>,
    profile: &'profile SpeechFormantAspirationProfile,
    event: VoiceEvent,
) -> Result<AspirationRealization<'source, 'profile>, FeatureRealizationRefusal<'source>> {
    realize_aspiration_features(
        features.features().map(|entry| entry.feature()),
        profile,
        event,
    )
}
pub(crate) fn realize_aspiration_features<'source, 'profile>(
    features: impl Iterator<Item = &'source SpeechFeature>,
    profile: &'profile SpeechFormantAspirationProfile,
    event: VoiceEvent,
) -> Result<AspirationRealization<'source, 'profile>, FeatureRealizationRefusal<'source>> {
    let mut selected = None;
    let mut identity = None;
    // Every inherited feature must be covered. No feature can be dropped while
    // returning a playable event, including a later unsupported key.
    for feature in features {
        if feature.identity() != profile.feature_id() {
            return Err(FeatureRealizationRefusal::UnsupportedFeature(feature));
        }
        identity = Some(
            SpeechFeatureIdentityMatch::new(
                feature.identity().clone(),
                profile.feature_id().clone(),
            )
            .map_err(FeatureRealizationRefusal::Identity)?,
        );
        selected = Some(feature);
    }
    let Some(feature) = selected else {
        return Ok(AspirationRealization {
            profile,
            feature: None,
            identity,
            event,
        });
    };
    use generated::SpeechSpecificationState as S;
    let (state, boolean_kind, boolean_value) = match feature.specification() {
        FeatureSpecification::Known(SpeechFeatureValue::Boolean(value)) => (S::known, true, *value),
        FeatureSpecification::Known(_) => (S::known, false, false),
        FeatureSpecification::Unknown => (S::unknown, false, false),
        FeatureSpecification::Unspecified => (S::unspecified, false, false),
        FeatureSpecification::NotApplicable => (S::not_applicable, false, false),
        FeatureSpecification::Variable(_) => (S::variable, false, false),
        FeatureSpecification::Gradient(_) => (S::gradient, false, false),
    };
    let VoiceEvent::phone(mut input) = event else {
        return Err(FeatureRealizationRefusal::UnsupportedPhone);
    };
    match generated::speech_aspiration_realize(generated::SpeechAspirationInput {
        phone: input.phone,
        state,
        boolean_kind,
        boolean_value,
    })
    .ok_or(FeatureRealizationRefusal::CompiledPlot)?
    {
        generated::SpeechAspirationResult::realized(phone) => input.phone = phone,
        generated::SpeechAspirationResult::unresolved => {
            return Err(FeatureRealizationRefusal::Unresolved(
                feature.specification(),
            ))
        }
        generated::SpeechAspirationResult::unsupported_value => {
            return Err(FeatureRealizationRefusal::UnsupportedValue(
                feature.specification(),
            ))
        }
        generated::SpeechAspirationResult::unsupported_phone => {
            return Err(FeatureRealizationRefusal::UnsupportedPhone)
        }
    }
    Ok(AspirationRealization {
        profile,
        feature: Some(feature),
        identity,
        event: VoiceEvent::phone(input),
    })
}
