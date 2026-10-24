//! Linux recorder realization beneath the ordinary kernel and retained Root
//! owner. Preparation and matching identity data create no start or authority.
mod recovery;
pub use recovery::{FileRecoveryCompletion, FileRecoveryRefusal, PreparedFileRecovery};

mod batch;
pub use batch::{FileBatchSoftware, PreparedFileBatchRecovery};
