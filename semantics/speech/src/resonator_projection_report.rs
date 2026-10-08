//! Core projection fidelity for the explicitly selected finite-series profile.
use crate::{
    semantic::{SpeechResonatorProjectionRequest, SpeechStableResonatorQ14},
    PreparedSpeechResonatorQ14,
};
use conduit_core::projection::*;
#[derive(Debug, PartialEq, Eq)]
pub struct SpeechResonatorPrecisionLoss {
    internal_fraction_bits: u8,
    polynomial_steps: u8,
    coefficient_fraction_bits: u8,
}
impl SpeechResonatorPrecisionLoss {
    pub fn internal_fraction_bits(&self) -> u8 {
        self.internal_fraction_bits
    }
    pub fn polynomial_steps(&self) -> u8 {
        self.polynomial_steps
    }
    pub fn coefficient_fraction_bits(&self) -> u8 {
        self.coefficient_fraction_bits
    }
}
pub struct SpeechResonatorProjection<'a> {
    receipt: &'a PreparedSpeechResonatorQ14,
    loss: SpeechResonatorPrecisionLoss,
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).expect("bounded contract")
}
impl<'a> SpeechResonatorProjection<'a> {
    pub fn new(receipt: &'a PreparedSpeechResonatorQ14) -> Self {
        Self {
            receipt,
            loss: SpeechResonatorPrecisionLoss {
                internal_fraction_bits: 20,
                polynomial_steps: 8,
                coefficient_fraction_bits: 14,
            },
        }
    }
    pub fn facts(&self) -> [ProjectionFact<'_, Self>; 3] {
        [
            ProjectionFact::Preserved {
                obligation: text(
                    "original-hz-fractions-independent-center-bandwidth-declared-rate",
                ),
            },
            ProjectionFact::Transformed {
                obligation: text("resonator-coefficients"),
                law: text("speech/q20-series8-q14-nearest@1"),
            },
            ProjectionFact::Lost {
                obligation: text("transcendental-and-coefficient-precision"),
                class: ProjectionLoss::Precision,
                detail: &self.loss,
                native_fact: Some(text("speech-resonator-original")),
            },
        ]
    }
    pub fn native(&self) -> [ProjectionNativeFact<'_>; 1] {
        [ProjectionNativeFact {
            identity: text("speech-resonator-original"),
            contract: text("speech/resonator-projection-request@1"),
            provider: text("speech/checked-source@1"),
            encoding: text("structured-native@1"),
            bytes: self.receipt.original_canonical(),
        }]
    }
}
impl ProjectionDomain for SpeechResonatorProjection<'_> {
    type Source = SpeechResonatorProjectionRequest;
    type Target = SpeechStableResonatorQ14;
    type Detail = SpeechResonatorPrecisionLoss;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("speech/resonator-projection-request@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("speech/stable-resonator-q14@1")
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
        if source != self.receipt.original()
            || target != Some(self.receipt.result())
            || mechanism != ProjectionMechanism::Completed
            || !scores.is_empty()
            || facts.len() != 3
            || native.len() != 1
        {
            return false;
        }
        let expected = self.native()[0];
        let actual = native[0];
        actual.identity == expected.identity
            && actual.contract == expected.contract
            && actual.provider == expected.provider
            && actual.encoding == expected.encoding
            && actual.bytes == expected.bytes
            && matches!(&facts[0],ProjectionFact::Preserved{obligation} if obligation.as_str()=="original-hz-fractions-independent-center-bandwidth-declared-rate")
            && matches!(&facts[1],ProjectionFact::Transformed{obligation,law} if obligation.as_str()=="resonator-coefficients"&&law.as_str()=="speech/q20-series8-q14-nearest@1")
            && matches!(&facts[2],ProjectionFact::Lost{obligation,class:ProjectionLoss::Precision,detail,native_fact:Some(fact)} if obligation.as_str()=="transcendental-and-coefficient-precision"&&**detail==self.loss&&fact.as_str()=="speech-resonator-original")
    }
}
/// Explicit acceptance of this finite-series and Q14 precision loss only. This
/// does not assert a general log/exp/trigonometry accuracy capability.
pub struct SpeechQ20Series8Q14Policy;
impl ProjectionPolicy<SpeechResonatorProjection<'_>> for SpeechQ20Series8Q14Policy {
    fn identity(&self) -> ProjectionText<'_> {
        text("speech/accept-q20-series8-q14@1")
    }
    fn permits(
        &self,
        _: &SpeechResonatorProjectionRequest,
        _: &SpeechStableResonatorQ14,
        facts: &[ProjectionFact<'_, SpeechResonatorProjection<'_>>],
    ) -> bool {
        facts.len() == 3
            && matches!(&facts[2],ProjectionFact::Lost{class:ProjectionLoss::Precision,detail,..} if detail.internal_fraction_bits==20&&detail.polynomial_steps==8&&detail.coefficient_fraction_bits==14)
    }
}
