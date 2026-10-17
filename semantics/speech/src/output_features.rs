//! Bounded borrowed output-feature inheritance; no rule selection or lookup.
use crate::{
    admission::{validate_feature_bundle, LocalSemanticRefusal},
    generated,
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFeatureLayer {
    DefaultRealization,
    PhoneDefinition,
    RuleOutput,
}
#[derive(Debug)]
pub enum OutputFeatureRefusal {
    Bundle {
        layer: OutputFeatureLayer,
        reason: LocalSemanticRefusal,
    },
    UnexpectedPhoneDefinition,
    PhoneIdentity(NativeBindingRefusal),
    Capacity,
    CompiledPlot,
}
pub struct InheritedOutputFeature<'a> {
    feature: &'a SpeechFeature,
    layer: OutputFeatureLayer,
}
impl<'a> InheritedOutputFeature<'a> {
    pub(crate) fn new(feature: &'a SpeechFeature, layer: OutputFeatureLayer) -> Self {
        Self { feature, layer }
    }
    pub fn feature(&self) -> &'a SpeechFeature {
        self.feature
    }
    pub fn layer(&self) -> OutputFeatureLayer {
        self.layer
    }
}
pub struct RuleOutputFeatures<'a> {
    rule: &'a SpeechAllophoneRule,
    default: Option<&'a SpeechFeatureBundle>,
    phone: Option<&'a SpeechPhone>,
    checked_phone: Option<SpeechPhoneDefinitionMatch>,
    features: [Option<InheritedOutputFeature<'a>>; 16],
    count: usize,
}
impl<'a> RuleOutputFeatures<'a> {
    pub fn rule(&self) -> &'a SpeechAllophoneRule {
        self.rule
    }
    pub fn default_features(&self) -> Option<&'a SpeechFeatureBundle> {
        self.default
    }
    pub fn phone_definition(&self) -> Option<&'a SpeechPhone> {
        self.phone
    }
    pub fn checked_phone(&self) -> Option<&SpeechPhoneDefinitionMatch> {
        self.checked_phone.as_ref()
    }
    pub fn features(&self) -> impl Iterator<Item = &InheritedOutputFeature<'a>> {
        self.features[..self.count].iter().flatten()
    }
}
fn specification_state(value: &PhoneSpecification) -> generated::SpeechSpecificationState {
    use generated::SpeechSpecificationState as S;
    match value {
        PhoneSpecification::Known(_) => S::known,
        PhoneSpecification::Unknown => S::unknown,
        PhoneSpecification::Unspecified => S::unspecified,
        PhoneSpecification::NotApplicable => S::not_applicable,
        PhoneSpecification::Variable(_) => S::variable,
        PhoneSpecification::Gradient(_) => S::gradient,
    }
}
/// Supplied default features are explicit preparation inputs, not inferred
/// token facts. A supplied phone is checked by exact ID, not inventory membership.
/// This receipt preserves missing layers and cannot authorize a rule or phone.
pub fn prepare_rule_output_features<'a>(
    rule: &'a SpeechAllophoneRule,
    default: Option<&'a SpeechFeatureBundle>,
    phone: Option<&'a SpeechPhone>,
) -> Result<RuleOutputFeatures<'a>, OutputFeatureRefusal> {
    let checked_phone = match (rule.phone(), phone) {
        (PhoneSpecification::Known(id), Some(phone)) => Some(
            SpeechPhoneDefinitionMatch::new(phone.identity().clone(), id.clone())
                .map_err(OutputFeatureRefusal::PhoneIdentity)?,
        ),
        (_, Some(_)) => return Err(OutputFeatureRefusal::UnexpectedPhoneDefinition),
        (_, None) => None,
    };
    let layers = [
        (OutputFeatureLayer::DefaultRealization, default),
        (
            OutputFeatureLayer::PhoneDefinition,
            phone.map(SpeechPhone::features),
        ),
        (OutputFeatureLayer::RuleOutput, Some(rule.output_features())),
    ];
    for (layer, bundle) in layers {
        if let Some(bundle) = bundle {
            validate_feature_bundle(bundle)
                .map_err(|reason| OutputFeatureRefusal::Bundle { layer, reason })?;
        }
    }
    let mut features: [Option<InheritedOutputFeature<'a>>; 16] = core::array::from_fn(|_| None);
    let mut count = 0;
    // At most 48 supplied keys. Preserve first appearance order, replacing only
    // the borrowed value selected by the native layer law. No play-time storage.
    for (_, bundle) in layers {
        for key in bundle
            .into_iter()
            .flat_map(|bundle| bundle.get().as_slice())
        {
            if features[..count]
                .iter()
                .flatten()
                .any(|entry| entry.feature.identity() == key.identity())
            {
                continue;
            }
            let find = |bundle: Option<&'a SpeechFeatureBundle>| {
                bundle.and_then(|bundle| {
                    bundle
                        .get()
                        .as_slice()
                        .iter()
                        .find(|feature| feature.identity() == key.identity())
                })
            };
            let default_feature = find(default);
            let definition_feature = find(phone.map(SpeechPhone::features));
            let output_feature = find(Some(rule.output_features()));
            let selected =
                generated::speech_output_feature_layer(generated::SpeechOutputFeatureLayerInput {
                    phone: specification_state(rule.phone()),
                    default_present: default_feature.is_some(),
                    definition_present: definition_feature.is_some(),
                    output_present: output_feature.is_some(),
                })
                .ok_or(OutputFeatureRefusal::CompiledPlot)?;
            let (feature, layer) = match selected {
                generated::SpeechOutputFeatureLayer::absent => continue,
                generated::SpeechOutputFeatureLayer::default_realization => {
                    (default_feature, OutputFeatureLayer::DefaultRealization)
                }
                generated::SpeechOutputFeatureLayer::phone_definition => {
                    (definition_feature, OutputFeatureLayer::PhoneDefinition)
                }
                generated::SpeechOutputFeatureLayer::rule_output => {
                    (output_feature, OutputFeatureLayer::RuleOutput)
                }
            };
            let feature = feature.ok_or(OutputFeatureRefusal::CompiledPlot)?;
            if count == features.len() {
                return Err(OutputFeatureRefusal::Capacity);
            }
            features[count] = Some(InheritedOutputFeature { feature, layer });
            count += 1;
        }
    }
    Ok(RuleOutputFeatures {
        rule,
        default,
        phone,
        checked_phone,
        features,
        count,
    })
}
