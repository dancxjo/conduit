//! Prepared, source-specific numerical offers for ordinary realization admission.
//! A sealed certificate is an implementation fact, not a host precision label.
use super::{
    Binary64AlgorithmLimits, Binary64AlgorithmRefusal, Binary64Operation, Binary64Rounding,
    ExactNumericRefusal, ExactNumericWork, ExactRational, IntegerEnclosure,
    IntegerEnclosureRefusal, MathematicalValue, PreparedBinary64Algorithm,
    PreparedIntegerEnclosure,
};
use crate::{
    PrimitiveInfoKind, StructuredConfigurationValue, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use alloc::boxed::Box;
use core::cmp::Ordering;
mod codec;
pub use codec::numerical_realization_profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericalAlgorithm {
    ExactRational,
    OutwardI64,
    Binary64 { rounding: Binary64Rounding },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericalTarget {
    ExactRational(Box<ExactRational>),
    OutwardI64(IntegerEnclosure),
    Binary64(Box<PreparedBinary64Algorithm>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericalRealizationRefusal {
    Numeric(ExactNumericRefusal),
    Integer(IntegerEnclosureRefusal),
    Binary64(Binary64AlgorithmRefusal),
    CertificateExtentExceeded,
    InvalidRequest,
    UnsupportedDenotation,
    NegativeTolerance,
    SourceMismatch,
    ToleranceExceeded,
    InvalidCertificate,
}
impl From<Binary64AlgorithmRefusal> for NumericalRealizationRefusal {
    fn from(value: Binary64AlgorithmRefusal) -> Self {
        Self::Binary64(value)
    }
}
impl From<ExactNumericRefusal> for NumericalRealizationRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
impl From<IntegerEnclosureRefusal> for NumericalRealizationRefusal {
    fn from(value: IntegerEnclosureRefusal) -> Self {
        Self::Integer(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericalRealizationRequest {
    source: MathematicalValue,
    maximum_error: ExactRational,
}
impl NumericalRealizationRequest {
    pub fn new(
        source: MathematicalValue,
        maximum_error: ExactRational,
    ) -> Result<Self, NumericalRealizationRefusal> {
        if maximum_error.is_negative() {
            return Err(NumericalRealizationRefusal::NegativeTolerance);
        }
        Ok(Self {
            source,
            maximum_error,
        })
    }
    pub fn source(&self) -> &MathematicalValue {
        &self.source
    }
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    /// The declaring Kind owns its exact request Type; this interprets only the
    /// reviewed two-field shape after the ordinary configuration gate checked it.
    pub fn from_configuration(
        value: &StructuredConfigurationValue,
        work: &mut ExactNumericWork,
    ) -> Result<Self, NumericalRealizationRefusal> {
        use NumericalRealizationRefusal as R;
        let value = StructuredInfoValue::from_canonical_bytes(value.canonical_value())
            .map_err(|_| R::InvalidRequest)?;
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            return Err(R::InvalidRequest);
        };
        if fields.len() != 2 {
            return Err(R::InvalidRequest);
        }
        let decode = |name: &str, work: &mut ExactNumericWork| {
            let field = fields
                .iter()
                .find(|f| f.name() == name)
                .ok_or(R::InvalidRequest)?;
            if !matches!(field.value().value_type().shape(), StructuredInfoTypeShape::Leaf(kind)
                if kind.as_str() == super::MATHEMATICAL_VALUE_ID)
            {
                return Err(R::InvalidRequest);
            }
            let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
                return Err(R::InvalidRequest);
            };
            MathematicalValue::decode(bytes, work).map_err(|_| R::InvalidRequest)
        };
        let source = decode("value", work)?;
        let maximum_error = decode("maximum-error", work)?
            .as_rational()
            .ok_or(R::UnsupportedDenotation)?;
        Self::new(source, maximum_error)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedNumericalRealization {
    source: MathematicalValue,
    algorithm: NumericalAlgorithm,
    target: NumericalTarget,
    maximum_error: ExactRational,
    prepared_error_limit: ExactRational,
    admitted_limb_work: u64,
    spent_limb_work: u64,
}
impl PreparedNumericalRealization {
    pub fn prepare(
        source: &MathematicalValue,
        algorithm: NumericalAlgorithm,
        maximum_outward_error: ExactRational,
        work: &mut ExactNumericWork,
    ) -> Result<Self, NumericalRealizationRefusal> {
        use NumericalRealizationRefusal as R;
        if let NumericalAlgorithm::Binary64 { rounding } = algorithm {
            let value = source.as_rational().ok_or(R::UnsupportedDenotation)?;
            return Self::prepare_binary64(
                source,
                &[Binary64Operation::Constant(Box::new(value))],
                rounding,
                maximum_outward_error,
                Binary64AlgorithmLimits {
                    maximum_steps: 32,
                    maximum_retained_bytes: 1_000_000,
                },
                work,
            );
        }
        if maximum_outward_error.is_negative() {
            return Err(R::NegativeTolerance);
        }
        let value = source.as_rational().ok_or(R::UnsupportedDenotation)?;
        let before = work.spent();
        let admitted_limb_work = work.remaining();
        let (target, maximum_error) = match algorithm {
            NumericalAlgorithm::Binary64 { .. } => {
                unreachable!("binary64 uses its operation-order preparation")
            }
            NumericalAlgorithm::ExactRational => (
                NumericalTarget::ExactRational(Box::new(value)),
                ExactRational::ZERO,
            ),
            NumericalAlgorithm::OutwardI64 => {
                let prepared = PreparedIntegerEnclosure::prepare(
                    value,
                    PrimitiveInfoKind::I64,
                    maximum_outward_error,
                    work,
                )?;
                let error = prepared.error();
                let maximum = if error
                    .lower_distance()
                    .checked_compare(error.upper_distance(), work)?
                    == Ordering::Greater
                {
                    error.lower_distance()
                } else {
                    error.upper_distance()
                };
                (NumericalTarget::OutwardI64(prepared.target()), maximum)
            }
        };
        Ok(Self {
            source: source.clone(),
            algorithm,
            target,
            maximum_error,
            admitted_limb_work,
            prepared_error_limit: maximum_outward_error,
            spent_limb_work: work.spent() - before,
        })
    }
    /// A Back may retain a specific arithmetic program. Its exact denotation
    /// must match the authored source, independently of its rounded result.
    pub fn prepare_binary64(
        source: &MathematicalValue,
        program: &[Binary64Operation],
        rounding: Binary64Rounding,
        maximum_error: ExactRational,
        limits: Binary64AlgorithmLimits,
        work: &mut ExactNumericWork,
    ) -> Result<Self, NumericalRealizationRefusal> {
        let denotation = source
            .as_rational()
            .ok_or(NumericalRealizationRefusal::UnsupportedDenotation)?;
        let before = work.spent();
        let admitted_limb_work = work.remaining();
        let prepared =
            PreparedBinary64Algorithm::prepare(program, rounding, maximum_error, limits, work)?;
        if prepared.source() != denotation {
            return Err(NumericalRealizationRefusal::SourceMismatch);
        }
        Ok(Self {
            source: source.clone(),
            algorithm: NumericalAlgorithm::Binary64 { rounding },
            maximum_error: prepared.error().maximum(),
            prepared_error_limit: maximum_error,
            target: NumericalTarget::Binary64(Box::new(prepared)),
            admitted_limb_work,
            spent_limb_work: work.spent() - before,
        })
    }
    pub const fn prepared_error_limit(&self) -> ExactRational {
        self.prepared_error_limit
    }
    pub fn admits(
        &self,
        request: &NumericalRealizationRequest,
        work: &mut ExactNumericWork,
    ) -> Result<(), NumericalRealizationRefusal> {
        use NumericalRealizationRefusal as R;
        if self.source != request.source {
            return Err(R::SourceMismatch);
        }
        if self
            .maximum_error
            .checked_compare(request.maximum_error, work)?
            == Ordering::Greater
        {
            return Err(R::ToleranceExceeded);
        }
        Ok(())
    }
    pub fn property(&self) -> Result<StructuredConfigurationValue, NumericalRealizationRefusal> {
        codec::encode(self)
    }
    pub fn from_property(
        property: &StructuredConfigurationValue,
        work: &mut ExactNumericWork,
    ) -> Result<Self, NumericalRealizationRefusal> {
        codec::decode(property, work)
    }
    pub fn source(&self) -> &MathematicalValue {
        &self.source
    }
    pub const fn algorithm(&self) -> NumericalAlgorithm {
        self.algorithm
    }
    pub fn target(&self) -> &NumericalTarget {
        &self.target
    }
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    pub const fn admitted_limb_work(&self) -> u64 {
        self.admitted_limb_work
    }
    pub const fn spent_limb_work(&self) -> u64 {
        self.spent_limb_work
    }
    /// Resident payload excludes allocator overhead; Host admission owns it.
    pub fn retained_payload_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
            + match self.target {
                NumericalTarget::ExactRational(_) => core::mem::size_of::<ExactRational>(),
                NumericalTarget::Binary64(ref algorithm) => algorithm.retained_payload_bytes(),
                _ => 0,
            }
    }
}
