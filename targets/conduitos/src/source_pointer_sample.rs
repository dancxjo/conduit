//! Mechanical conversion of a checked Source sample into portable pointer input.
//! Report interpretation, motion scaling and history remain in Source.
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType,
    validate_canonical_structured_value,
};
use conduit_semantic_catalog::NormalizedPointerSample;

pub struct SourcePointerSampleDecoder {
    validator: PreparedStructuredValueValidator,
}

impl SourcePointerSampleDecoder {
    /// Root supplies the exact selected checked sample schema before Play.
    pub fn prepare(schema: &StructuredInfoType) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: PreparedStructuredValueValidator::new(schema, 4096)?,
        })
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<NormalizedPointerSample, StructuredInfoRefusal> {
        self.validator.validate(bytes)?;
        let value = validate_canonical_structured_value(bytes)?;
        let integer = |name, kind| -> Result<[u8; 8], StructuredInfoRefusal> {
            value
                .record_field(name)?
                .ok_or(StructuredInfoRefusal::WrongRecordFields)?
                .primitive_bytes(kind)?
                .try_into()
                .map_err(|_| StructuredInfoRefusal::WrongType)
        };
        let pressed = value
            .record_field("primary-pressed")?
            .ok_or(StructuredInfoRefusal::WrongRecordFields)?
            .primitive_bytes("value/bool")?;
        let primary_pressed = match pressed {
            [0] => false,
            [1] => true,
            _ => return Err(StructuredInfoRefusal::WrongType),
        };
        Ok(NormalizedPointerSample {
            position_x: i64::from_le_bytes(integer("position-x", "value/i64")?),
            position_y: i64::from_le_bytes(integer("position-y", "value/i64")?),
            delta_x: i64::from_le_bytes(integer("delta-x", "value/i64")?),
            delta_y: i64::from_le_bytes(integer("delta-y", "value/i64")?),
            primary_pressed,
            coalesced: u64::from_le_bytes(integer("coalesced", "value/u64")?),
            dropped: u64::from_le_bytes(integer("dropped", "value/u64")?),
            queue_capacity: u64::from_le_bytes(integer("queue-capacity", "value/u64")?),
            sequence: u64::from_le_bytes(integer("sequence", "value/u64")?),
        })
    }
}
