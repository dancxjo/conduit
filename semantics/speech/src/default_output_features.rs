//! Borrowed default-phone inheritance: definition, then occurrence features.
use crate::{
    admission::validate_feature_bundle,
    generated,
    output_features::{InheritedOutputFeature, OutputFeatureLayer, OutputFeatureRefusal},
    semantic::*,
};
pub struct DefaultOutputFeatures<'a> {
    default: &'a SpeechFeatureBundle,
    definition: &'a SpeechPhone,
    features: [Option<InheritedOutputFeature<'a>>; 16],
    count: usize,
}
impl<'a> DefaultOutputFeatures<'a> {
    pub fn default_features(&self) -> &'a SpeechFeatureBundle {
        self.default
    }
    pub fn phone_definition(&self) -> &'a SpeechPhone {
        self.definition
    }
    pub fn features(&self) -> impl Iterator<Item = &InheritedOutputFeature<'a>> {
        self.features[..self.count].iter().flatten()
    }
}
/// Explicit supplied layers only. Membership and occurrence checks belong to
/// the enclosing default-phone admission; no phoneme features are inferred.
pub fn prepare_default_output_features<'a>(
    default: &'a SpeechFeatureBundle,
    definition: &'a SpeechPhone,
) -> Result<DefaultOutputFeatures<'a>, OutputFeatureRefusal> {
    let layers = [
        (OutputFeatureLayer::PhoneDefinition, definition.features()),
        (OutputFeatureLayer::DefaultRealization, default),
    ];
    for (layer, bundle) in layers {
        validate_feature_bundle(bundle)
            .map_err(|reason| OutputFeatureRefusal::Bundle { layer, reason })?;
    }
    let mut features: [Option<InheritedOutputFeature<'a>>; 16] = core::array::from_fn(|_| None);
    let mut count = 0;
    for (_, bundle) in layers {
        for key in bundle.get().as_slice() {
            if features[..count]
                .iter()
                .flatten()
                .any(|entry| entry.feature().identity() == key.identity())
            {
                continue;
            }
            let find = |bundle: &'a SpeechFeatureBundle| {
                bundle
                    .get()
                    .as_slice()
                    .iter()
                    .find(|feature| feature.identity() == key.identity())
            };
            let default_feature = find(default);
            let definition_feature = find(definition.features());
            let selected = generated::speech_default_feature_layer(
                generated::SpeechDefaultFeatureLayerInput {
                    default_present: default_feature.is_some(),
                    definition_present: definition_feature.is_some(),
                },
            )
            .ok_or(OutputFeatureRefusal::CompiledPlot)?;
            let (feature, layer) = match selected {
                generated::SpeechOutputFeatureLayer::default_realization => {
                    (default_feature, OutputFeatureLayer::DefaultRealization)
                }
                generated::SpeechOutputFeatureLayer::phone_definition => {
                    (definition_feature, OutputFeatureLayer::PhoneDefinition)
                }
                _ => return Err(OutputFeatureRefusal::CompiledPlot),
            };
            if count == features.len() {
                return Err(OutputFeatureRefusal::Capacity);
            }
            features[count] = Some(InheritedOutputFeature::new(
                feature.ok_or(OutputFeatureRefusal::CompiledPlot)?,
                layer,
            ));
            count += 1;
        }
    }
    Ok(DefaultOutputFeatures {
        default,
        definition,
        features,
        count,
    })
}
