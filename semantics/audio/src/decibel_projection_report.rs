//! Core fidelity report for exact supported dB transformations.
use crate::{AudioDecibelRatioReceipt, AudioRatioDecibelReceipt};
use conduit_core::projection::*;
mod sealed {
    pub trait Proof {}
}
pub trait AudioExactDecibelProof: sealed::Proof {
    type Source: PartialEq;
    type Target: PartialEq;
    fn source(&self) -> &Self::Source;
    fn target(&self) -> &Self::Target;
    fn original_frame(&self) -> &[u8];
    fn source_contract(&self) -> &'static str;
    fn target_contract(&self) -> &'static str;
}
macro_rules! proof {
    ($receipt:ty,$input:ty,$output:ty,$src:literal,$dst:literal) => {
        impl sealed::Proof for $receipt {}
        impl AudioExactDecibelProof for $receipt {
            type Source = $input;
            type Target = $output;
            fn source(&self) -> &Self::Source {
                self.original()
            }
            fn target(&self) -> &Self::Target {
                self.result()
            }
            fn original_frame(&self) -> &[u8] {
                self.original_canonical()
            }
            fn source_contract(&self) -> &'static str {
                $src
            }
            fn target_contract(&self) -> &'static str {
                $dst
            }
        }
    };
}
proof!(
    AudioRatioDecibelReceipt,
    crate::AudioReferencedLevelRatio,
    crate::AudioDecibelLevel,
    "audio/referenced-level-ratio@1",
    "audio/decibel-level@1"
);
proof!(
    AudioDecibelRatioReceipt,
    crate::AudioDecibelLevel,
    crate::AudioReferencedLevelRatio,
    "audio/decibel-level@1",
    "audio/referenced-level-ratio@1"
);
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).expect("bounded constant")
}
/// Only an existing immutable Source execution receipt can establish this proof.
pub struct AudioExactDecibelProjection<'a, R: AudioExactDecibelProof> {
    receipt: &'a R,
}
impl<'a, R: AudioExactDecibelProof> AudioExactDecibelProjection<'a, R> {
    pub fn new(receipt: &'a R) -> Self {
        Self { receipt }
    }
    pub fn facts(&self) -> [ProjectionFact<'_, Self>; 2] {
        [
            ProjectionFact::Preserved {
                obligation: text("reference-unit-role-provenance-convention"),
            },
            ProjectionFact::Transformed {
                obligation: text("level-value"),
                law: text("audio/source-exact-power-of-ten@1"),
            },
        ]
    }
    pub fn native(&self) -> [ProjectionNativeFact<'_>; 1] {
        [ProjectionNativeFact {
            identity: text("audio-original"),
            contract: text(self.receipt.source_contract()),
            provider: text("audio/checked-source@1"),
            encoding: text("structured-native@1"),
            bytes: self.receipt.original_frame(),
        }]
    }
}
impl<R: AudioExactDecibelProof> ProjectionDomain for AudioExactDecibelProjection<'_, R> {
    type Source = R::Source;
    type Target = R::Target;
    type Detail = ();
    fn source_contract(&self) -> ProjectionText<'_> {
        text(self.receipt.source_contract())
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text(self.receipt.target_contract())
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
        if source != self.receipt.source()
            || target != Some(self.receipt.target())
            || mechanism != ProjectionMechanism::Completed
            || !scores.is_empty()
            || facts.len() != 2
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
            && matches!(&facts[0],ProjectionFact::Preserved{obligation} if obligation.as_str()=="reference-unit-role-provenance-convention")
            && matches!(&facts[1],ProjectionFact::Transformed{obligation,law} if obligation.as_str()=="level-value" && law.as_str()=="audio/source-exact-power-of-ten@1")
    }
}
/// This capability authorizes no loss, including no logarithm approximation.
pub struct AudioExactDecibelPolicy;
impl<R: AudioExactDecibelProof> ProjectionPolicy<AudioExactDecibelProjection<'_, R>>
    for AudioExactDecibelPolicy
{
    fn identity(&self) -> ProjectionText<'_> {
        text("audio/exact-decibels-only@1")
    }
    fn permits(
        &self,
        _: &R::Source,
        _: &R::Target,
        _: &[ProjectionFact<'_, AudioExactDecibelProjection<'_, R>>],
    ) -> bool {
        false
    }
}
