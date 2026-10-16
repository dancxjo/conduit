//! One finite compiled enclosure/rounding profile; no fallback or adaptive retry.
use super::*;
impl PreparedNumericalRealization {
    pub(super) fn prepare_enclosed(
        source: &MathematicalValue,
        rounding: Binary64Rounding,
        maximum_error: ExactRational,
        work: &mut ExactNumericWork,
    ) -> Result<Self, NumericalRealizationRefusal> {
        use NumericalRealizationRefusal as R;
        if maximum_error.is_negative() {
            return Err(R::NegativeTolerance);
        }
        let before = work.spent();
        let admitted_limb_work = work.remaining();
        let requirements = crate::MathematicalEnclosureRequirements {
            maximum_width: maximum_error.checked_divide(ExactRational::from_i128(4), work)?,
            maximum_leaf_width: maximum_error
                .checked_divide(ExactRational::from_i128(128), work)?,
            maximum_constant_terms: 32,
            maximum_root_refinements: 64,
            maximum_scratch_payload_bytes: 131072,
            maximum_retained_payload_bytes: 131072,
        };
        let prepared = PreparedEnclosedBinary64::prepare(
            source,
            requirements,
            rounding,
            maximum_error,
            Binary64AlgorithmLimits {
                maximum_steps: 1,
                maximum_retained_bytes: 131072,
            },
            work,
        )
        .map_err(R::EnclosedBinary64)?;
        Ok(Self {
            source: source.clone(),
            algorithm: NumericalAlgorithm::EnclosedBinary64 { rounding },
            maximum_error: prepared.maximum_error(),
            target: NumericalTarget::EnclosedBinary64(Box::new(prepared)),
            prepared_error_limit: maximum_error,
            admitted_limb_work,
            spent_limb_work: work.spent() - before,
        })
    }
}
