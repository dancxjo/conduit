//! All canonical output buffers and schema span tables prepare before execution.
use super::*;
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as Composer,
    StructuredCompositionPreparationRefusal as PreparationRefusal,
    StructuredInfoRefusal as Refusal,
};
const UNIT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/read-unit.value"));
/// Inline state, allocator overhead and caller-owned input are separate charges.
/// Every prepared canonical output and schema field-span table is included here.
pub const READ_ENCODER_MAXIMUM_BACKING_BYTES: usize = 2
    * (READ_RESULT_BYTES
        + READ_RECORDED_BYTES
        + READ_GAP_BYTES
        + READ_GAP_REASON_BYTES
        + 2 * READ_COUNT_BYTES
        + READ_BYTES_BYTES);

pub struct Encoder {
    result: Composer,
    recorded: Composer,
    gap: Composer,
    reason: Composer,
    index: Composer,
    end: Composer,
    record: Composer,
}
impl Encoder {
    pub fn new() -> Result<Self, PreparationRefusal> {
        fn prepare(schema: &'static [u8], maximum: usize) -> Result<Composer, PreparationRefusal> {
            Composer::try_from_static_schema(schema, maximum, maximum * 2)
        }
        Ok(Self {
            result: prepare(RESULT_SCHEMA, READ_RESULT_BYTES)?,
            recorded: prepare(
                include_bytes!(concat!(env!("OUT_DIR"), "/read-recorded.type")),
                READ_RECORDED_BYTES,
            )?,
            gap: prepare(
                include_bytes!(concat!(env!("OUT_DIR"), "/read-gap.type")),
                READ_GAP_BYTES,
            )?,
            reason: prepare(
                include_bytes!(concat!(env!("OUT_DIR"), "/read-gap-reason.type")),
                READ_GAP_REASON_BYTES,
            )?,
            index: prepare(
                include_bytes!(concat!(env!("OUT_DIR"), "/read-count.type")),
                READ_COUNT_BYTES,
            )?,
            end: prepare(
                include_bytes!(concat!(env!("OUT_DIR"), "/read-count.type")),
                READ_COUNT_BYTES,
            )?,
            record: prepare(
                include_bytes!(concat!(env!("OUT_DIR"), "/read-bytes.type")),
                READ_BYTES_BYTES,
            )?,
        })
    }
    pub fn backing(&self) -> Option<usize> {
        [
            &self.result,
            &self.recorded,
            &self.gap,
            &self.reason,
            &self.index,
            &self.end,
            &self.record,
        ]
        .iter()
        .try_fold(0usize, |sum, composer| {
            sum.checked_add(composer.prepared_heap_bytes())
        })
    }
    pub fn encoded(&self) -> &[u8] {
        self.result.encoded()
    }
    pub fn encode(&mut self, outcome: ReadResult<'_>) -> Result<&[u8], Refusal> {
        match outcome {
            ReadResult::Recorded {
                index,
                captured_end,
                record,
            } => {
                if index >= captured_end || record.len() > MAXIMUM_RECORD_BYTES {
                    return Err(Refusal::MalformedCanonicalEncoding);
                }
                let end = self
                    .end
                    .leaf(&conduit_core::encode_count(u64::from(captured_end)))?;
                let index = self
                    .index
                    .leaf(&conduit_core::encode_count(u64::from(index)))?;
                let record = self.record.leaf(record)?;
                // Canonical field name order: captured-end, index, record.
                let payload =
                    self.recorded
                        .record(&[validate(end)?, validate(index)?, validate(record)?])?;
                self.result.variant("recorded", validate(payload)?)
            }
            ReadResult::Gap {
                index,
                captured_end,
                reason,
            } => {
                if index >= captured_end {
                    return Err(Refusal::MalformedCanonicalEncoding);
                }
                let end = self
                    .end
                    .leaf(&conduit_core::encode_count(u64::from(captured_end)))?;
                let index = self
                    .index
                    .leaf(&conduit_core::encode_count(u64::from(index)))?;
                let tag = match reason {
                    GapReason::Vacant => "vacant",
                    GapReason::UnpublishedOrDamaged => "unpublished-or-damaged",
                    GapReason::Corrupt => "corrupt",
                };
                let reason = self.reason.variant(tag, validate(UNIT)?)?;
                let payload =
                    self.gap
                        .record(&[validate(end)?, validate(index)?, validate(reason)?])?;
                self.result.variant("gap", validate(payload)?)
            }
            ReadResult::End(end) => {
                let payload = self.end.leaf(&conduit_core::encode_count(u64::from(end)))?;
                self.result.variant("end", validate(payload)?)
            }
            outcome => {
                let tag = match outcome {
                    ReadResult::Cancelled => "cancelled",
                    ReadResult::Busy => "busy",
                    ReadResult::RecoveryRequired => "recovery-required",
                    ReadResult::Unavailable => "unavailable",
                    _ => unreachable!(),
                };
                self.result.variant(tag, validate(UNIT)?)
            }
        }
    }
}
