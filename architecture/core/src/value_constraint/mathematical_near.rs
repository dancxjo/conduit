//! Explicit prepared replay for a qualified symbolic physical projection.
use super::*;
use crate::{
    Binary32Rounding, ExactNumericWork, MathematicalEnclosureRequirements,
    NearQuantityEvidenceRefusal, PreparedMathematicalNearQuantityVerifier, RationalQuantity,
};

/// This optional consumer profile leaves the ordinary contract validator intact.
/// It pins the numerical replay strategy before Play, independently of the
/// supplier's execution identity. Native and rational frames retain their exact
/// verifier; mathematical frames use the prepared symbolic replay strategy.
pub struct PreparedMathematicalNearValueContract {
    contract: CheckedValueContract,
    verifier: PreparedMathematicalNearQuantityVerifier,
    maximum_work: u64,
}
impl PreparedMathematicalNearValueContract {
    pub fn new(
        contract: &CheckedValueContract,
        requirements: MathematicalEnclosureRequirements,
        rounding: Binary32Rounding,
        maximum_work: u64,
    ) -> Result<Self, NearQuantityEvidenceRefusal> {
        use NearQuantityEvidenceRefusal as R;
        contract.validate_definition().map_err(|_| R::Schema)?;
        let (limit, schema) = contract
            .constraints
            .iter()
            .find_map(|c| {
                if let ValueConstraint::NearQuantity { limit, schema, .. } = c {
                    Some((limit, schema))
                } else {
                    None
                }
            })
            .ok_or(R::Schema)?;
        // Definition checking establishes exactly one Near constraint and its
        // physical source family. Retain the entire authored predicate unchanged.
        let schema =
            crate::StructuredInfoType::from_canonical_bytes(schema).map_err(|_| R::Schema)?;
        let mut work = ExactNumericWork::new(maximum_work);
        let limit =
            RationalQuantity::from_quantity(**limit, &mut work).map_err(|_| R::InvalidLimit)?;
        let verifier = PreparedMathematicalNearQuantityVerifier::new(
            &schema,
            requirements,
            rounding,
            limit,
            maximum_work,
        )?;
        Ok(Self {
            contract: contract.clone(),
            verifier,
            maximum_work,
        })
    }
    pub const fn contract(&self) -> &CheckedValueContract {
        &self.contract
    }
    /// Replay buffers only; excludes the retained authored contract, this
    /// container, input/output storage, stack and allocator bookkeeping.
    pub fn prepared_verifier_heap_bytes(&self) -> usize {
        self.verifier.prepared_heap_bytes()
    }
    pub const fn maximum_numerical_work(&self) -> u64 {
        self.maximum_work
    }
    pub fn validate(
        &mut self,
        canonical: &[u8],
        work: &mut ExactNumericWork,
    ) -> Result<(), ValueConstraintRefusal> {
        if canonical.len() > self.contract.maximum_bytes as usize {
            return Err(ValueConstraintRefusal::Oversize {
                actual: u32::try_from(canonical.len()).unwrap_or(u32::MAX),
                maximum: self.contract.maximum_bytes,
            });
        }
        for constraint in &self.contract.constraints {
            if let ValueConstraint::NearQuantity { source_kind, .. } = constraint {
                // The verifier checks the complete pinned canonical schema and
                // physical family before replay; no unvalidated primitive bypass.
                self.verifier
                    .verify_admitted(canonical, source_kind.as_str(), None, work)
                    .map_err(ValueConstraintRefusal::NearQuantity)?;
            } else {
                constraint.validate_value(self.contract.value_kind.as_str(), canonical)?;
            }
        }
        Ok(())
    }
}
