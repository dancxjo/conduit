//! Physical checked-program custody around the existing ordered computation report.
//! Exact artifact references require the original program and incoming frame for replay.
use super::*;
use conduit_core::projection::*;
use core::marker::PhantomData;
mod codec;
mod consumer;
pub use codec::*;
pub use consumer::*;
const PROGRAM: &str = "checked-program-custody";
const FRAME: &str = "physical-family-role-coordinates";
const VALUE: &str = "mathematical-denotation";
const ORDER: &str = "numeric-operation-order";
const LAW: &str = "numeric/source-bound-binary64-order@1";
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).unwrap()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualifiedQuantityBinary64Target {
    coordinate: IeeeF64,
    unit: Unit,
    role: QuantityRole,
}
impl QualifiedQuantityBinary64Target {
    pub const fn coordinate(self) -> IeeeF64 {
        self.coordinate
    }
    pub const fn unit(self) -> Unit {
        self.unit
    }
    pub const fn role(self) -> QuantityRole {
        self.role
    }
}
#[derive(Debug, Clone, Copy)]
pub struct QualifiedQuantityBinary64Policy {
    program_digest: [u8; 32],
    unit: Unit,
    role: QuantityRole,
    numeric: Binary64AlgorithmPolicy,
    digest: [u8; 32],
}
impl QualifiedQuantityBinary64Policy {
    pub(super) fn new(
        program_digest: [u8; 32],
        unit: Unit,
        role: QuantityRole,
        numeric: Binary64AlgorithmPolicy,
    ) -> Self {
        let mut hash = Sha256::new();
        hash.update(b"numeric/source-bound-binary64-policy@1");
        hash.update(program_digest);
        hash.update(unit.encode());
        hash.update([role_tag(role)]);
        hash.update(
            numeric
                .configuration_digest()
                .expect("bound numeric policy"),
        );
        Self {
            program_digest,
            unit,
            role,
            numeric,
            digest: hash.finalize().into(),
        }
    }
}
fn role_tag(role: QuantityRole) -> u8 {
    match role {
        QuantityRole::Linear => 0,
        QuantityRole::Point => 1,
        QuantityRole::Delta => 2,
    }
}
impl QualifiedQuantityBinary64Result<'_> {
    pub fn canonical_program(&self) -> &[u8] {
        self.canonical_program
    }
    pub const fn incoming_evidence_digest(&self) -> [u8; 32] {
        self.incoming_digest
    }
    pub const fn policy(&self) -> QualifiedQuantityBinary64Policy {
        self.policy
    }
    pub fn target(&self) -> QualifiedQuantityBinary64Target {
        QualifiedQuantityBinary64Target {
            coordinate: self.algorithm.target(),
            unit: self.source.unit(),
            role: self.source.role(),
        }
    }
    pub fn projection_facts(&self) -> [ProjectionFact<'_, QualifiedQuantityBinary64Domain<'_>>; 4] {
        [
            ProjectionFact::Preserved {
                obligation: text(PROGRAM),
            },
            ProjectionFact::Preserved {
                obligation: text(FRAME),
            },
            if self.error.maximum() == ExactRational::ZERO {
                ProjectionFact::Preserved {
                    obligation: text(VALUE),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text(VALUE),
                    class: ProjectionLoss::Precision,
                    detail: &self.error,
                    native_fact: None,
                }
            },
            ProjectionFact::Transformed {
                obligation: text(ORDER),
                law: text(LAW),
            },
        ]
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct QualifiedQuantityBinary64Domain<'a>(PhantomData<&'a [u8]>);
impl QualifiedQuantityBinary64Domain<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
impl<'a> ProjectionDomain for QualifiedQuantityBinary64Domain<'a> {
    type Source = QualifiedQuantityBinary64Result<'a>;
    type Target = QualifiedQuantityBinary64Target;
    type Detail = Binary64ErrorClaim;
    fn source_contract(&self) -> ProjectionText<'_> {
        text("numeric/qualified-checked-quantity@1")
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
        if target != Some(&source.target())
            || facts.len() != 4
            || !native.is_empty()
            || !scores.is_empty()
            || mechanism != ProjectionMechanism::Completed
        {
            return false;
        }
        facts.iter().any(|f|matches!(f,ProjectionFact::Preserved{obligation} if obligation.as_str()==PROGRAM))
        && facts.iter().any(|f|matches!(f,ProjectionFact::Preserved{obligation} if obligation.as_str()==FRAME))
        && facts.iter().any(|f|matches!(f,ProjectionFact::Transformed{obligation,law} if obligation.as_str()==ORDER && law.as_str()==LAW))
        && facts.iter().any(|f|match f {
            ProjectionFact::Preserved{obligation}=>obligation.as_str()==VALUE && source.error.maximum()==ExactRational::ZERO,
            ProjectionFact::Lost{obligation,class,detail,native_fact}=>obligation.as_str()==VALUE && *class==ProjectionLoss::Precision
                && **detail==source.error && source.error.maximum()!=ExactRational::ZERO && native_fact.is_none(),_=>false })
    }
}
impl<'a> ProjectionPolicy<QualifiedQuantityBinary64Domain<'a>> for QualifiedQuantityBinary64Policy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/source-bound-binary64-policy@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        Some(self.digest)
    }
    fn permits(
        &self,
        source: &QualifiedQuantityBinary64Result<'a>,
        target: &QualifiedQuantityBinary64Target,
        facts: &[ProjectionFact<'_, QualifiedQuantityBinary64Domain<'a>>],
    ) -> bool {
        self.digest == source.policy.digest
            && QualifiedQuantityBinary64Domain::new().validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
