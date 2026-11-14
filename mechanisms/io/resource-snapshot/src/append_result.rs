//! Exact generic typed outcome; immutable schemas are linked once, not decoded in Play.
include!(concat!(env!("OUT_DIR"), "/append-profile.rs"));
pub(crate) const RESULT_SCHEMA: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/append-result.type"));
pub fn result_type() -> Result<conduit_core::StructuredInfoType, conduit_core::StructuredInfoRefusal>
{
    conduit_core::StructuredInfoType::from_canonical_bytes(RESULT_SCHEMA)
}

/// Descriptive media outcome. Decoding grants no storage or acknowledgment authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodedAppendResult {
    Recorded(u32),
    Capacity,
    Uncertain,
    RecoveryRequired,
}
/// Validate the exact linked canonical Type without allocating a schema tree.
pub fn decode(bytes: &[u8]) -> Result<DecodedAppendResult, conduit_core::StructuredInfoRefusal> {
    use conduit_core::{BorrowedStructuredValueValidator, StructuredInfoRefusal as R};
    let validator =
        BorrowedStructuredValueValidator::new(RESULT_SCHEMA, APPEND_RESULT_BYTES as usize)?;
    let value = validator.validate(bytes)?;
    Ok(match value.variant_tag()? {
        "recorded" => {
            let payload = value.variant_payload("recorded")?.ok_or(R::WrongType)?;
            let index = conduit_core::decode_count(payload.primitive_bytes("value/u64")?)
                .map_err(|_| R::MalformedCanonicalEncoding)?;
            DecodedAppendResult::Recorded(
                u32::try_from(index).map_err(|_| R::MalformedCanonicalEncoding)?,
            )
        }
        "capacity" => DecodedAppendResult::Capacity,
        "uncertain" => DecodedAppendResult::Uncertain,
        "recovery-required" => DecodedAppendResult::RecoveryRequired,
        _ => return Err(R::WrongType),
    })
}
