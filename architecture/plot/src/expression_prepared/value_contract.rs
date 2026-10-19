//! Optional prepared replay belongs to the consuming expression profile.
use super::*;
use conduit_core::{
    Binary32Rounding, CheckedValueContract, ExactNumericWork, MathematicalEnclosureRequirements,
    PreparedMathematicalNearValueContract, ValueConstraint, ValueConstraintRefusal,
};
pub(super) enum PreparedValueContract {
    Ordinary(CheckedValueContract),
    Mathematical(Box<PreparedMathematicalNearValueContract>),
}
impl PreparedValueContract {
    pub(super) fn contract(&self) -> &CheckedValueContract {
        match self {
            Self::Ordinary(c) => c,
            Self::Mathematical(c) => c.contract(),
        }
    }
    pub(super) fn validate(&mut self, bytes: &[u8]) -> Result<(), ValueConstraintRefusal> {
        match self {
            Self::Ordinary(c) => c.validate(bytes),
            Self::Mathematical(c) => c.validate(
                bytes,
                &mut ExactNumericWork::new(c.maximum_numerical_work()),
            ),
        }
    }
    pub(super) fn replay_bounds(&self) -> Option<(u64, u64)> {
        match self {
            Self::Ordinary(_) => None,
            Self::Mathematical(c) => Some((
                (core::mem::size_of::<PreparedMathematicalNearValueContract>()
                    + c.prepared_verifier_heap_bytes()) as u64,
                c.maximum_numerical_work(),
            )),
        }
    }
}
impl PreparedPortableExpressionEvaluator {
    /// Explicit consumer replay profile. All storage is prepared here; this
    /// constructor alone is not a Host offer or a sealed resource admission.
    pub fn new_with_mathematical_near_contracts(
        program: &PortableExpressionProgram,
        requirements: MathematicalEnclosureRequirements,
        rounding: Binary32Rounding,
        maximum_work: u64,
    ) -> Result<Self, Refusal> {
        let mut evaluator = Self::new(program)?;
        let mut count = 0;
        for boundary in [
            &mut evaluator.input_value_contract,
            &mut evaluator.output_value_contract,
        ] {
            if let Some(contract) = boundary {
                if contract
                    .contract()
                    .constraints
                    .iter()
                    .any(|c| matches!(c, ValueConstraint::NearQuantity { .. }))
                {
                    let prepared = PreparedMathematicalNearValueContract::new(
                        contract.contract(),
                        requirements,
                        rounding,
                        maximum_work,
                    )
                    .map_err(|why| {
                        Refusal::UnsupportedType(alloc::format!(
                            "mathematical Near contract: {why:?}"
                        ))
                    })?;
                    *contract = PreparedValueContract::Mathematical(Box::new(prepared));
                    count += 1;
                }
            }
        }
        if count == 0 {
            return Err(Refusal::UnsupportedType(
                "mathematical Near requires a consuming contract".into(),
            ));
        }
        Ok(evaluator)
    }
}
