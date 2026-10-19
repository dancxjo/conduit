//! Fidelity of finite point values, not of their retained continuous originals.
//! Reports borrow the complete immutable receipt as their typed source. Native
//! fact bytes are deliberately absent: no 4096-byte carrier or digest substitute
//! is asserted. Original canonical bytes remain accessible through `source()`.
use crate::{AudioControlProjectionReceipt, AudioTrajectoryQuantity};
use alloc::vec::Vec;
use conduit_core::projection::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug, PartialEq)]
pub struct AudioControlPointTarget {
    query_canonical: Vec<u8>,
    values: Vec<AudioTrajectoryQuantity>,
    value_canonical: Vec<Vec<u8>>,
    selected_indices: Vec<usize>,
    clock_binding_canonical: Vec<u8>,
}
impl AudioControlPointTarget {
    pub fn query_canonical(&self) -> &[u8] {
        &self.query_canonical
    }
    pub fn values(&self) -> &[AudioTrajectoryQuantity] {
        &self.values
    }
    pub fn value_canonical(&self) -> &[Vec<u8>] {
        &self.value_canonical
    }
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }
    pub fn clock_binding_canonical(&self) -> &[u8] {
        &self.clock_binding_canonical
    }
}
#[derive(Debug, PartialEq)]
pub struct AudioControlPointCollectionTarget {
    schedule_canonical: Vec<u8>,
    points: Vec<AudioControlPointTarget>,
}
impl AudioControlPointCollectionTarget {
    pub fn schedule_canonical(&self) -> &[u8] {
        &self.schedule_canonical
    }
    pub fn points(&self) -> &[AudioControlPointTarget] {
        &self.points
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum AudioControlPointLoss {
    /// Original scoped trajectory identities, provenance, segments and policies
    /// remain in the source; this target contains evaluated point values only.
    OriginalTrajectoryContract,
    /// No values or coverage are promised between queries or outside the finite
    /// query set. This is not a sample-and-hold reconstruction.
    BetweenAndOutsideQueries,
}
pub struct AudioControlPointProjection<'a> {
    receipt: &'a AudioControlProjectionReceipt,
    target: AudioControlPointCollectionTarget,
}
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("bounded contract")
}
impl<'a> AudioControlPointProjection<'a> {
    pub fn new(receipt: &'a AudioControlProjectionReceipt) -> Result<Self, NativeBindingRefusal> {
        let mut points = Vec::with_capacity(16);
        for point in receipt.points() {
            let mut values = Vec::with_capacity(8);
            let mut value_canonical = Vec::with_capacity(8);
            for evaluation in point.control().evaluations() {
                let value = evaluation.result().clone();
                value_canonical.push(value.clone().encode()?);
                values.push(value);
            }
            points.push(AudioControlPointTarget {
                query_canonical: point.query_canonical().into(),
                values,
                value_canonical,
                selected_indices: point.control().selected_indices().into(),
                clock_binding_canonical: point.clock().binding_canonical().into(),
            });
        }
        Ok(Self {
            receipt,
            target: AudioControlPointCollectionTarget {
                schedule_canonical: receipt.schedule_canonical().into(),
                points,
            },
        })
    }
    pub fn source(&self) -> &AudioControlProjectionReceipt {
        self.receipt
    }
    pub fn target(&self) -> &AudioControlPointCollectionTarget {
        &self.target
    }
    pub fn facts(&self) -> [ProjectionFact<'_, Self>; 6] {
        [
            ProjectionFact::Preserved {
                obligation: text("exact-schedule-and-declared-clock-binding"),
            },
            ProjectionFact::Transformed {
                obligation: text("exact-query-times"),
                law: text("audio/source-control-query-time@1"),
            },
            ProjectionFact::Transformed {
                obligation: text("ordered-exact-point-values"),
                law: text("audio/source-trajectory-evaluation@1"),
            },
            ProjectionFact::Preserved {
                obligation: text("point-control-selection-indices"),
            },
            ProjectionFact::Lost {
                obligation: text("original-scoped-trajectory-contract"),
                class: ProjectionLoss::Truncation,
                detail: &AudioControlPointLoss::OriginalTrajectoryContract,
                native_fact: None,
            },
            ProjectionFact::Lost {
                obligation: text("between-and-outside-query-behavior"),
                class: ProjectionLoss::Truncation,
                detail: &AudioControlPointLoss::BetweenAndOutsideQueries,
                native_fact: None,
            },
        ]
    }
}
impl ProjectionDomain for AudioControlPointProjection<'_> {
    type Source = AudioControlProjectionReceipt;
    type Target = AudioControlPointCollectionTarget;
    type Detail = AudioControlPointLoss;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("audio/full-control-projection-receipt@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("audio/point-values-only@1")
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
        if !core::ptr::eq(source, self.receipt)
            || target != Some(&self.target)
            || !native.is_empty()
            || !scores.is_empty()
            || mechanism != ProjectionMechanism::Completed
        {
            return false;
        }
        same_facts(facts, &self.facts())
    }
}
fn same_facts<D: ProjectionDomain>(
    actual: &[ProjectionFact<'_, D>],
    expected: &[ProjectionFact<'_, D>],
) -> bool
where
    D::Detail: PartialEq,
{
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(a, b)| match (a, b) {
            (
                ProjectionFact::Preserved { obligation: a },
                ProjectionFact::Preserved { obligation: b },
            ) => a == b,
            (
                ProjectionFact::Transformed {
                    obligation: a,
                    law: al,
                },
                ProjectionFact::Transformed {
                    obligation: b,
                    law: bl,
                },
            ) => a == b && al == bl,
            (
                ProjectionFact::Lost {
                    obligation: a,
                    class: ac,
                    detail: ad,
                    native_fact: an,
                },
                ProjectionFact::Lost {
                    obligation: b,
                    class: bc,
                    detail: bd,
                    native_fact: bn,
                },
            ) => a == b && ac == bc && ad == bd && an == bn,
            _ => false,
        })
}
/// Explicitly consumes point samples without authorizing any reconstruction.
pub struct AudioPointSamplesOnlyPolicy;
impl ProjectionPolicy<AudioControlPointProjection<'_>> for AudioPointSamplesOnlyPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("audio/point-samples-only-policy@1")
    }
    fn permits(
        &self,
        _: &AudioControlProjectionReceipt,
        _: &AudioControlPointCollectionTarget,
        facts: &[ProjectionFact<'_, AudioControlPointProjection<'_>>],
    ) -> bool {
        facts.len() == 6
            && matches!(
                facts[4],
                ProjectionFact::Lost {
                    class: ProjectionLoss::Truncation,
                    detail: AudioControlPointLoss::OriginalTrajectoryContract,
                    ..
                }
            )
            && matches!(
                facts[5],
                ProjectionFact::Lost {
                    class: ProjectionLoss::Truncation,
                    detail: AudioControlPointLoss::BetweenAndOutsideQueries,
                    ..
                }
            )
    }
}

/// Integer clock ticks retain their declared identity and exact query, but a
/// fractional tick remains a precision loss even alongside the full source.
#[derive(Debug, PartialEq, Eq)]
pub struct AudioIntegerClockTarget {
    binding_canonical: Vec<u8>,
    query_canonical: Vec<u8>,
    whole_ticks: i128,
}
impl AudioIntegerClockTarget {
    pub fn binding_canonical(&self) -> &[u8] {
        &self.binding_canonical
    }
    pub fn query_canonical(&self) -> &[u8] {
        &self.query_canonical
    }
    pub fn whole_ticks(&self) -> i128 {
        self.whole_ticks
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct AudioClockTickPrecisionLoss {
    remainder_numerator: i128,
    remainder_denominator: i128,
}
impl AudioClockTickPrecisionLoss {
    pub fn remainder_numerator(&self) -> i128 {
        self.remainder_numerator
    }
    pub fn remainder_denominator(&self) -> i128 {
        self.remainder_denominator
    }
}
pub struct AudioIntegerClockProjection<'a> {
    receipt: &'a crate::AudioClockProjectionReceipt,
    target: AudioIntegerClockTarget,
    loss: AudioClockTickPrecisionLoss,
}
impl<'a> AudioIntegerClockProjection<'a> {
    pub fn new(receipt: &'a crate::AudioClockProjectionReceipt) -> Self {
        let raw = receipt.result().raw();
        Self {
            receipt,
            target: AudioIntegerClockTarget {
                binding_canonical: receipt.binding_canonical().into(),
                query_canonical: receipt.query_canonical().into(),
                whole_ticks: *raw.whole_ticks(),
            },
            loss: AudioClockTickPrecisionLoss {
                remainder_numerator: *raw.remainder_numerator(),
                remainder_denominator: *raw.remainder_denominator(),
            },
        }
    }
    pub fn source(&self) -> &crate::AudioClockProjectionReceipt {
        self.receipt
    }
    pub fn target(&self) -> &AudioIntegerClockTarget {
        &self.target
    }
    pub fn facts(&self) -> [ProjectionFact<'_, Self>; 3] {
        [
            ProjectionFact::Preserved {
                obligation: text("full-declared-clock-binding"),
            },
            ProjectionFact::Preserved {
                obligation: text("exact-original-query"),
            },
            if self.loss.remainder_numerator == 0 {
                ProjectionFact::Transformed {
                    obligation: text("integer-tick-precision"),
                    law: text("audio/source-declared-affine-clock@1"),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text("integer-tick-precision"),
                    class: ProjectionLoss::Precision,
                    detail: &self.loss,
                    native_fact: None,
                }
            },
        ]
    }
}
impl ProjectionDomain for AudioIntegerClockProjection<'_> {
    type Source = crate::AudioClockProjectionReceipt;
    type Target = AudioIntegerClockTarget;
    type Detail = AudioClockTickPrecisionLoss;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("audio/exact-affine-clock-receipt@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("audio/integer-clock-ticks@1")
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
        core::ptr::eq(source, self.receipt)
            && target == Some(&self.target)
            && native.is_empty()
            && scores.is_empty()
            && mechanism == ProjectionMechanism::Completed
            && same_facts(facts, &self.facts())
    }
}
pub struct AudioFloorClockTickPolicy;
impl ProjectionPolicy<AudioIntegerClockProjection<'_>> for AudioFloorClockTickPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("audio/explicit-floor-clock-tick@1")
    }
    fn permits(
        &self,
        _: &crate::AudioClockProjectionReceipt,
        _: &AudioIntegerClockTarget,
        facts: &[ProjectionFact<'_, AudioIntegerClockProjection<'_>>],
    ) -> bool {
        facts.len() == 3
            && matches!(&facts[2],ProjectionFact::Lost{class:ProjectionLoss::Precision,detail,..}
            if detail.remainder_numerator>0&&detail.remainder_numerator<detail.remainder_denominator)
    }
}
