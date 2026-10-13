//! Typed results and private checked witness custody.
use super::*;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericResultEvidence {
    Enclosure(RationalInterval),
    ExactMathematical(MathValue),
    ExactPhysical(alloc::boxed::Box<crate::Quantity>),
    ExactBoolean(bool),
    IeeeBits(Binary64Value),
}
impl NumericResultEvidence {
    pub fn mathematical(&self) -> Option<&MathValue> {
        match self {
            Self::ExactMathematical(value) => Some(value),
            _ => None,
        }
    }
    pub fn interval(&self) -> Option<&RationalInterval> {
        match self {
            Self::Enclosure(value) => Some(value),
            _ => None,
        }
    }
    pub fn boolean(&self) -> Option<bool> {
        match self {
            Self::ExactBoolean(value) => Some(*value),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericAlgorithm {
    OrderedRationalV1,
    OrderedCheckedI64V1,
    SymbolicRetentionV1,
    OrderedSoftwareBinary64V1,
    CertifiedRationalIntervalV1,
    ExactComparisonV1,
    OrderedIeeeSoftwareBinary64V1,
}
impl NumericAlgorithm {
    pub const fn identity(self) -> &'static str {
        match self {
            Self::SymbolicRetentionV1 => "conduit.numeric/symbolic-retention@1",
            Self::OrderedCheckedI64V1 => "conduit.numeric/ordered-checked-i64@1",
            Self::OrderedRationalV1 => "conduit.numeric/ordered-rational@1",
            Self::OrderedSoftwareBinary64V1 => "conduit.numeric/ordered-software-binary64@1",
            Self::CertifiedRationalIntervalV1 => "conduit.numeric/certified-rational-interval@1",
            Self::ExactComparisonV1 => "conduit.numeric/exact-comparison@1",
            Self::OrderedIeeeSoftwareBinary64V1 => {
                "conduit.numeric/ordered-ieee-software-binary64@1"
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericCertificateClaim {
    pub engine_identity: [u8; 32],
    pub graph_identity: [u8; 32],
    pub algorithm: NumericAlgorithm,
    pub requirement_identity: [u8; 32],
    pub result: NumericResultEvidence,
    pub error: RationalInterval,
    pub steps: Vec<Binary64StepEvidence>,
    pub boolean_steps: Vec<BooleanStepEvidence>,
    pub integer_steps: Vec<I64StepEvidence>,
    pub flags: Binary64Flags,
    pub work_steps: u64,
    pub storage_bytes: u32,
    pub required_limb_capacity: u16,
    pub required_symbolic_nodes: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedNumericWitness {
    pub(super) requirement: NumericRequirement,
    pub(super) claim: NumericCertificateClaim,
}
impl VerifiedNumericWitness {
    pub fn required_limb_capacity(&self) -> u16 {
        self.claim.required_limb_capacity
    }
    pub fn required_symbolic_nodes(&self) -> u16 {
        self.claim.required_symbolic_nodes
    }
    pub fn selected_representation(&self) -> NumericRepresentation {
        match self.claim.algorithm {
            NumericAlgorithm::OrderedCheckedI64V1 => NumericRepresentation::CheckedI64,
            NumericAlgorithm::OrderedRationalV1 | NumericAlgorithm::ExactComparisonV1 => {
                NumericRepresentation::ExactRational
            }
            NumericAlgorithm::OrderedSoftwareBinary64V1 => NumericRepresentation::Binary64,
            NumericAlgorithm::OrderedIeeeSoftwareBinary64V1 => NumericRepresentation::IeeeBinary64,
            NumericAlgorithm::CertifiedRationalIntervalV1 => {
                NumericRepresentation::CertifiedInterval
            }
            NumericAlgorithm::SymbolicRetentionV1 => NumericRepresentation::Symbolic,
        }
    }
    pub const fn work_unit_identity(&self) -> &'static str {
        crate::numeric_projection::NUMERIC_WORK_UNIT_ID
    }
    pub fn requirement(&self) -> &NumericRequirement {
        &self.requirement
    }
    pub fn claim(&self) -> &NumericCertificateClaim {
        &self.claim
    }
    pub fn graph_identity(&self) -> &[u8; 32] {
        &self.claim.graph_identity
    }
    pub fn requirement_identity(&self) -> &[u8; 32] {
        &self.claim.requirement_identity
    }
}
