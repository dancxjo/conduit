//! Linux file effects beneath the shared immutable snapshot record boundary.
//! Root retains Host/Boot/Play, live capability and storage-capacity admission.
#[cfg(not(target_os = "linux"))]
compile_error!("conduit-linux-snapshot-store requires Linux file-descriptor semantics");
#[cfg(target_os = "linux")]
mod directory;
#[cfg(target_os = "linux")]
mod snapshot;
#[cfg(target_os = "linux")]
pub use directory::SnapshotDirectory;
#[cfg(target_os = "linux")]
pub use snapshot::{FileSnapshotRefusal, IoStage, PreparedFileSnapshot};

#[cfg(target_os = "linux")]
mod operation;
#[cfg(target_os = "linux")]
pub use operation::{FileOperationRefusal, PreparedFileSnapshotOperation};

pub use conduit_resource_snapshot::{
    snapshot_reference, PreparedSnapshotOperation, AUTHORITY_CONTRACT, PUBLISH_OPERATION,
    READ_OPERATION,
};
