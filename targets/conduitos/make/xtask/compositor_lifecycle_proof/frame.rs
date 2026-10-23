//! Existing successful-ACK frame line; no frame or guest execution is synthesized.
use super::{AckRef, Refusal};
pub(super) const PREFIX: &str = "CONDUIT_COMPOSITOR_SERVICE_FRAME ";
#[derive(Debug)]
pub(super) struct Frame {
    pub ack: AckRef,
    pub preceding_records: usize,
    pub counters: [u64; 4],
}
pub(super) fn parse(line: &str, preceding_records: usize) -> Result<Frame, Refusal> {
    const KEYS: [&str; 10] = [
        "service_instance",
        "namespace",
        "frame_sequence",
        "guest_sequence",
        "pixels_written",
        "damage_count",
        "entries",
        "privilege_transitions",
        "address_space_switches",
        "scheduler_returns",
    ];
    let mut values = [None; 10];
    for field in line
        .strip_prefix(PREFIX)
        .ok_or(Refusal::Frame)?
        .split_whitespace()
    {
        let (key, number) = field.split_once('=').ok_or(Refusal::Frame)?;
        let index = KEYS
            .iter()
            .position(|value| *value == key)
            .ok_or(Refusal::Frame)?;
        if values[index].is_some()
            || number.is_empty()
            || !number.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(Refusal::Frame);
        }
        values[index] = Some(number.parse::<u64>().map_err(|_| Refusal::Frame)?);
    }
    if values.iter().any(Option::is_none) {
        return Err(Refusal::Frame);
    }
    let values = values.map(|value| value.unwrap());
    if values[..6].contains(&0) || values[4] > u64::from(u32::MAX) || values[5] > u64::from(u8::MAX)
    {
        return Err(Refusal::Frame);
    }
    Ok(Frame {
        preceding_records,
        counters: [values[6], values[7], values[8], values[9]],
        ack: AckRef {
            instance: values[0],
            namespace: values[1],
            frame_sequence: values[2],
            guest_sequence: values[3],
        },
    })
}
