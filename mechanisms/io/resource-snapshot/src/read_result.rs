//! Exact borrowed media outcomes. These convey no commit or acknowledgment authority.
include!(concat!(env!("OUT_DIR"), "/read-profile.rs"));
pub(crate) const RESULT_SCHEMA: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/read-result.type"));
pub const MAXIMUM_RECORD_BYTES: usize = 200;
mod encoder;
pub use encoder::{Encoder, READ_ENCODER_MAXIMUM_BACKING_BYTES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapReason {
    Vacant,
    UnpublishedOrDamaged,
    Corrupt,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadResult<'a> {
    Recorded {
        index: u32,
        captured_end: u32,
        record: &'a [u8],
    },
    Gap {
        index: u32,
        captured_end: u32,
        reason: GapReason,
    },
    End(u32),
    Cancelled,
    Busy,
    RecoveryRequired,
    Unavailable,
}
/// Linked canonical cancellation contains no record payload and needs no allocator.
pub fn cancelled() -> &'static [u8] {
    include_bytes!(concat!(env!("OUT_DIR"), "/read-cancelled.value"))
}

pub fn decode(bytes: &[u8]) -> Result<ReadResult<'_>, conduit_core::StructuredInfoRefusal> {
    use conduit_core::{validate_canonical_structured_value, StructuredInfoRefusal as R};
    if bytes.len() > READ_RESULT_BYTES {
        return Err(R::CanonicalEncodingTooLarge);
    }
    if !bytes.starts_with(RESULT_SCHEMA) {
        return Err(R::WrongType);
    }
    // The exact linked schema is checked above. Borrow the canonical view from
    // the caller's bytes, so returned payloads outlive this function's temporary
    // validator without an owned Type tree or an unsafe lifetime extension.
    let value = validate_canonical_structured_value(bytes)?;
    let tag = value.variant_tag()?;
    let payload = value.variant_payload(tag)?.ok_or(R::WrongType)?;
    let count = |value: conduit_core::ValidatedCanonicalStructuredValue<'_>| {
        let count = conduit_core::decode_count(value.primitive_bytes("value/u64")?)
            .map_err(|_| R::MalformedCanonicalEncoding)?;
        u32::try_from(count).map_err(|_| R::MalformedCanonicalEncoding)
    };
    Ok(match tag {
        "recorded" | "gap" => {
            let field = |name| payload.record_field(name)?.ok_or(R::WrongRecordFields);
            let index = count(field("index")?)?;
            let captured_end = count(field("captured-end")?)?;
            if index >= captured_end {
                return Err(R::MalformedCanonicalEncoding);
            }
            if tag == "recorded" {
                let record = field("record")?.primitive_bytes("value/bytes")?;
                if record.len() > MAXIMUM_RECORD_BYTES {
                    return Err(R::CanonicalEncodingTooLarge);
                }
                ReadResult::Recorded {
                    index,
                    captured_end,
                    record,
                }
            } else {
                let reason = match field("reason")?.variant_tag()? {
                    "vacant" => GapReason::Vacant,
                    "unpublished-or-damaged" => GapReason::UnpublishedOrDamaged,
                    "corrupt" => GapReason::Corrupt,
                    _ => return Err(R::WrongType),
                };
                ReadResult::Gap {
                    index,
                    captured_end,
                    reason,
                }
            }
        }
        "end" => ReadResult::End(count(payload)?),
        "cancelled" => ReadResult::Cancelled,
        "busy" => ReadResult::Busy,
        "recovery-required" => ReadResult::RecoveryRequired,
        "unavailable" => ReadResult::Unavailable,
        _ => return Err(R::WrongType),
    })
}
#[cfg(test)]
#[path = "read_result/tests.rs"]
mod tests;
