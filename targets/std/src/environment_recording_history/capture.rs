//! Preserve one checked original recording generation, never a generic JSON fallback.
use super::CheckedEnvironmentRecordingHistory;
use crate::environment_recording_history::{
    freeze_generation, freeze_preparation_backing_bound,
    selected_capture_preparation_backing_bound, validate_capture_selection,
    CapturedSnapshotSelection, PreparedEnvironmentRecordingHistorySource,
};
use conduit_dht11_observation::{
    recording::RecordingContext,
    reported_generation::{PreparedReportedRecordingGeneration, ReportedGenerationChannel},
    ObservationContract,
};
use conduit_linux_snapshot_store::PreparedFileSnapshot;

/// Requested preparation backing, not allocator RSS. The caller separately funds
/// the already checked ObservationContract and native worker stack/control/buffers.
pub fn provider_preparation_bytes(contract: &ObservationContract) -> Result<usize, String> {
    let freeze = freeze_preparation_backing_bound(contract)?;
    selected_capture_preparation_backing_bound()?
        .checked_add(
            PreparedReportedRecordingGeneration::preparation_backing_bound(contract)
                .ok_or("environment recording converter preparation backing overflow")?,
        )
        .and_then(|n| n.checked_add(4 * 128))
        .and_then(|n| {
            n.checked_add(core::mem::size_of::<
                PreparedEnvironmentRecordingHistorySource,
            >())
        })
        .and_then(|n| n.checked_add(freeze))
        .ok_or_else(|| "environment recording capture preparation backing overflow".into())
}

/// Call inside the funded ImageWorkerFactory closure. Caller establishes exact
/// current Host/Boot correspondence and separately retains the read grants.
/// Reported producer IDs select canonical recording context, never IO authority.
pub fn prepare_capture(
    contract: &ObservationContract,
    parameters: &CheckedEnvironmentRecordingHistory<'_>,
    selection: CapturedSnapshotSelection<'_>,
) -> Result<PreparedEnvironmentRecordingHistorySource, String> {
    validate_capture_selection(&selection)?;
    let reference = selection.reference;
    if reference.content_profile.as_str() != "text/json-utf8@1"
        || reference.access_class.as_str() != "resource/snapshot@1"
        || !(1..=4096).contains(&reference.extent.bytes)
        || reference.extent.items != Some(1)
        || reference.lifetime.expires_at.is_some()
        || parameters
            .reference
            .as_ref()
            .is_some_and(|configured| configured != reference)
    {
        return Err(
            "environment recording capture requires the exact immutable JSON reference".into(),
        );
    }
    // The checked parser normally guarantees these bounds; this public borrowed
    // struct can also be assembled directly by trusted Root, so fence before clones.
    if parameters
        .producer
        .iter()
        .chain(parameters.channels.iter().flatten())
        .any(|text| text.is_empty() || text.len() > 128 || text.chars().any(char::is_control))
    {
        return Err("environment recording reported text exceeds its bounded profile".into());
    }
    let host = parameters.producer[0].into();
    let boot = parameters.producer[1].into();
    let plan = parameters.producer[2].into();
    let play = parameters.producer[3].into();
    let mut converter = PreparedReportedRecordingGeneration::prepare(
        RecordingContext {
            host: &host,
            boot: &boot,
            plan: &plan,
            play: &play,
        },
        contract,
        1,
        [
            channel(parameters.channels[0]),
            channel(parameters.channels[1]),
        ],
    )
    .map_err(debug)?;
    let mut record = PreparedFileSnapshot::prepare_selected(
        selection.directory,
        selection.selected,
        reference,
        selection.original_grant,
        selection.local_reference,
    )
    .map_err(debug)?;
    let json = core::str::from_utf8(record.read(selection.selected.authority).map_err(debug)?)
        .map_err(debug)?;
    converter.read_next(json).map_err(debug)?;
    let encoded = converter.encode().map_err(debug)?;
    // No directory, grant, context clone or decoder survives in the provider.
    freeze_generation(encoded, contract)
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("environment recording capture: {error:?}")
}

fn channel(names: [&str; 4]) -> ReportedGenerationChannel<'_> {
    ReportedGenerationChannel {
        source_identity: names[0],
        source_name: names[1],
        channel_identity: names[2],
        channel_name: names[3],
    }
}
