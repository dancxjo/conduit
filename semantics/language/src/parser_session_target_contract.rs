//! Explicit external target contract, separate from proven Session allocations.
//! A generic Session cannot inspect an opaque executor's heap. The target owns
//! this declaration; Session admission includes it in the simultaneous budget.
use crate::parser_session_fixed_ingress::ParserSessionExecutor;
use alloc::rc::Rc;
use conduit_core::Plan;
use conduit_plot::{CheckedSyntaxDocument, ExpandedAuthoringPlot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserSessionTargetStorageContract {
    retained_bytes: usize,
    execution_temporary_bytes: usize,
    maximum_input_bytes: usize,
    maximum_output_bytes: usize,
    shared_storage_excluded: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserSessionTargetContractRefusal {
    Capacity,
    Overflow,
}
impl ParserSessionTargetStorageContract {
    /// Declares target-owned storage, including its original Source, Plan,
    /// execution owners and queues. This is a target obligation, not a measured
    /// Native-family or Session heap receipt. Session preparation verifies the
    /// exact original Source/Plan independently of these resource quantities.
    pub fn new(
        retained_bytes: usize,
        execution_temporary_bytes: usize,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, ParserSessionTargetContractRefusal> {
        if retained_bytes == 0
            || maximum_input_bytes == 0
            || maximum_output_bytes == 0
            || maximum_input_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || maximum_output_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(ParserSessionTargetContractRefusal::Capacity);
        }
        retained_bytes
            .checked_add(execution_temporary_bytes)
            .ok_or(ParserSessionTargetContractRefusal::Overflow)?;
        Ok(Self {
            retained_bytes,
            execution_temporary_bytes,
            maximum_input_bytes,
            maximum_output_bytes,
            shared_storage_excluded: false,
        })
    }
    /// Declares only this target's private retained storage. The complete shared
    /// immutable owner must be returned by shared_storage_owner and is charged
    /// separately by exact Rc identity. This remains an opaque target obligation.
    pub fn excluding_shared_storage(
        retained_bytes: usize,
        execution_temporary_bytes: usize,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, ParserSessionTargetContractRefusal> {
        let mut contract = Self::new(
            retained_bytes,
            execution_temporary_bytes,
            maximum_input_bytes,
            maximum_output_bytes,
        )?;
        contract.shared_storage_excluded = true;
        Ok(contract)
    }
    pub(crate) fn shared_storage_excluded(self) -> bool {
        self.shared_storage_excluded
    }
    pub fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
    pub fn execution_temporary_bytes(self) -> usize {
        self.execution_temporary_bytes
    }
    pub fn maximum_input_bytes(self) -> usize {
        self.maximum_input_bytes
    }
    pub fn maximum_output_bytes(self) -> usize {
        self.maximum_output_bytes
    }
    pub fn combined_bytes(self) -> usize {
        self.retained_bytes + self.execution_temporary_bytes
    }
}
/// An already prepared external target retains its original checked Source and
/// immutable original Plan. The fixed Session factory admits complete topology,
/// ordered cords, all Type/law metadata and the original seal before any ingress.
/// Whole canonical input/output custody then remains checked at every call.
pub trait ParserSessionPreparedTarget: ParserSessionExecutor {
    fn checked_source(&self) -> &CheckedSyntaxDocument;
    fn expanded_source(&self) -> &ExpandedAuthoringPlot;
    fn original_plan_owner(&self) -> Rc<Plan>;
    fn storage_contract(&self) -> ParserSessionTargetStorageContract;
    fn shared_storage_owner(
        &self,
    ) -> Option<&Rc<crate::parser_session_shared_target_storage::ParserSessionSharedTargetStorage>>
    {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_contract_refuses_unrepresentable_and_oversized_envelopes() {
        assert_eq!(
            ParserSessionTargetStorageContract::new(usize::MAX, 1, 1, 1),
            Err(ParserSessionTargetContractRefusal::Overflow)
        );
        assert_eq!(
            ParserSessionTargetStorageContract::new(
                1,
                0,
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1,
                1
            ),
            Err(ParserSessionTargetContractRefusal::Capacity)
        );
        let contract = ParserSessionTargetStorageContract::new(64, 32, 1024, 1024).unwrap();
        assert_eq!(contract.combined_bytes(), 96);
        assert_eq!(contract.execution_temporary_bytes(), 32);
    }
}

/// Owns a target before an ingress exists. Any refusal or unwind must cancel
/// even this unconsumed target; release transfers that obligation to the next
/// preparation guard or the final prepared ingress. No allocation is performed.
pub(crate) struct ParserTargetPreparationGuard<E: ParserSessionExecutor> {
    target: Option<E>,
}
impl<E: ParserSessionExecutor> ParserTargetPreparationGuard<E> {
    pub(crate) fn new(target: E) -> Self {
        Self {
            target: Some(target),
        }
    }
    pub(crate) fn get(&self) -> &E {
        self.target.as_ref().expect("owned target")
    }
    pub(crate) fn release(&mut self) -> E {
        self.target.take().expect("single target transfer")
    }
}
impl<E: ParserSessionExecutor> Drop for ParserTargetPreparationGuard<E> {
    fn drop(&mut self) {
        if let Some(target) = &mut self.target {
            target.cancel();
        }
    }
}

/// Allocation-free individual target reservation. Legacy full-storage contracts
/// may supply an owner but never subtract it from their already complete charge.
pub(crate) fn target_declared_bytes<T: ParserSessionPreparedTarget>(target: &T) -> Option<usize> {
    let contract = target.storage_contract();
    if contract.shared_storage_excluded() {
        contract
            .combined_bytes()
            .checked_add(target.shared_storage_owner()?.declared_retained_bytes())
    } else {
        Some(contract.combined_bytes())
    }
}
pub(crate) fn validate_shared_source_owner<T: ParserSessionPreparedTarget>(target: &T) -> bool {
    match target.shared_storage_owner() {
        Some(owner) => owner.validate_source_owner(target.checked_source()).is_ok(),
        None => !target.storage_contract().shared_storage_excluded(),
    }
}
