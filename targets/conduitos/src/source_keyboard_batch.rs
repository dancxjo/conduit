//! Mechanical checked-Source output conversion into whole-report input ingress.
//! USB report decoding, normalization and delta meaning remain in Source.
use crate::{
    arch::HidKeyTransition,
    keyboard_input::{KeyboardIngress, KeyboardIngressRefusal},
};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType,
    validate_canonical_structured_value,
};

pub struct SourceKeyboardBatchDecoder {
    validator: PreparedStructuredValueValidator,
}

pub struct SourceKeyboardBatch {
    transitions: [HidKeyTransition; 20],
    count: usize,
}

impl SourceKeyboardBatchDecoder {
    /// Root supplies the exact selected checked output schema before Play.
    pub fn prepare(schema: &StructuredInfoType) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: PreparedStructuredValueValidator::new(schema, 4096)?,
        })
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<SourceKeyboardBatch, StructuredInfoRefusal> {
        self.validator.validate(bytes)?;
        let value = validate_canonical_structured_value(bytes)?;
        let slots = value
            .record_field("slots")?
            .ok_or(StructuredInfoRefusal::WrongRecordFields)?;
        if slots.collection_length()? != 20 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let mut batch = SourceKeyboardBatch {
            transitions: [HidKeyTransition::default(); 20],
            count: 0,
        };
        for index in 0..20 {
            let slot = slots
                .collection_index(index)?
                .ok_or(StructuredInfoRefusal::WrongType)?;
            if let Some(change) = slot.variant_payload("changed")? {
                let octet = |name| -> Result<u8, StructuredInfoRefusal> {
                    let bytes = change
                        .record_field(name)?
                        .ok_or(StructuredInfoRefusal::WrongRecordFields)?
                        .primitive_bytes("value/u8")?;
                    match bytes {
                        [value] => Ok(*value),
                        _ => Err(StructuredInfoRefusal::WrongType),
                    }
                };
                let pressed = change
                    .record_field("pressed")?
                    .ok_or(StructuredInfoRefusal::WrongRecordFields)?
                    .primitive_bytes("value/bool")?;
                let pressed = match pressed {
                    [0] => false,
                    [1] => true,
                    _ => return Err(StructuredInfoRefusal::WrongType),
                };
                batch.transitions[batch.count] =
                    HidKeyTransition::from_source(octet("usage")?, pressed, octet("modifiers")?);
                batch.count += 1;
            }
        }
        Ok(batch)
    }
}

impl SourceKeyboardBatch {
    pub fn transitions(&self) -> &[HidKeyTransition] {
        &self.transitions[..self.count]
    }

    /// Existing ingress validates and admits the complete batch atomically.
    pub fn admit(&self, ingress: &mut KeyboardIngress) -> Result<(), KeyboardIngressRefusal> {
        ingress.admit_report(self.transitions())
    }
}
