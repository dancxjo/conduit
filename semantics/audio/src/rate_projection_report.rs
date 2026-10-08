//! #4952 reports for the declared integer frame grid, not its retained source.
use crate::{
    AudioCumulativeFrameRequest, AudioCumulativeProjectionReceipt, AudioFrameGridFidelity,
    AudioIntegerFrameTarget, AudioSampleProjectionReceipt, AudioSampleProjectionRequest,
};
use conduit_core::projection::*;

mod sealed {
    pub trait Proof {}
}
/// Implemented only by immutable, Source-executed Audio projection receipts.
pub trait AudioFrameProjectionProof: sealed::Proof {
    type Source: PartialEq;
    fn source(&self) -> &Self::Source;
    fn source_canonical(&self) -> &[u8];
    fn target(&self) -> &AudioIntegerFrameTarget;
    fn fidelity(&self) -> AudioFrameGridFidelity;
    fn remainder(&self) -> u64;
    fn denominator(&self) -> u64;
    fn contract(&self) -> &'static str;
    fn law(&self) -> &'static str;
}
impl sealed::Proof for AudioSampleProjectionReceipt {}
impl AudioFrameProjectionProof for AudioSampleProjectionReceipt {
    type Source = AudioSampleProjectionRequest;
    fn source(&self) -> &Self::Source {
        self.original()
    }
    fn source_canonical(&self) -> &[u8] {
        self.original_canonical()
    }
    fn target(&self) -> &AudioIntegerFrameTarget {
        self.integer_target()
    }
    fn fidelity(&self) -> AudioFrameGridFidelity {
        *self.result().fidelity()
    }
    fn remainder(&self) -> u64 {
        *self.result().raw().remainder_numerator()
    }
    fn denominator(&self) -> u64 {
        *self.result().fraction().denominator()
    }
    fn contract(&self) -> &'static str {
        "audio/sample-projection-request@1"
    }
    fn law(&self) -> &'static str {
        "audio/source-floor-at-rate@1"
    }
}
impl sealed::Proof for AudioCumulativeProjectionReceipt {}
impl AudioFrameProjectionProof for AudioCumulativeProjectionReceipt {
    type Source = AudioCumulativeFrameRequest;
    fn source(&self) -> &Self::Source {
        self.original()
    }
    fn source_canonical(&self) -> &[u8] {
        self.original_canonical()
    }
    fn target(&self) -> &AudioIntegerFrameTarget {
        self.integer_target()
    }
    fn fidelity(&self) -> AudioFrameGridFidelity {
        *self.result().fidelity()
    }
    fn remainder(&self) -> u64 {
        *self.result().raw().remainder_numerator()
    }
    fn denominator(&self) -> u64 {
        *self.result().fraction().denominator()
    }
    fn contract(&self) -> &'static str {
        "audio/cumulative-frame-request@1"
    }
    fn law(&self) -> &'static str {
        "audio/source-cumulative-floor@1"
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AudioFramePrecisionLoss {
    remainder_numerator: u64,
    denominator: u64,
}
impl AudioFramePrecisionLoss {
    pub fn remainder_numerator(&self) -> u64 {
        self.remainder_numerator
    }
    pub fn denominator(&self) -> u64 {
        self.denominator
    }
}
/// Borrows exact executed proof; cannot manufacture a numerical projection.
pub struct AudioFrameGridProjection<'a, R: AudioFrameProjectionProof> {
    receipt: &'a R,
    loss: AudioFramePrecisionLoss,
}
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("bounded constant contract")
}
impl<'a, R: AudioFrameProjectionProof> AudioFrameGridProjection<'a, R> {
    pub fn new(receipt: &'a R) -> Self {
        Self {
            receipt,
            loss: AudioFramePrecisionLoss {
                remainder_numerator: receipt.remainder(),
                denominator: receipt.denominator(),
            },
        }
    }
    pub fn facts(&self) -> [ProjectionFact<'_, Self>; 4] {
        [
            ProjectionFact::Preserved {
                obligation: text("anchor"),
            },
            ProjectionFact::Preserved {
                obligation: text("declared-rate"),
            },
            ProjectionFact::Preserved {
                obligation: text("floor-policy"),
            },
            match self.receipt.fidelity() {
                AudioFrameGridFidelity::Exact => ProjectionFact::Transformed {
                    obligation: text("frame-grid-precision"),
                    law: text(self.receipt.law()),
                },
                AudioFrameGridFidelity::FloorWithRemainder => ProjectionFact::Lost {
                    obligation: text("frame-grid-precision"),
                    class: ProjectionLoss::Precision,
                    detail: &self.loss,
                    native_fact: Some(text("audio-original")),
                },
            },
        ]
    }
    pub fn native(&self) -> [ProjectionNativeFact<'_>; 1] {
        [ProjectionNativeFact {
            identity: text("audio-original"),
            contract: text(self.receipt.contract()),
            provider: text("audio/checked-source@1"),
            encoding: text("structured-native@1"),
            bytes: self.receipt.source_canonical(),
        }]
    }
}
impl<R: AudioFrameProjectionProof> ProjectionDomain for AudioFrameGridProjection<'_, R> {
    type Source = R::Source;
    type Target = AudioIntegerFrameTarget;
    type Detail = AudioFramePrecisionLoss;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(self.receipt.contract())
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("audio/integer-frame-target@1")
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
            || facts.len() != 4
            || native.len() != 1
        {
            return false;
        }
        let expected = self.native()[0];
        let actual = native[0];
        if actual.identity != expected.identity
            || actual.contract != expected.contract
            || actual.provider != expected.provider
            || actual.encoding != expected.encoding
            || actual.bytes != expected.bytes
        {
            return false;
        }
        for (fact, name) in facts[..3]
            .iter()
            .zip(["anchor", "declared-rate", "floor-policy"])
        {
            if !matches!(fact,ProjectionFact::Preserved {obligation} if obligation.as_str()==name) {
                return false;
            }
        }
        match (&facts[3], self.receipt.fidelity()) {
            (ProjectionFact::Transformed { obligation, law }, AudioFrameGridFidelity::Exact) => {
                obligation.as_str() == "frame-grid-precision" && law.as_str() == self.receipt.law()
            }
            (
                ProjectionFact::Lost {
                    obligation,
                    class: ProjectionLoss::Precision,
                    detail,
                    native_fact: Some(id),
                },
                AudioFrameGridFidelity::FloorWithRemainder,
            ) => {
                obligation.as_str() == "frame-grid-precision"
                    && **detail == self.loss
                    && id.as_str() == "audio-original"
            }
            _ => false,
        }
    }
}
/// Authorizes only the known retained fractional-frame endpoint precision loss.
pub struct AudioFloorFrameGridPolicy;
impl<R: AudioFrameProjectionProof> ProjectionPolicy<AudioFrameGridProjection<'_, R>>
    for AudioFloorFrameGridPolicy
{
    fn identity(&self) -> ProjectionText<'_> {
        text("audio/explicit-floor-frame-grid@1")
    }
    fn permits(
        &self,
        _: &R::Source,
        _: &AudioIntegerFrameTarget,
        facts: &[ProjectionFact<'_, AudioFrameGridProjection<'_, R>>],
    ) -> bool {
        facts.len() == 4
            && matches!(&facts[3],ProjectionFact::Lost {class:ProjectionLoss::Precision,detail,..} if detail.remainder_numerator>0 && detail.remainder_numerator<detail.denominator)
    }
}
