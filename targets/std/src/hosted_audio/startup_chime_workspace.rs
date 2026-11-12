//! Partial requested-allocation term for PreparedChimeOutput::prepare only.
//! No PCM, gate, kernel/stores, canonical-offer validation, execution, device
//! discovery, or Root/IMAGE authority is represented by this number.
//!
//! String/OsString and Vec producers reserve their complete capacities before
//! writing. On qualified Linux, OsString has a byte Vec backend. Each clone of
//! a String requests its length via slice::to_vec; spare source capacity is not
//! cloned. The existing build qualification pins that slice/Vec implementation.
//! All requests are summed, including owners dropped before the result and the
//! maximum static error String, so refusal prefixes and live overlap are covered.

use super::{startup_chime, HostedPlaybackSelection};
use std::{ffi::OsString, fmt::Write, mem::size_of};

fn sum(values: &[usize]) -> Option<usize> {
    values.iter().try_fold(0usize, |a, b| a.checked_add(*b))
}
fn digits(mut n: u16) -> usize {
    let mut len = 1;
    while n >= 10 {
        n /= 10;
        len += 1;
    }
    len
}
fn endpoint_len(selection: &HostedPlaybackSelection, prefix: &str) -> Option<usize> {
    sum(&[
        prefix.len(),
        selection.observation.base_identity.len(),
        "/card-".len(),
        selection.observation.card_id.len(),
        "/device-".len(),
        digits(selection.observation.device),
    ])
}
pub(super) fn endpoint(selection: &HostedPlaybackSelection, prefix: &str) -> String {
    let mut result =
        String::with_capacity(endpoint_len(selection, prefix).expect("playback endpoint length"));
    result.push_str(prefix);
    result.push_str(&selection.observation.base_identity);
    result.push_str("/card-");
    result.push_str(&selection.observation.card_id);
    result.push_str("/device-");
    write!(&mut result, "{}", selection.observation.device)
        .expect("write playback device to String");
    result
}
fn target_len(selection: &HostedPlaybackSelection) -> Option<usize> {
    sum(&[
        "hw:CARD=".len(),
        selection.observation.card_id.len(),
        ",DEV=".len(),
        digits(selection.observation.device),
    ])
}
pub(super) fn alsa_target(selection: &HostedPlaybackSelection) -> String {
    let mut result = String::with_capacity(target_len(selection).expect("ALSA target length"));
    result.push_str("hw:CARD=");
    result.push_str(&selection.observation.card_id);
    result.push_str(",DEV=");
    write!(&mut result, "{}", selection.observation.device).expect("write ALSA device to String");
    result
}

/// Allocation-free bound for one call, derived from the borrowed selection.
/// A None return refuses qualification before entering funded preparation.
/// Missing-digest selections are included: preparation then takes an earlier
/// static-error prefix. The retained observation and caller captures need their
/// own continuing custody; only selected.clone() is charged here.
pub(crate) fn derive(selection: &HostedPlaybackSelection) -> Option<usize> {
    if !cfg!(conduit_pinned_collection_layout) {
        return None;
    }
    let endpoint = endpoint_len(selection, "std/alsa-output/")?;
    let pool = endpoint_len(selection, "std/audio/alsa/")?;
    let target = target_len(selection)?;
    let instance = endpoint.checked_add(1 + 64)?;
    let base = selection
        .source_base_id
        .as_ref()
        .map_or(endpoint, String::len);
    let mechanism = if selection.source_base_id.is_some() {
        "audio/speaker"
    } else {
        "std.base/alsa-playback@1"
    };
    // checked_provider_binding: digest hex, endpoint, instance, cloned base id,
    // implementation and mechanism. pool_id(): one temporary formatted owner.
    let provider = sum(&[
        64,
        endpoint,
        instance,
        base,
        conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION.len(),
        mechanism.len(),
        pool,
    ])?;
    // selection.clone(): four observation strings, BootId, optional Source Base.
    // All remaining fields, including cfg(test) fake behavior, are inline Copy.
    let o = &selection.observation;
    let cloned_selection = sum(&[
        o.card_id.len(),
        o.card_name.len(),
        o.device_name.len(),
        o.base_identity.len(),
        selection.boot_id.as_str().len(),
        selection.source_base_id.as_ref().map_or(0, String::len),
    ])?;
    let vector = (startup_chime::ARGUMENTS.len() + 1).checked_mul(size_of::<OsString>())?;
    let static_arguments = startup_chime::ARGUMENTS
        .iter()
        .try_fold(0usize, |n, s| n.checked_add(s.len()))?;
    // std/ffi/os_str.rs From<String> -> Buf::from_string; Linux bytes.rs
    // from_string uses String::into_bytes, moving the existing Vec without
    // cloning. The one ALSA target String allocation becomes the OsString.
    let arguments = sum(&[vector, static_arguments, target])?;
    let refusal = [
        startup_chime::MISSING_SELECTION,
        startup_chime::UNCHECKED_PROVIDER,
        startup_chime::IDENTITY_MISMATCH,
        startup_chime::WORKSPACE_DRIFT,
    ]
    .iter()
    .map(|s| s.len())
    .max()?;
    let total = sum(&[provider, cloned_selection, arguments, refusal])?;
    // Every supported producer uses a byte capacity. Vec additionally scales
    // by sizeof(OsString); reject any layout exceeding RawVec's isize bound.
    if total > isize::MAX as usize {
        None
    } else {
        Some(total)
    }
}
