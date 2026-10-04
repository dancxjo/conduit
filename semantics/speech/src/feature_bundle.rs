//! Exact-key feature matching against an explicit observed bundle. Definition
//! inheritance, occurrence binding and rule eligibility remain separate.
use crate::{
    admission::{validate_feature_bundle, LocalSemanticRefusal},
    feature_match::{compare_feature_observation, FeatureComparisonRefusal},
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum FeatureBundleSide {
    Requirement,
    Observation,
}
#[derive(Debug)]
pub enum FeatureBundleRefusal {
    Bundle {
        side: FeatureBundleSide,
        reason: LocalSemanticRefusal,
    },
    Key {
        index: usize,
        reason: NativeBindingRefusal,
    },
    Comparison {
        index: usize,
        reason: FeatureComparisonRefusal,
    },
}
pub struct FeatureReceipt<'a> {
    requirement: &'a SpeechFeature,
    observation: Option<&'a SpeechFeature>,
    checked_key: Option<SpeechFeatureIdentityMatch>,
    decision: SpeechContextDecision,
}
impl<'a> FeatureReceipt<'a> {
    pub fn requirement(&self) -> &'a SpeechFeature {
        self.requirement
    }
    pub fn observation(&self) -> Option<&'a SpeechFeature> {
        self.observation
    }
    pub fn checked_key(&self) -> Option<&SpeechFeatureIdentityMatch> {
        self.checked_key.as_ref()
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
pub struct FeatureBundleComparison<'a> {
    requirements: &'a SpeechFeatureBundle,
    observations: &'a SpeechFeatureBundle,
    receipts: [Option<FeatureReceipt<'a>>; 16],
    count: usize,
}
impl<'a> FeatureBundleComparison<'a> {
    pub fn requirements(&self) -> &'a SpeechFeatureBundle {
        self.requirements
    }
    pub fn observations(&self) -> &'a SpeechFeatureBundle {
        self.observations
    }
    pub fn comparisons(&self) -> impl Iterator<Item = &FeatureReceipt<'a>> {
        self.receipts[..self.count]
            .iter()
            .filter_map(Option::as_ref)
    }
}
pub fn compare_feature_bundle<'a>(
    requirements: &'a SpeechFeatureBundle,
    observations: &'a SpeechFeatureBundle,
) -> Result<FeatureBundleComparison<'a>, FeatureBundleRefusal> {
    validate_feature_bundle(requirements).map_err(|reason| FeatureBundleRefusal::Bundle {
        side: FeatureBundleSide::Requirement,
        reason,
    })?;
    validate_feature_bundle(observations).map_err(|reason| FeatureBundleRefusal::Bundle {
        side: FeatureBundleSide::Observation,
        reason,
    })?;
    let mut receipts = core::array::from_fn(|_| None);
    for (index, requirement) in requirements.get().as_slice().iter().enumerate() {
        let observation = observations
            .get()
            .as_slice()
            .iter()
            .find(|feature| feature.identity() == requirement.identity());
        let checked_key = observation
            .map(|feature| {
                SpeechFeatureIdentityMatch::new(
                    feature.identity().clone(),
                    requirement.identity().clone(),
                )
            })
            .transpose()
            .map_err(|reason| FeatureBundleRefusal::Key { index, reason })?;
        let decision = compare_feature_observation(
            requirement.specification(),
            observation.map(SpeechFeature::specification),
        )
        .map_err(|reason| FeatureBundleRefusal::Comparison { index, reason })?;
        receipts[index] = Some(FeatureReceipt {
            requirement,
            observation,
            checked_key,
            decision,
        });
    }
    Ok(FeatureBundleComparison {
        requirements,
        observations,
        receipts,
        count: requirements.get().as_slice().len(),
    })
}
