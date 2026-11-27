//! Exact canonical representation; field ceilings are enforced by the codec.
use super::Refusal;
use alloc::vec::Vec;
use conduit_core::{StructuredFieldType as Field, StructuredInfoType as Type};

pub const INFO_ID: &str = "device/dht11-reported-generation@1";
pub const TEXT_BYTES: usize = 128;
pub const MAXIMUM_GENERATIONS: u16 = 64;
pub const MAXIMUM_RECOVERED_SAMPLES: u32 =
    MAXIMUM_GENERATIONS as u32 * crate::recording::MAXIMUM_CHUNK_SAMPLES as u32;
pub(super) const TEXT_FIELDS: [&str; 12] = [
    "producer-host",
    "producer-boot",
    "producer-plan",
    "producer-play",
    "temperature-source",
    "temperature-source-name",
    "temperature-channel",
    "temperature-channel-name",
    "humidity-source",
    "humidity-source-name",
    "humidity-channel",
    "humidity-channel-name",
];
pub(super) const SCALARS: [(&str, &str, usize); 9] = [
    ("recovered-chunks", "value/u16", 2),
    ("recovered-samples", "value/u32", 4),
    ("recovered-first-sequence", "value/u64", 8),
    ("recovered-last-sequence", "value/u64", 8),
    ("samples-outside-view", "value/u32", 4),
    ("preceding-history-known", "value/bool", 1),
    ("predecessor-known", "value/bool", 1),
    ("predecessor-sequence", "value/u64", 8),
    ("predecessor-boot-ms", "value/u64", 8),
];
pub fn value_type() -> Result<Type, Refusal> {
    let mut fields = Vec::with_capacity(TEXT_FIELDS.len() + SCALARS.len() + 1);
    for name in TEXT_FIELDS.into_iter().chain(["original-recording"]) {
        fields.push(Field::new(name, Type::leaf("value/text".into())?)?);
    }
    for (name, kind, _) in SCALARS {
        fields.push(Field::new(name, Type::leaf(kind.into())?)?);
    }
    Ok(Type::record(INFO_ID.into(), fields)?)
}
/// Type prefix, record framing and exact bounded leaf payload/framing. This is
/// independent of any worker, file or Face envelope; no cap is enlarged to fit it.
pub fn maximum_canonical_bytes() -> Result<u32, Refusal> {
    let prefix = value_type()?.canonical_bytes()?.len();
    let mut bytes = prefix.checked_add(5).ok_or(Refusal::Capacity)?;
    for (name, maximum) in TEXT_FIELDS
        .into_iter()
        .map(|name| (name, TEXT_BYTES))
        .chain([("original-recording", crate::recording::MAXIMUM_CHUNK_BYTES)])
        .chain(SCALARS.into_iter().map(|(name, _, bytes)| (name, bytes)))
    {
        bytes = bytes
            .checked_add(4 + name.len() + 5)
            .and_then(|n| n.checked_add(maximum))
            .ok_or(Refusal::Capacity)?;
    }
    if bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(Refusal::Capacity);
    }
    u32::try_from(bytes).map_err(|_| Refusal::Capacity)
}

pub(super) fn text_valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= TEXT_BYTES && !value.chars().any(char::is_control)
}
