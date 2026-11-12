//! Shared finite read-only recovery of the one Root-owned record residence.
use conduit_resource_snapshot::{append::*, native_append::AppendNativeIdentity, nor::NorStorage};
use sha2::{Digest, Sha256};
pub const RESERVED_PREFIX_BYTES: u32 = 8192;
#[allow(clippy::type_complexity)]
pub(crate) fn recover<S: NorStorage>(
    storage: S,
    identity: &AppendNativeIdentity,
    body: &conduit_body::BodyId,
    prefix_bytes: u32,
    slots: u32,
) -> Result<
    (
        PreparedNorRecordResidence<S>,
        Result<RecordRecovery, RecordResidenceRefusal>,
    ),
    (S, RecordResidenceRefusal),
> {
    let mut hash = Sha256::new();
    hash.update(b"conduit.root/nor-record-residence@1");
    for value in [body.as_str(), identity.host_id.as_str()] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    let residence = RecordResidenceId(hash.finalize().into());
    if body.as_str().is_empty() || prefix_bytes < RESERVED_PREFIX_BYTES {
        return Err((storage, RecordResidenceRefusal::Geometry));
    }
    let mut provider =
        PreparedNorRecordResidence::prepare(storage, residence, prefix_bytes, slots)?;
    let recovery = provider.recover();
    Ok((provider, recovery))
}
