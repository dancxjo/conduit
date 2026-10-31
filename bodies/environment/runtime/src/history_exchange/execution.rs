//! Prepare the ordinary HostCallBack together with its exact selected owner.
//! The caller installs this Back into the shared kernel, routes its actual
//! HostCallRequested event to the owner, and returns completion to that kernel.
use super::{
    contract::HISTORY_MAXIMUM_BYTES, HistoryCallRefusal, HistoryCallSelection, HistoryHostCall,
    HistoryProvider, ReadyHistoryBase,
};
use conduit_core::{BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim};
use conduit_kernel::scheduler::HostCallBack;

pub struct SelectedHistoryExecution<P> {
    back: HostCallBack,
    owner: HistoryHostCall<P>,
}
impl<P: HistoryProvider> SelectedHistoryExecution<P> {
    /// Possession and exact Plan/Play are checked before exposing the Back.
    /// This constructor performs no evidence read and never creates a grant.
    pub fn prepare(
        base: ReadyHistoryBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: HistoryCallSelection<'_>,
    ) -> Result<Self, HistoryCallRefusal> {
        let owner = base.bind_selected(table, handle, claim, selection)?;
        Ok(Self {
            back: HostCallBack::new(HISTORY_MAXIMUM_BYTES),
            owner,
        })
    }
    /// Retain both parts for the same kernel lifetime. HostCallBack supplies the
    /// monotonic RequestId checked by owner.invoke; rebuilding the Back per
    /// request would reset that sequence and is not a continuing execution.
    pub fn into_parts(self) -> (HostCallBack, HistoryHostCall<P>) {
        (self.back, self.owner)
    }
}
