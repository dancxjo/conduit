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
pub use conduit_resource_snapshot::{
    snapshot_reference, PreparedSnapshotOperation, AUTHORITY_CONTRACT, MAXIMUM_RECORD_BYTES,
    PUBLISH_OPERATION, READ_OPERATION,
};
#[cfg(target_os = "linux")]
pub use operation::{FileOperationRefusal, PreparedFileSnapshotOperation};

/// Fixed snapshot operation and record backing plus bounded canonical names,
/// reference and owner-prefix allocations. This excludes whole Body preparation.
pub fn preparation_byte_bound(owner: &conduit_core::HostId) -> u64 {
    (std::mem::size_of::<PreparedFileSnapshotOperation<'static>>()
        + 2 * (owner.as_str().len() + 8 + 2 + 2 + 512 + 73)
        + 512) as u64
}
