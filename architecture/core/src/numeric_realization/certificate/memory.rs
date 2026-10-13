//! Conservative memory custody ledger; this is independent of live queue bytes.
use super::*;
fn interval_owned(value: &RationalInterval) -> usize {
    value.lower().owned_storage_bytes() + value.upper().owned_storage_bytes()
}
fn result_owned(value: &NumericResultEvidence) -> usize {
    match value {
        NumericResultEvidence::ExactMathematical(value) => value.owned_storage_bytes(),
        NumericResultEvidence::ExactPhysical(value) => {
            core::mem::size_of::<crate::Quantity>() + value.encoded().len()
        }
        NumericResultEvidence::Enclosure(value) => interval_owned(value),
        _ => 0,
    }
}
impl VerifiedNumericWitness {
    /// Includes engine preparation peak, retained graph/requirement/claim,
    /// prepared output, verification/encoding copies and allocator headroom.
    /// Four times the retained payload plus a per-object allowance covers alignment
    /// and metadata for the reviewed fixed-limb/Vec/Box storage families.
    pub fn required_memory_reservation_bytes(
        &self,
        graph: &NumericComputation,
    ) -> Result<u32, NumericEngineRefusal> {
        let claim = &self.claim;
        let mut owned = core::mem::size_of::<Self>()
            + result_owned(&claim.result)
            + interval_owned(&claim.error);
        owned += claim.steps.capacity() * core::mem::size_of::<Binary64StepEvidence>();
        owned += claim
            .steps
            .iter()
            .map(|step| step.operands.capacity() * 8)
            .sum::<usize>();
        owned += claim.boolean_steps.capacity() * core::mem::size_of::<BooleanStepEvidence>();
        owned += claim
            .boolean_steps
            .iter()
            .map(|step| step.operands.capacity() * 8)
            .sum::<usize>();
        owned += claim.integer_steps.capacity() * core::mem::size_of::<I64StepEvidence>();
        owned += claim
            .integer_steps
            .iter()
            .map(|step| step.operands.capacity() * 8)
            .sum::<usize>();
        owned += self.requirement.parameters.capacity()
            * core::mem::size_of::<crate::NumericParameterRange>();
        owned += self
            .requirement
            .parameters
            .iter()
            .map(|range| range.lower.owned_storage_bytes() + range.upper.owned_storage_bytes())
            .sum::<usize>();
        owned += match &self.requirement.accuracy {
            NumericAccuracy::Absolute(value) | NumericAccuracy::EnclosureWidth(value) => {
                value.owned_storage_bytes()
            }
            NumericAccuracy::Relative {
                relative,
                zero_absolute,
            } => relative.owned_storage_bytes() + zero_absolute.owned_storage_bytes(),
            _ => 0,
        };
        owned += graph.owned_storage_bytes();
        // The prepared output's canonical representation is no larger than
        // these retained carriers plus bounded codec headers. Two retained
        // copies cover readmission while the selected witness remains live.
        let output = result_owned(&claim.result)
            .saturating_mul(2)
            .saturating_add(256);
        let reservation = (claim.storage_bytes as usize)
            .checked_add(owned.checked_mul(4).ok_or(NumericRefusal::StorageBudget)?)
            .and_then(|value| value.checked_add(output))
            .and_then(|value| value.checked_add(4096 + graph.nodes().len() * 64))
            .ok_or(NumericRefusal::StorageBudget)?;
        u32::try_from(reservation).map_err(|_| NumericRefusal::StorageBudget.into())
    }
}
