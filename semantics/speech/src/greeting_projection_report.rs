//! Actual Core report view; all facts borrow retained original/effect receipts.
use crate::{semantic::*, PreparedGreetingPhoneGestures};
use alloc::vec::Vec;
use conduit_core::projection::*;
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).expect("bounded contract")
}
pub struct GreetingApproximationProjection<'a> {
    receipt: &'a PreparedGreetingPhoneGestures,
    facts: Vec<ProjectionFact<'a, Self>>,
    native: Vec<ProjectionNativeFact<'a>>,
    policy: GreetingApproximationProjectionPolicy,
}
pub struct GreetingApproximationProjectionPolicy {
    selected: SpeechGreetingLossPolicy,
}
impl<'a> GreetingApproximationProjection<'a> {
    pub fn new(receipt: &'a PreparedGreetingPhoneGestures) -> Self {
        let mut facts = alloc::vec![
            ProjectionFact::Preserved {
                obligation: text("original-ipa-features")
            },
            ProjectionFact::Transformed {
                obligation: text("authored-acoustic-targets"),
                law: text(receipt.lowered().profile_identity())
            }
        ];
        if !matches!(
            receipt.effect(),
            SpeechGreetingApproximationEffect::OrdinaryThreePoleProfile
        ) {
            facts.push(ProjectionFact::Lost {
                obligation: text("declared-phonetic-mechanism"),
                class: ProjectionLoss::Approximation,
                detail: receipt.effect(),
                native_fact: Some(text("original-symbol")),
            })
        }
        let mut native = alloc::vec![
            ProjectionNativeFact {
                identity: text("original-symbol"),
                contract: text("speech/gesture-symbol@1"),
                provider: text("speech/original-native@1"),
                encoding: text("structured-native@1"),
                bytes: receipt.original_symbol_canonical()
            },
            ProjectionNativeFact {
                identity: text("selected-policy"),
                contract: text("speech/greeting-loss-policy@2"),
                provider: text("speech/original-native@2"),
                encoding: text("structured-native@1"),
                bytes: receipt.selected_policy_canonical()
            },
            ProjectionNativeFact {
                identity: text("declared-effect"),
                contract: text("speech/greeting-approximation-effect@2"),
                provider: text("speech/checked-source@2"),
                encoding: text("structured-native@1"),
                bytes: receipt.effect_canonical()
            },
        ];
        if let Some(bytes) = receipt.original_policy_request_canonical() {
            native.push(ProjectionNativeFact {
                identity: text("selected-loss-policy"),
                contract: text("speech/greeting-policy-request@2"),
                provider: text("speech/checked-source@2"),
                encoding: text("structured-native@1"),
                bytes,
            })
        }
        Self {
            receipt,
            facts,
            native,
            policy: GreetingApproximationProjectionPolicy {
                selected: *receipt.selected_policy(),
            },
        }
    }
    pub fn report(&self) -> Result<ProjectionReport<'_, Self>, ProjectionRefusal> {
        ProjectionReport::new(
            self,
            &self.policy,
            ProjectionInput {
                basis: ProjectionBasis {
                    report: text("speech/authored-greeting-fidelity@2"),
                    source: text("original-phone"),
                    target: Some(text("declared-profile-effect")),
                    projector: text("speech/checked-source@2"),
                    requested_route: text("caller-selected-authored-approximation"),
                    boundary: None,
                },
                source: self.receipt.lowered().declared_phone(),
                target: Some(self.receipt.effect()),
                facts: &self.facts,
                native: &self.native,
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
    }
}
impl ProjectionDomain for GreetingApproximationProjection<'_> {
    type Source = SpeechPhone;
    type Target = SpeechGreetingApproximationEffect;
    type Detail = SpeechGreetingApproximationEffect;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("speech/phone@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("speech/greeting-approximation-effect@2")
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        if source != self.receipt.lowered().declared_phone()
            || target != Some(self.receipt.effect())
            || mechanism != ProjectionMechanism::Completed
            || !scores.is_empty()
            || facts.len() != self.facts.len()
            || native.len() != self.native.len()
        {
            return false;
        }
        if !native.iter().zip(&self.native).all(|(a, b)| {
            a.identity == b.identity
                && a.contract == b.contract
                && a.provider == b.provider
                && a.encoding == b.encoding
                && a.bytes == b.bytes
        }) {
            return false;
        }
        matches!(&facts[0],ProjectionFact::Preserved{obligation} if obligation.as_str()=="original-ipa-features")
            && matches!(&facts[1],ProjectionFact::Transformed{obligation,law} if obligation.as_str()=="authored-acoustic-targets"&&law.as_str()==self.receipt.lowered().profile_identity())
            && (facts.len() == 2
                || matches!(&facts[2],ProjectionFact::Lost{obligation,class:ProjectionLoss::Approximation,detail,native_fact:Some(native)} if obligation.as_str()=="declared-phonetic-mechanism"&&*detail==self.receipt.effect()&&native.as_str()=="original-symbol"))
    }
}
impl ProjectionPolicy<GreetingApproximationProjection<'_>>
    for GreetingApproximationProjectionPolicy
{
    fn identity(&self) -> ProjectionText<'_> {
        match self.selected {
            SpeechGreetingLossPolicy::RefuseUnsupportedMechanisms => {
                text("speech/refuse-unsupported-mechanisms@2")
            }
            SpeechGreetingLossPolicy::AcceptLateralRhoticAndStepDiphthongApproximationV2 => {
                text("speech/accept-lateral-rhotic-step-diphthong@2")
            }
        }
    }
    fn permits(
        &self,
        _: &SpeechPhone,
        target: &SpeechGreetingApproximationEffect,
        facts: &[ProjectionFact<'_, GreetingApproximationProjection<'_>>],
    ) -> bool {
        matches!(
            self.selected,
            SpeechGreetingLossPolicy::AcceptLateralRhoticAndStepDiphthongApproximationV2
        ) && facts.len() == 3
            && !matches!(
                target,
                SpeechGreetingApproximationEffect::OrdinaryThreePoleProfile
            )
    }
}
