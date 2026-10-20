//! Source-bound linear coordinate computation; admission and emission remain Host work.
use crate::{PortableExpressionNode, PortableExpressionOperation as Op, PortableExpressionProgram};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::*;
use sha2::{Digest, Sha256};
mod evidence;
mod lowering;
pub use evidence::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualifiedQuantityBinary64Refusal {
    Program,
    UnsupportedOperation,
    UnsupportedDenotation,
    Physical,
    ExplicitCoordinateConversionRequired,
    UnsupportedRole,
    Storage,
    Evidence(NearQuantityEvidenceRefusal),
    Numerical(Binary64AlgorithmRefusal),
    Work(ExactNumericRefusal),
}
use QualifiedQuantityBinary64Refusal as R;
impl From<ExactNumericRefusal> for R {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Work(value)
    }
}
impl From<Binary64AlgorithmRefusal> for R {
    fn from(value: Binary64AlgorithmRefusal) -> Self {
        Self::Numerical(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualifiedQuantityBinary64Limits {
    pub algorithm: Binary64AlgorithmLimits,
    pub maximum_retained_payload_bytes: u32,
    pub maximum_numerical_work: u64,
}

/// A bounded coordinate profile, not an installed offer or an output certificate.
/// Value-coordinate changes, point arithmetic and Scalar arithmetic require their own laws.
pub struct PreparedQualifiedQuantityBinary64 {
    canonical_program: Vec<u8>,
    program_digest: [u8; 32],
    input_kind: KindId,
    input_limit: RationalQuantity,
    input_maximum_bytes: u32,
    verifier: PreparedNearQuantityVerifier,
    unit: Unit,
    role: QuantityRole,
    exact: PreparedBinary64ProgramRunner,
    projected: PreparedBinary64ProgramRunner,
    retained: usize,
    maximum_work: u64,
    policy: QualifiedQuantityBinary64Policy,
}

/// Available only after incoming evidence replay and the selected computation succeed.
pub struct QualifiedQuantityBinary64Result<'a> {
    algorithm: &'a PreparedBinary64Algorithm,
    source: RationalQuantity,
    incoming_evidence: &'a [u8],
    incoming_source_digest: [u8; 32],
    incoming_digest: [u8; 32],
    canonical_program: &'a [u8],
    policy: QualifiedQuantityBinary64Policy,
    error: Binary64ErrorClaim,
}
impl QualifiedQuantityBinary64Result<'_> {
    pub fn algorithm(&self) -> &PreparedBinary64Algorithm {
        self.algorithm
    }
    pub const fn source(&self) -> RationalQuantity {
        self.source
    }
    pub fn incoming_evidence(&self) -> &[u8] {
        self.incoming_evidence
    }
    pub const fn incoming_source_wire_digest(&self) -> [u8; 32] {
        self.incoming_source_digest
    }
}

impl PreparedQualifiedQuantityBinary64 {
    pub fn new(
        program: &PortableExpressionProgram,
        unit: Unit,
        role: QuantityRole,
        rounding: Binary64Rounding,
        limits: QualifiedQuantityBinary64Limits,
        work: &mut ExactNumericWork,
    ) -> Result<Self, R> {
        let canonical_program = program.canonical_bytes().map_err(|_| R::Program)?;
        let Op::QualifiedQuantity {
            input,
            output,
            physical_input,
            value,
        } = &program.root.operation
        else {
            return Err(R::Program);
        };
        let (input_kind, input_limit, schema) = contract(input, work)?;
        let (_, output_limit, _) = contract(output, work)?;
        if role == QuantityRole::Point {
            return Err(R::UnsupportedRole);
        }
        let zero = Quantity::from_decimal_role(0, 0, unit, role).map_err(|_| R::Physical)?;
        let input_limit = input_limit
            .convert_to_unit(unit, work)
            .map_err(rational_error)?;
        let output_limit = output_limit
            .convert_to_unit(unit, work)
            .map_err(rational_error)?;
        QuantityValueProfile::Mathematical
            .decode(input_kind.as_str(), &zero.encode(), work)
            .map_err(|e| match e {
                QuantityValueContractRefusal::Encoding(QuantityValueRefusal::Rational(
                    RationalQuantityRefusal::Numeric(e),
                )) => R::Work(e),
                _ => R::Physical,
            })?;
        let verifier = PreparedNearQuantityVerifier::new(&schema).map_err(R::Evidence)?;
        let mut operations = lowering::lower(
            value,
            physical_input,
            zero,
            limits.algorithm.maximum_steps,
            work,
        )?;
        let fixed = core::mem::size_of::<Self>()
            + canonical_program.capacity()
            + input_kind.as_str().len()
            + verifier.prepared_heap_bytes();
        let remaining = (limits.maximum_retained_payload_bytes as usize)
            .checked_sub(fixed)
            .ok_or(R::Storage)?;
        let runner_limits = |bytes| Binary64ProgramRunnerLimits {
            algorithm: limits.algorithm,
            maximum_retained_payload_bytes: bytes as u32,
            maximum_execution_work: limits.maximum_numerical_work,
        };
        let exact = PreparedBinary64ProgramRunner::new(
            &operations,
            &[0],
            rounding,
            output_limit.coordinate(),
            runner_limits(remaining),
            work,
        )?;
        let remaining = remaining
            .checked_sub(exact.retained_payload_bytes())
            .ok_or(R::Storage)?;
        operations[0] =
            Binary64Operation::ProjectedConstant(Box::new(Binary64ProjectedInput::new(
                ExactRational::ZERO,
                IeeeF64::from_bits(0),
                input_limit.coordinate(),
                work,
            )?));
        let projected = PreparedBinary64ProgramRunner::new(
            &operations,
            &[0],
            rounding,
            output_limit.coordinate(),
            runner_limits(remaining),
            work,
        )?;
        let retained = fixed + exact.retained_payload_bytes() + projected.retained_payload_bytes();
        let program_digest = Sha256::digest(&canonical_program).into();
        Ok(Self {
            canonical_program,
            program_digest,
            input_kind: input_kind.clone(),
            input_limit,
            input_maximum_bytes: input.maximum_bytes,
            verifier,
            unit,
            role,
            exact,
            projected,
            retained,
            maximum_work: limits.maximum_numerical_work,
            policy: QualifiedQuantityBinary64Policy::new(
                program_digest,
                unit,
                role,
                Binary64AlgorithmPolicy {
                    maximum_error: output_limit.coordinate(),
                    rounding,
                },
            ),
        })
    }

    /// Canonical program content; this digest is not a Source, Plan or execution identity.
    pub const fn program_digest(&self) -> [u8; 32] {
        self.program_digest
    }
    pub fn canonical_program(&self) -> &[u8] {
        &self.canonical_program
    }
    pub const fn policy(&self) -> QualifiedQuantityBinary64Policy {
        self.policy
    }
    pub const fn coordinate_unit(&self) -> Unit {
        self.unit
    }
    pub const fn coordinate_role(&self) -> QuantityRole {
        self.role
    }
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained
    }
    pub const fn maximum_numerical_work(&self) -> u64 {
        self.maximum_work
    }

    pub fn execute<'a>(
        &'a mut self,
        incoming: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<QualifiedQuantityBinary64Result<'a>, R> {
        work.with_limit(self.maximum_work, |local| {
            self.execute_inner(incoming, local)
        })
    }

    fn execute_inner<'a>(
        &'a mut self,
        incoming: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<QualifiedQuantityBinary64Result<'a>, R> {
        if incoming.len() > self.input_maximum_bytes as usize {
            return Err(R::Storage);
        }
        let verified = self
            .verifier
            .verify(
                incoming,
                self.input_kind.as_str(),
                self.input_limit,
                None,
                work,
            )
            .map_err(R::Evidence)?;
        if verified.source().role() != self.role {
            return Err(R::UnsupportedRole);
        }
        require_coordinate(verified.source().unit(), self.unit)?;
        let source = verified.source().coordinate();
        let algorithm = match verified.representation() {
            VerifiedNearRepresentation::ExactRational => self.exact.execute(&[source], work)?,
            VerifiedNearRepresentation::Binary32 { target, .. } => {
                // Widen the actual represented value exactly, including negative zero.
                // Do not recompute it from the original mathematical source.
                let represented = ExactRational::from_binary32(target, work)?;
                let widened = PreparedBinary64Projection::prepare(
                    represented,
                    Binary64Rounding::NearestTiesEven,
                    ExactRational::ZERO,
                    work,
                )
                .map_err(|e| R::Numerical(e.into()))?;
                let bits = if target.bits() == 0x8000_0000 {
                    IeeeF64::from_bits(1 << 63)
                } else {
                    widened.target()
                };
                let input =
                    Binary64ProjectedInput::new(source, bits, self.input_limit.coordinate(), work)?;
                self.projected
                    .execute_inputs(&[Binary64AlgorithmInput::Projected(&input)], work)?
            }
        };
        let source = RationalQuantity::new(algorithm.source(), self.unit, self.role)
            .map_err(|_| R::Physical)?;
        Ok(QualifiedQuantityBinary64Result {
            algorithm,
            source,
            incoming_evidence: incoming,
            incoming_source_digest: verified.source_wire_digest(),
            incoming_digest: Sha256::digest(incoming).into(),
            canonical_program: &self.canonical_program,
            policy: self.policy,
            error: algorithm.error(),
        })
    }
}

fn contract<'a>(
    c: &'a CheckedValueContract,
    work: &mut ExactNumericWork,
) -> Result<(&'a KindId, RationalQuantity, StructuredInfoType), R> {
    let [ValueConstraint::NearQuantity {
        source_kind,
        limit,
        schema,
    }] = c.constraints.as_slice()
    else {
        return Err(R::Program);
    };
    let limit = RationalQuantity::from_quantity(**limit, work).map_err(|e| match e {
        RationalQuantityRefusal::Numeric(e) => R::Work(e),
        _ => R::Physical,
    })?;
    let schema = StructuredInfoType::from_canonical_bytes(schema).map_err(|_| R::Program)?;
    Ok((source_kind, limit, schema))
}

fn rational_error(error: RationalQuantityRefusal) -> R {
    match error {
        RationalQuantityRefusal::Numeric(error) => R::Work(error),
        _ => R::Physical,
    }
}
fn require_coordinate(actual: Unit, selected: Unit) -> Result<(), R> {
    if actual == selected {
        return Ok(());
    }
    if actual.dimension() != selected.dimension() || actual.family() != selected.family() {
        return Err(R::Physical);
    }
    Err(R::ExplicitCoordinateConversionRequired)
}
