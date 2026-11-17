//! Exact prepared playback identity producers; no discovery or authority.
use super::HostedPlaybackSelection;
use std::fmt::Write;
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
pub(super) fn endpoint_len(selection: &HostedPlaybackSelection, prefix: &str) -> Option<usize> {
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
pub(super) fn target_len(selection: &HostedPlaybackSelection) -> Option<usize> {
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
