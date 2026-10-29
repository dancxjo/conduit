//! Bounded raw signed evidence retrieval through an exactly selected Host Call.
//! Retrieval neither commits recorder State nor authorizes evidence retirement.
pub mod codec;
pub mod contract;
pub mod execution;
pub use execution::SelectedHistoryExecution;
pub mod installation;
pub mod owner;
pub use contract::HistoryReadContract;
pub use installation::{HistoryNativeIdentity, ReadyHistoryBase};
pub use owner::{HistoryAttachment, HistoryCallRefusal, HistoryCallSelection, HistoryHostCall};

pub const MAXIMUM_SIGNED_BATCH_BYTES: usize = 2048;
#[derive(Debug, PartialEq, Eq)]
pub enum HistoryProviderRefusal {
    Unavailable,
    InvalidCursor,
    Capacity,
}
/// Independent finite retained evidence provider; no retry or transport policy.
pub trait HistoryProvider {
    fn read_signed_batch(
        &mut self,
        cursor: u32,
        output: &mut [u8; MAXIMUM_SIGNED_BATCH_BYTES],
    ) -> Result<usize, HistoryProviderRefusal>;
    fn revoke(&mut self);
}

#[cfg(test)]
mod kernel_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
