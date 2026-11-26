//! Final domain validation and freezing; never a generic JSON decoder fallback.
use super::PreparedEnvironmentRecordingHistorySource;
use conduit_dht11_observation::{reported_generation, ObservationContract};

pub(crate) fn freeze_preparation_backing_bound(
    contract: &ObservationContract,
) -> Result<usize, String> {
    let maximum = reported_generation::maximum_canonical_bytes().map_err(debug)? as usize;
    reported_generation::PreparedReportedRecordingGenerationInput::decoding_backing_bound(contract)
        // Encoded Vec capacity and its boxed replacement can coexist. This
        // derives from the domain contract, not an enlarged worker envelope.
        .and_then(|bytes| bytes.checked_add(maximum.checked_mul(3)?))
        .ok_or_else(|| "reported generation freeze preparation backing overflow".into())
}
pub(crate) fn freeze_generation(
    encoded: Vec<u8>,
    contract: &ObservationContract,
) -> Result<PreparedEnvironmentRecordingHistorySource, String> {
    let decoder =
        reported_generation::PreparedReportedRecordingGenerationInput::prepare().map_err(debug)?;
    if encoded.len() > decoder.maximum_bytes() as usize || encoded.len() > 16 * 1024 {
        return Err("reported generation exceeds the existing worker result profile".into());
    }
    decoder.decode(&encoded, contract).map_err(debug)?;
    Ok(PreparedEnvironmentRecordingHistorySource {
        frozen: encoded.into_boxed_slice(),
    })
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("reported generation freeze: {error:?}")
}
