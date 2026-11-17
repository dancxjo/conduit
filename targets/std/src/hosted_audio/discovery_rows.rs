//! The shared borrowed grammar for playback listings; no observations allocated.
use std::fmt;
pub(super) struct PlaybackRow<'a> {
    pub card_index: u16,
    pub card_id: &'a str,
    pub card_name: &'a str,
    pub device: u16,
    pub device_name: &'a str,
}
#[derive(Debug)]
pub(super) struct RowError<'a> {
    reason: &'static str,
    value: &'a str,
}
impl fmt::Display for RowError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} in {:?}", self.reason, self.value)
    }
}
pub(super) fn playback_rows(
    listing: &str,
) -> impl Iterator<Item = Result<PlaybackRow<'_>, RowError<'_>>> {
    listing
        .lines()
        .filter(|line| line.starts_with("card "))
        .map(parse_row)
}
fn parse_row(line: &str) -> Result<PlaybackRow<'_>, RowError<'_>> {
    let error = |reason| RowError {
        reason,
        value: line,
    };
    let (card, device) = line
        .split_once(", device ")
        .ok_or_else(|| error("missing device delimiter"))?;
    let card = card.strip_prefix("card ").expect("filtered card row");
    let (index, tail) = card
        .split_once(": ")
        .ok_or_else(|| error("missing card delimiter"))?;
    let (card_id, card_name) = bracketed_name(tail)?;
    let (device, tail) = device
        .split_once(": ")
        .ok_or_else(|| error("missing device name"))?;
    let (_, device_name) = bracketed_name(tail)?;
    Ok(PlaybackRow {
        card_index: index.parse().map_err(|_| error("invalid card index"))?,
        card_id,
        card_name,
        device: device.parse().map_err(|_| error("invalid device index"))?,
        device_name,
    })
}
fn bracketed_name(value: &str) -> Result<(&str, &str), RowError<'_>> {
    let open = value.find('[').ok_or(RowError {
        reason: "missing '['",
        value,
    })?;
    let close = value[open + 1..]
        .find(']')
        .map(|i| open + 1 + i)
        .ok_or(RowError {
            reason: "missing ']'",
            value,
        })?;
    Ok((value[..open].trim(), value[open + 1..close].trim()))
}
