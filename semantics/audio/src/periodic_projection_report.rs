//! Fidelity of the existing initial periodic numeric profile, not PCM playback.
use crate::{AudioPeriodicCursor, AudioPeriodicRequest};
use conduit_core::projection::*;

#[derive(Debug, PartialEq)]
pub struct AudioPeriodicNumericTarget {
    request: AudioPeriodicRequest,
    period_q8: i128,
    amplitude_q15: i128,
    initial_phase_q8: i128,
}
impl AudioPeriodicNumericTarget {
    pub fn request(&self) -> &AudioPeriodicRequest {
        &self.request
    }
    pub fn period_q8(&self) -> i128 {
        self.period_q8
    }
    pub fn amplitude_q15(&self) -> i128 {
        self.amplitude_q15
    }
    pub fn initial_phase_q8(&self) -> i128 {
        self.initial_phase_q8
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct AudioPeriodicNumericLoss {
    remainder: i128,
    denominator: i128,
}
impl AudioPeriodicNumericLoss {
    pub fn remainder(&self) -> i128 {
        self.remainder
    }
    pub fn denominator(&self) -> i128 {
        self.denominator
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum PeriodicNumericReportRefusal {
    AdvancedCursor,
}
/// Accepts only the initial, already owner-admitted cursor. This reports checked
/// numerical laws, not authenticity of an execution history or installed Play.
pub struct AudioPeriodicNumericProjection<'a> {
    cursor: &'a AudioPeriodicCursor,
    target: AudioPeriodicNumericTarget,
    losses: [AudioPeriodicNumericLoss; 3],
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).expect("bounded contract")
}
impl<'a> AudioPeriodicNumericProjection<'a> {
    pub fn new(cursor: &'a AudioPeriodicCursor) -> Result<Self, PeriodicNumericReportRefusal> {
        let p = cursor.profile().projection();
        let r = p.request();
        if *cursor.emitted() != 0 || cursor.remaining() != r.frames() {
            return Err(PeriodicNumericReportRefusal::AdvancedCursor);
        }
        Ok(Self {
            cursor,
            target: AudioPeriodicNumericTarget {
                request: r.clone(),
                period_q8: *p.period_q8(),
                amplitude_q15: *p.amplitude_q15(),
                initial_phase_q8: *cursor.initial_phase_q8(),
            },
            losses: [
                AudioPeriodicNumericLoss {
                    remainder: *p.period_remainder(),
                    denominator: i128::from(*r.frequency().value().numerator_hz()),
                },
                AudioPeriodicNumericLoss {
                    remainder: *p.amplitude_remainder(),
                    denominator: i128::from(*r.amplitude().denominator()),
                },
                AudioPeriodicNumericLoss {
                    remainder: *cursor.phase_remainder(),
                    denominator: i128::from(*r.phase().denominator()),
                },
            ],
        })
    }
    pub fn source(&self) -> &AudioPeriodicCursor {
        self.cursor
    }
    pub fn target(&self) -> &AudioPeriodicNumericTarget {
        &self.target
    }
    pub fn facts(&self) -> [ProjectionFact<'_, Self>; 6] {
        let precision = |index: usize, name: &'static str, law: &'static str| {
            if self.losses[index].remainder == 0 {
                ProjectionFact::Transformed {
                    obligation: text(name),
                    law: text(law),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text(name),
                    class: ProjectionLoss::Precision,
                    detail: &self.losses[index],
                    native_fact: None,
                }
            }
        };
        [
            ProjectionFact::Preserved {
                obligation: text("full-original-request-context"),
            },
            ProjectionFact::Preserved {
                obligation: text("initial-finite-frame-budget"),
            },
            ProjectionFact::Transformed {
                obligation: text("phase-modulo-whole-turns"),
                law: text("audio/periodic-equivalent-phase-modulo@1"),
            },
            precision(0, "period-q8-precision", "audio/source-period-q8@1"),
            precision(1, "amplitude-q15-precision", "audio/source-amplitude-q15@1"),
            precision(
                2,
                "initial-phase-q8-precision",
                "audio/source-modulo-phase-q8@1",
            ),
        ]
    }
}
impl ProjectionDomain for AudioPeriodicNumericProjection<'_> {
    type Source = AudioPeriodicCursor;
    type Target = AudioPeriodicNumericTarget;
    type Detail = AudioPeriodicNumericLoss;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("audio/admitted-initial-periodic-cursor@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("audio/period-q8-amplitude-q15-phase-q8@1")
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
        if source != self.cursor
            || target != Some(&self.target)
            || !native.is_empty()
            || !scores.is_empty()
            || mechanism != ProjectionMechanism::Completed
        {
            return false;
        }
        let expected = self.facts();
        facts.len() == expected.len()
            && facts.iter().zip(&expected).all(|(a, b)| match (a, b) {
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
}
pub struct AudioPeriodicNumericPolicy;
impl ProjectionPolicy<AudioPeriodicNumericProjection<'_>> for AudioPeriodicNumericPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("audio/explicit-periodic-integer-quantization@1")
    }
    fn permits(
        &self,
        _: &AudioPeriodicCursor,
        _: &AudioPeriodicNumericTarget,
        facts: &[ProjectionFact<'_, AudioPeriodicNumericProjection<'_>>],
    ) -> bool {
        facts.len() == 6
            && facts[3..].iter().all(|f| match f {
                ProjectionFact::Transformed { .. } => true,
                ProjectionFact::Lost {
                    class: ProjectionLoss::Precision,
                    detail,
                    ..
                } => detail.remainder > 0 && detail.remainder < detail.denominator,
                _ => false,
            })
    }
}
