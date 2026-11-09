//! Standard Core canonical composition with fully prepared finite storage.
use super::*;
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer,
    StructuredCompositionPreparationRefusal, StructuredInfoRefusal,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppendOutcome {
    Recorded(DurableRecordIndex),
    Capacity,
    Uncertain,
    RecoveryRequired,
}
const UNIT_VALUE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/append-unit.value"));
/// Finite pre-admission payload ceiling for canonical encoder buffers/span tables.
/// Identity strings, inline state and allocator overhead are separate charges.
/// A future larger Core layout refuses preparation rather than enlarging this.
pub const APPEND_ENCODER_MAXIMUM_BACKING_BYTES: usize =
    APPEND_RESULT_BYTES as usize * 2 + crate::append_result::APPEND_INDEX_BYTES;
pub(super) struct Encoder {
    result: PreparedStructuredComposer,
    index: PreparedStructuredComposer,
}
impl Encoder {
    pub fn new() -> Result<Self, StructuredCompositionPreparationRefusal> {
        Ok(Self {
            result: PreparedStructuredComposer::try_from_static_schema(
                crate::append_result::RESULT_SCHEMA,
                APPEND_RESULT_BYTES as usize,
                APPEND_RESULT_BYTES as usize * 2,
            )?,
            index: PreparedStructuredComposer::try_from_static_schema(
                include_bytes!(concat!(env!("OUT_DIR"), "/append-index.type")),
                crate::append_result::APPEND_INDEX_BYTES,
                crate::append_result::APPEND_INDEX_BYTES,
            )?,
        })
    }
    pub fn backing(&self) -> Option<usize> {
        self.result
            .prepared_heap_bytes()
            .checked_add(self.index.prepared_heap_bytes())
    }
    pub fn encoded(&self) -> &[u8] {
        self.result.encoded()
    }
    pub fn encode(&mut self, outcome: AppendOutcome) -> Result<&[u8], StructuredInfoRefusal> {
        let (tag, payload) = match outcome {
            AppendOutcome::Recorded(index) => (
                "recorded",
                self.index.leaf(&c::encode_count(u64::from(index.0)))?,
            ),
            AppendOutcome::Capacity => ("capacity", UNIT_VALUE),
            AppendOutcome::Uncertain => ("uncertain", UNIT_VALUE),
            AppendOutcome::RecoveryRequired => ("recovery-required", UNIT_VALUE),
        };
        self.result.variant(tag, validate(payload)?)
    }
}
