//! Full checked Language Source through the existing ordinary Std/kernel fixture.
//! Inputs remain the Native-admitted fallback owner's exact requests.
#[path = "resource_epoch_kernel.rs"]
mod resource_epoch_kernel;
use conduit_core::SourceDocumentId;
use conduit_language::analysis_epoch_owner::fallback_round::{
    FallbackObservationProvider, FallbackRoundRefusal,
};
pub struct Runtime {
    inner: resource_epoch_kernel::Runtime,
    pub operations: u64,
    pub source_id: SourceDocumentId,
}
impl Runtime {
    pub fn new() -> Self {
        let inner = resource_epoch_kernel::Runtime::new();
        Self {
            source_id: inner.source_id.clone(),
            inner,
            operations: 0,
        }
    }
    pub fn execute(&mut self, operation: &str, input: &[u8]) -> Vec<u8> {
        let output = self.inner.execute(operation, input);
        // The shared fixture increments only after verifying the actual Plan,
        // Play, HostCallCompleted, BackCompleted and absence of BackFailed.
        self.operations = u64::try_from(self.inner.operations).unwrap();
        output
    }
    pub fn assert_signs(&self) {
        assert!(self.operations > 0);
        assert_eq!(
            self.operations,
            u64::try_from(self.inner.operations).unwrap()
        );
    }
}
impl FallbackObservationProvider for Runtime {
    fn observe(&mut self, operation: &str, input: &[u8]) -> Result<Vec<u8>, FallbackRoundRefusal> {
        Ok(self.execute(operation, input))
    }
}
