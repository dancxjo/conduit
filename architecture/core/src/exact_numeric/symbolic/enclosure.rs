//! Algorithm-wide certified rational enclosure of bounded exact expressions.
//! Every intermediate is exact interval arithmetic; this is not an IEEE
//! operation sequence or a claim about measurement uncertainty.
use super::{MathematicalNode as Node, MathematicalValue, Representation};
use crate::{
    projection::*, ExactNumericRefusal, ExactNumericWork, ExactRational, PreparedSquareRoot,
    SquareRootRefusal,
};
use alloc::vec::Vec;
use core::{cmp::Ordering, mem::size_of};
mod interval;
mod inverse_trigonometry;
pub use inverse_trigonometry::*;
mod inverse_circular;
pub use inverse_circular::*;
mod pi;
mod typed_atan2;
pub use interval::MathematicalInterval;
pub use typed_atan2::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalEnclosureRefusal {
    Numeric(ExactNumericRefusal),
    Root(SquareRootRefusal),
    NegativeTolerance,
    ConstantTermLimit,
    ExactConstantEnclosureUnsupported,
    ToleranceExceeded,
    ScratchStorageLimit,
    RetainedStorageLimit,
    InvalidExpression,
    UnsupportedTrigonometricEnclosure,
}
impl From<ExactNumericRefusal> for MathematicalEnclosureRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
impl From<SquareRootRefusal> for MathematicalEnclosureRefusal {
    fn from(value: SquareRootRefusal) -> Self {
        Self::Root(value)
    }
}
/// Absolute interval widths, finite algorithm work and Rust payload storage
/// ceilings are realization requirements, not changes to mathematical meaning.
/// Host admission separately accounts for allocator overhead and stack frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathematicalEnclosureRequirements {
    pub maximum_width: ExactRational,
    pub maximum_leaf_width: ExactRational,
    pub maximum_constant_terms: u16,
    pub maximum_root_refinements: u32,
    pub maximum_scratch_payload_bytes: u32,
    pub maximum_retained_payload_bytes: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedMathematicalEnclosure {
    source: MathematicalValue,
    target: MathematicalInterval,
    width: ExactRational,
    requirements: MathematicalEnclosureRequirements,
    scratch_payload_bytes: u32,
    retained_payload_bytes: u32,
    limb_work: u64,
    admitted_limb_work: u64,
    maximum_used_constant_terms: u16,
}
impl PreparedMathematicalEnclosure {
    pub fn prepare(
        source: &MathematicalValue,
        requirements: MathematicalEnclosureRequirements,
        work: &mut ExactNumericWork,
    ) -> Result<Self, MathematicalEnclosureRefusal> {
        use MathematicalEnclosureRefusal as R;
        if requirements.maximum_width.is_negative() || requirements.maximum_leaf_width.is_negative()
        {
            return Err(R::NegativeTolerance);
        }
        let nodes = match &source.representation {
            Representation::Expression(nodes) => nodes.as_slice(),
            _ => &[],
        };
        let scratch_payload_bytes = (nodes.len() * size_of::<MathematicalInterval>()) as u32;
        let retained_payload_bytes = (size_of::<Self>() + core::mem::size_of_val(nodes)) as u32;
        if scratch_payload_bytes > requirements.maximum_scratch_payload_bytes {
            return Err(R::ScratchStorageLimit);
        }
        if retained_payload_bytes > requirements.maximum_retained_payload_bytes {
            return Err(R::RetainedStorageLimit);
        }
        let before = work.spent();
        let admitted_limb_work = work.remaining();
        let mut maximum_used_constant_terms = 0;
        let target = if let Some(q) = source.as_rational() {
            MathematicalInterval::point(q)
        } else {
            let mut values = Vec::<MathematicalInterval>::with_capacity(nodes.len());
            for node in nodes {
                work.charge(1)?;
                let get = |id: super::MathematicalNodeId| {
                    values
                        .get(id.index as usize)
                        .copied()
                        .ok_or(R::InvalidExpression)
                };
                let value = match *node {
                    Node::Rational(q) => MathematicalInterval::point(q),
                    Node::Pi => {
                        let (interval, terms) = pi::enclose(
                            requirements.maximum_leaf_width,
                            requirements.maximum_constant_terms,
                            work,
                        )?;
                        maximum_used_constant_terms = maximum_used_constant_terms.max(terms);
                        interval
                    }
                    Node::SineTurns(_) | Node::CosineTurns(_) | Node::TangentTurns(_) => {
                        return Err(R::UnsupportedTrigonometricEnclosure)
                    }
                    Node::Tau => return Err(R::InvalidExpression), // Canonical construction expands 2π.
                    Node::Negate(a) => get(a)?.negated(),
                    Node::Add(a, b) => get(a)?.add(get(b)?, work)?,
                    Node::Subtract(a, b) => get(a)?.subtract(get(b)?, work)?,
                    Node::Multiply(a, b) => get(a)?.multiply(get(b)?, work)?,
                    Node::Divide(a, b) => {
                        let divisor = get(b)?;
                        if divisor.lower() != divisor.upper() {
                            return Err(R::InvalidExpression);
                        }
                        get(a)?.scale(
                            ExactRational::ONE.checked_divide(divisor.lower(), work)?,
                            work,
                        )?
                    }
                    Node::PrincipalSquareRoot(a) => {
                        let radicand = get(a)?;
                        if radicand.lower() != radicand.upper() {
                            return Err(R::InvalidExpression);
                        }
                        let root = PreparedSquareRoot::prepare(
                            radicand.lower(),
                            requirements.maximum_leaf_width,
                            requirements.maximum_root_refinements,
                            work,
                        )?;
                        MathematicalInterval::bounds(root.target().lower(), root.target().upper())
                    }
                };
                values.push(value);
            }
            *values.last().ok_or(R::InvalidExpression)?
        };
        let width = target.width(work)?;
        if width.checked_compare(requirements.maximum_width, work)? == Ordering::Greater {
            return Err(R::ToleranceExceeded);
        }
        Ok(Self {
            source: source.clone(),
            target,
            width,
            requirements,
            scratch_payload_bytes,
            retained_payload_bytes,
            limb_work: work.spent() - before,
            admitted_limb_work,
            maximum_used_constant_terms,
        })
    }
    pub fn source(&self) -> &MathematicalValue {
        &self.source
    }
    pub const fn target(&self) -> MathematicalInterval {
        self.target
    }
    pub const fn width(&self) -> ExactRational {
        self.width
    }
    pub const fn requirements(&self) -> MathematicalEnclosureRequirements {
        self.requirements
    }
    pub const fn scratch_payload_bytes(&self) -> u32 {
        self.scratch_payload_bytes
    }
    pub const fn retained_payload_bytes(&self) -> u32 {
        self.retained_payload_bytes
    }
    pub const fn admitted_limb_work(&self) -> u64 {
        self.admitted_limb_work
    }
    pub(in crate::exact_numeric) fn restore_admitted_work(&mut self, admitted: u64) {
        self.admitted_limb_work = admitted;
    }
    pub const fn limb_work(&self) -> u64 {
        self.limb_work
    }
    pub const fn maximum_used_constant_terms(&self) -> u16 {
        self.maximum_used_constant_terms
    }
    /// Only checked containment can prove different values. Overlapping
    /// enclosures remain unknown; approximate closeness is not equality.
    pub fn compare(
        &self,
        other: &Self,
        work: &mut ExactNumericWork,
    ) -> Result<super::MathematicalComparison, MathematicalEnclosureRefusal> {
        use super::MathematicalComparison as C;
        // Structural inspection is bounded by the admitted graph profile.
        work.charge(
            (2 * super::MATHEMATICAL_VALUE_MAX_NODES * (2 * crate::EXACT_NUMERIC_MAX_BITS / 32 + 1))
                as u64,
        )?;
        if self.source == other.source {
            return Ok(C::ProvedEqual);
        }
        if self.target.lower() == self.target.upper()
            && other.target.lower() == other.target.upper()
        {
            return Ok(
                if self
                    .target
                    .lower()
                    .checked_compare(other.target.lower(), work)?
                    .is_eq()
                {
                    C::ProvedEqual
                } else {
                    C::ProvedDifferent
                },
            );
        }
        if self
            .target
            .upper()
            .checked_compare(other.target.lower(), work)?
            == Ordering::Less
            || self
                .target
                .lower()
                .checked_compare(other.target.upper(), work)?
                == Ordering::Greater
        {
            return Ok(C::ProvedDifferent);
        }
        Ok(C::Unknown)
    }
    pub fn facts(&self) -> [ProjectionFact<'_, MathematicalEnclosureDomain>; 2] {
        [
            if self.width == ExactRational::ZERO {
                ProjectionFact::Preserved {
                    obligation: text(VALUE),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text(VALUE),
                    class: ProjectionLoss::Precision,
                    detail: &self.width,
                    native_fact: None,
                }
            },
            ProjectionFact::Transformed {
                obligation: text(REPRESENTATION),
                law: text(LAW),
            },
        ]
    }
}
const VALUE: &str = "exact-denotation";
const REPRESENTATION: &str = "numeric-representation";
const LAW: &str = "numeric/exact-interval-expression@1";
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("bounded reviewed numerical identity")
}
#[derive(Debug, Clone, Copy)]
pub struct MathematicalEnclosureDomain;
impl ProjectionDomain for MathematicalEnclosureDomain {
    type Source = PreparedMathematicalEnclosure;
    type Target = MathematicalInterval;
    type Detail = ExactRational;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(super::MATHEMATICAL_VALUE_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text(LAW)
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
        if mechanism != ProjectionMechanism::Completed
            || !native.is_empty()
            || !scores.is_empty()
            || target != Some(&source.target)
            || facts.len() != 2
        {
            return false;
        }
        let value = facts.iter().any(|fact| match fact {
            ProjectionFact::Preserved { obligation } => {
                obligation.as_str() == VALUE && source.width == ExactRational::ZERO
            }
            ProjectionFact::Lost {
                obligation,
                class,
                detail,
                native_fact,
            } => {
                obligation.as_str() == VALUE
                    && *class == ProjectionLoss::Precision
                    && **detail == source.width
                    && native_fact.is_none()
                    && source.width != ExactRational::ZERO
            }
            _ => false,
        });
        value
            && facts.iter().any(|fact| {
                matches!(fact, ProjectionFact::Transformed { obligation,law }
            if obligation.as_str() == REPRESENTATION && law.as_str() == LAW)
            })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct MathematicalEnclosurePolicy {
    pub requirements: MathematicalEnclosureRequirements,
}
impl ProjectionPolicy<MathematicalEnclosureDomain> for MathematicalEnclosurePolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/exact-interval-expression-policy@1")
    }
    fn permits(
        &self,
        source: &PreparedMathematicalEnclosure,
        target: &MathematicalInterval,
        facts: &[ProjectionFact<'_, MathematicalEnclosureDomain>],
    ) -> bool {
        self.requirements == source.requirements
            && MathematicalEnclosureDomain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
