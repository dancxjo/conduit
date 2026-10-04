//! Original standalone allophone input patterns. Context, output realization,
//! inheritance, occurrence binding, rule policy and selection remain separate.
use crate::{
    feature_bundle::{
        compare_feature_bundle_observation, FeatureBundleComparison, FeatureBundleRefusal,
    },
    generated,
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum IdentityPatternRefusal {
    Native(NativeBindingRefusal),
    CompiledPlot,
}
pub struct IdentityPatternComparison<'a, Specification, Identity> {
    requirement: &'a Specification,
    observation: &'a Specification,
    identity: Option<Result<Identity, NativeBindingRefusal>>,
    decision: SpeechContextDecision,
}
impl<'a, Specification, Identity> IdentityPatternComparison<'a, Specification, Identity> {
    pub fn requirement(&self) -> &'a Specification {
        self.requirement
    }
    pub fn observation(&self) -> &'a Specification {
        self.observation
    }
    pub fn identity(&self) -> Option<&Result<Identity, NativeBindingRefusal>> {
        self.identity.as_ref()
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
fn retained(value: generated::SpeechContextDecision) -> SpeechContextDecision {
    match value {
        generated::SpeechContextDecision::matched => SpeechContextDecision::Matched,
        generated::SpeechContextDecision::mismatched => SpeechContextDecision::Mismatched,
        generated::SpeechContextDecision::requirement_unresolved => {
            SpeechContextDecision::RequirementUnresolved
        }
        generated::SpeechContextDecision::observation_unresolved => {
            SpeechContextDecision::ObservationUnresolved
        }
    }
}
fn projected(value: &SpeechContextDecision) -> generated::SpeechContextDecision {
    match value {
        SpeechContextDecision::Matched => generated::SpeechContextDecision::matched,
        SpeechContextDecision::Mismatched => generated::SpeechContextDecision::mismatched,
        SpeechContextDecision::RequirementUnresolved => {
            generated::SpeechContextDecision::requirement_unresolved
        }
        SpeechContextDecision::ObservationUnresolved => {
            generated::SpeechContextDecision::observation_unresolved
        }
    }
}
macro_rules! pattern_comparison {
    ($function:ident, $specification:ident, $identity:ident) => {
        pub fn $function<'a>(requirement: &'a $specification, observation: &'a $specification)
            -> Result<IdentityPatternComparison<'a, $specification, $identity>, IdentityPatternRefusal> {
            use generated::SpeechSpecificationState as S;
            let state=|value: &$specification| match value {
                $specification::Known(_) => S::known,
                $specification::Unknown => S::unknown,
                $specification::Unspecified => S::unspecified,
                $specification::NotApplicable => S::not_applicable,
                $specification::Variable(_) => S::variable,
                $specification::Gradient(_) => S::gradient,
            };
            let identity=match (requirement,observation) {
                ($specification::Known(required),$specification::Known(observed)) =>
                    Some($identity::new(observed.clone(),required.clone())),
                _=>None,
            };
            let identical=match &identity {
                Some(Ok(_))=>true,
                Some(Err(NativeBindingRefusal::ViolatedInvariant { index:0 }))=>false,
                Some(Err(reason))=>return Err(IdentityPatternRefusal::Native(reason.clone())),
                None=>false, // Ignored by the native state law; not an observation.
            };
            let decision=generated::speech_identity_pattern_compare(generated::SpeechIdentityPatternComparisonInput {
                requirement:state(requirement),observation:state(observation),identical,
            }).ok_or(IdentityPatternRefusal::CompiledPlot)?;
            Ok(IdentityPatternComparison { requirement,observation,identity,decision:retained(decision) })
        }
    }
}
pattern_comparison!(
    compare_phoneme_pattern,
    PhonemeSpecification,
    SpeechPhonemePatternIdentity
);
pattern_comparison!(
    compare_phone_pattern,
    PhoneSpecification,
    SpeechPhonePatternIdentity
);

#[derive(Debug)]
pub enum RuleInputRefusal {
    Phoneme(IdentityPatternRefusal),
    Features(FeatureBundleRefusal),
    CompiledPlot,
}
pub struct AllophoneRuleInputComparison<'a> {
    rule: &'a SpeechAllophoneRule,
    phoneme: IdentityPatternComparison<'a, PhonemeSpecification, SpeechPhonemePatternIdentity>,
    features: FeatureBundleComparison<'a>,
    decision: SpeechContextDecision,
}
impl<'a> AllophoneRuleInputComparison<'a> {
    pub fn rule(&self) -> &'a SpeechAllophoneRule {
        self.rule
    }
    pub fn phoneme(
        &self,
    ) -> &IdentityPatternComparison<'a, PhonemeSpecification, SpeechPhonemePatternIdentity> {
        &self.phoneme
    }
    pub fn features(&self) -> &FeatureBundleComparison<'a> {
        &self.features
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
/// Explicit input evidence only. No definition features or variant/base IDs are
/// substituted. Both successful components survive even when one mismatches.
pub fn compare_allophone_rule_input<'a>(
    rule: &'a SpeechAllophoneRule,
    phoneme: &'a PhonemeSpecification,
    features: Option<&'a SpeechFeatureBundle>,
) -> Result<AllophoneRuleInputComparison<'a>, RuleInputRefusal> {
    let phoneme =
        compare_phoneme_pattern(rule.phoneme(), phoneme).map_err(RuleInputRefusal::Phoneme)?;
    let features = compare_feature_bundle_observation(rule.input_features(), features)
        .map_err(RuleInputRefusal::Features)?;
    let decision = generated::speech_context_conjunction(generated::SpeechContextConjunction {
        left: projected(phoneme.decision()),
        right: projected(features.decision()),
    })
    .ok_or(RuleInputRefusal::CompiledPlot)?;
    Ok(AllophoneRuleInputComparison {
        rule,
        phoneme,
        features,
        decision: retained(decision),
    })
}
