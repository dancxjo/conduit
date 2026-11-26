//! Final generic validation and freezing shared by explicit Source realizations.
use super::PreparedEnvironmentRecordingHistorySource;
use conduit_data::{PreparedQuantityObservationSnapshotInput, ResolvedQuantityObservationSnapshot};
use conduit_plot::rust_binding::NativeRustBinding;
const MAXIMUM_FROZEN_BYTES: usize = 16 * 1024;

pub(crate) fn freeze_preparation_backing_bound() -> Result<usize, String> {
    conduit_data::quantity_observation_snapshot_input_preparation_backing_bound()
        .map_err(debug)?
        .checked_add(conduit_data::resolved_quantity_observation_snapshot_backing_bound())
        .and_then(|bytes| bytes.checked_add(3 * MAXIMUM_FROZEN_BYTES))
        .ok_or_else(|| "snapshot freeze preparation backing overflow".into())
}
pub(crate) fn freeze_snapshot(
    snapshot: ResolvedQuantityObservationSnapshot,
) -> Result<PreparedEnvironmentRecordingHistorySource, String> {
    let decoder = PreparedQuantityObservationSnapshotInput::prepare().map_err(debug)?;
    conduit_data::validate_quantity_observation_snapshot(&snapshot).map_err(debug)?;
    let encoded = snapshot.encode().map_err(debug)?;
    if encoded.len() > MAXIMUM_FROZEN_BYTES || encoded.len() > decoder.maximum_bytes() as usize {
        return Err("quantity history capture exceeds the existing worker result profile".into());
    }
    decoder.decode(&encoded).map_err(debug)?;
    Ok(PreparedEnvironmentRecordingHistorySource {
        frozen: encoded.into_boxed_slice(),
    })
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("quantity history freeze: {error:?}")
}
