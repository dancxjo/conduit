//! Explicit pre-Play preparation of an immutable, ordered Native law bank.
use super::NativeBindingRefusal;
use crate::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
use alloc::vec::Vec;
use conduit_core::{InfoBool, StructuredInfoType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedNativeInvariantRefusal {
    Capacity,
    InvalidLaw {
        index: usize,
        refusal: crate::PortableExpressionEvaluationRefusal,
    },
}

/// Owns every supplied law in order. Preparation refuses unsupported laws;
/// evaluation never substitutes another Type or drops a law. The caller must
/// supply the complete checked Type's invariant bank, as for the allocating
/// `validate_native_invariants` entrance. Generated constructors do not yet use
/// this explicit preparation entrance.
///
/// Evaluation borrows already canonical bytes and retains capacity-stable
/// evaluator storage. The complete canonical input is validated once; every
/// law still verifies its exact immutable input Type.
pub struct PreparedNativeInvariantAdmission {
    laws: Vec<PreparedPortableExpressionEvaluator>,
    maximum_input_bytes: usize,
}

impl PreparedNativeInvariantAdmission {
    pub fn new(
        value_type: &StructuredInfoType,
        invariants: &[PortableExpressionProgram],
        maximum_laws: usize,
        maximum_input_bytes: usize,
    ) -> Result<Self, PreparedNativeInvariantRefusal> {
        if invariants.len() > maximum_laws {
            return Err(PreparedNativeInvariantRefusal::Capacity);
        }
        let mut laws = Vec::with_capacity(invariants.len());
        for (index, law) in invariants.iter().enumerate() {
            let invalid = |refusal| PreparedNativeInvariantRefusal::InvalidLaw { index, refusal };
            if &law.input_type != value_type
                || !matches!(law.output_type.shape(), conduit_core::StructuredInfoTypeShape::Leaf(name) if name.as_str() == conduit_core::BOOL_INFO_ID)
            {
                return Err(invalid(
                    crate::PortableExpressionEvaluationRefusal::InvalidProgram,
                ));
            }
            let ceiling = law.maximum_prepared_input_bytes().map_err(invalid)?;
            if ceiling as usize > maximum_input_bytes {
                return Err(PreparedNativeInvariantRefusal::Capacity);
            }
            laws.push(PreparedPortableExpressionEvaluator::new(law).map_err(invalid)?);
        }
        Ok(Self {
            laws,
            maximum_input_bytes,
        })
    }

    pub fn validate(&mut self, canonical: &[u8]) -> Result<(), NativeBindingRefusal> {
        if canonical.len() > self.maximum_input_bytes {
            return Err(NativeBindingRefusal::InvalidInvariant(
                crate::PortableExpressionEvaluationRefusal::InvalidInput,
            ));
        }
        if self.laws.is_empty() {
            return Ok(());
        }
        let input = crate::expression_prepared::PreparedCanonicalInput::new(canonical)
            .map_err(NativeBindingRefusal::InvalidInvariant)?;
        for (index, law) in self.laws.iter_mut().enumerate() {
            let result = law
                .evaluate_canonical(&input)
                .map_err(NativeBindingRefusal::InvalidInvariant)?;
            let accepted = InfoBool::decode(result).map(InfoBool::get).map_err(|_| {
                NativeBindingRefusal::InvalidInvariant(
                    crate::PortableExpressionEvaluationRefusal::InvalidProgram,
                )
            })?;
            if !accepted {
                return Err(NativeBindingRefusal::ViolatedInvariant { index });
            }
        }
        Ok(())
    }
}
