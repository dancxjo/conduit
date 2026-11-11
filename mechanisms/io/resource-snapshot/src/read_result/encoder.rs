//! One caller-funded canonical result cache; schema and names stay linked once.
use super::*;
use conduit_core::{
    BorrowedStructuredValue as V, PreparedLinkedStructuredComposer,
    StructuredCompositionPreparationRefusal as PreparationRefusal,
    StructuredInfoRefusal as Refusal,
};
pub const READ_ENCODER_MAXIMUM_BACKING_BYTES: usize = READ_RESULT_BYTES;
pub struct Encoder {
    writer: PreparedLinkedStructuredComposer<'static>,
    output: alloc::vec::Vec<u8>,
    length: usize,
}
impl Encoder {
    pub fn new() -> Result<Self, PreparationRefusal> {
        let writer = PreparedLinkedStructuredComposer::new(RESULT_SCHEMA, READ_RESULT_BYTES)?;
        let mut output = alloc::vec::Vec::new();
        output
            .try_reserve_exact(READ_RESULT_BYTES)
            .map_err(|_| PreparationRefusal::Allocation)?;
        if output.capacity() > READ_ENCODER_MAXIMUM_BACKING_BYTES {
            return Err(PreparationRefusal::Capacity);
        }
        output.resize(READ_RESULT_BYTES, 0);
        Ok(Self {
            writer,
            output,
            length: 0,
        })
    }
    pub fn backing(&self) -> Option<usize> {
        Some(self.output.capacity())
    }
    pub fn encoded(&self) -> &[u8] {
        &self.output[..self.length]
    }
    pub fn encode(&mut self, outcome: ReadResult<'_>) -> Result<&[u8], Refusal> {
        let unit = V::Empty;
        let index = match outcome {
            ReadResult::Recorded { index, .. } | ReadResult::Gap { index, .. } => index,
            _ => 0,
        };
        let end = match outcome {
            ReadResult::Recorded { captured_end, .. } | ReadResult::Gap { captured_end, .. } => {
                captured_end
            }
            ReadResult::End(end) => end,
            _ => 0,
        };
        if matches!(
            outcome,
            ReadResult::Recorded { .. } | ReadResult::Gap { .. }
        ) && index >= end
        {
            return Err(Refusal::MalformedCanonicalEncoding);
        }
        let index = conduit_core::encode_count(u64::from(index));
        let end = conduit_core::encode_count(u64::from(end));
        let mut fields = [V::Leaf(&end), V::Leaf(&index), unit];
        let reason = match outcome {
            ReadResult::Gap {
                reason: GapReason::Vacant,
                ..
            } => "vacant",
            ReadResult::Gap {
                reason: GapReason::UnpublishedOrDamaged,
                ..
            } => "unpublished-or-damaged",
            _ => "corrupt",
        };
        let reason = V::Variant {
            tag: reason,
            payload: &unit,
        };
        let tag = match outcome {
            ReadResult::Recorded { record, .. } => {
                if record.len() > MAXIMUM_RECORD_BYTES {
                    return Err(Refusal::MalformedCanonicalEncoding);
                }
                fields[2] = V::Leaf(record);
                "recorded"
            }
            ReadResult::Gap { .. } => {
                fields[2] = reason;
                "gap"
            }
            ReadResult::End(_) => "end",
            ReadResult::Cancelled => "cancelled",
            ReadResult::Busy => "busy",
            ReadResult::RecoveryRequired => "recovery-required",
            ReadResult::Unavailable => "unavailable",
        };
        let payload = match outcome {
            ReadResult::Recorded { .. } | ReadResult::Gap { .. } => V::Record(&fields),
            ReadResult::End(_) => V::Leaf(&end),
            _ => unit,
        };
        let bytes = self.writer.encode_into(
            V::Variant {
                tag,
                payload: &payload,
            },
            &mut self.output,
        )?;
        self.length = bytes.len();
        Ok(&self.output[..self.length])
    }
}
#[cfg(test)]
#[allow(dead_code)] // Exact previous production implementation retained as test oracle only.
#[path = "encoder_legacy.rs"]
mod legacy;
#[cfg(test)]
#[path = "encoder_parity_tests.rs"]
mod tests;
