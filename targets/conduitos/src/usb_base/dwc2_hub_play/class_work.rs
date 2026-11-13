//! Fixed typed forwarding storage for Source-selected class work.
//! No descriptor, report, endpoint selection or continuation meaning lives here.
use alloc::vec::Vec;
use conduit_core::*;
#[derive(Debug)]
pub(super) enum Refusal {
    Binding,
    Value(StructuredInfoRefusal),
}
pub(super) enum Work<'a> {
    Descriptor(&'a ValuePayload),
    KeyboardRetained(&'a ValuePayload),
}
pub(super) struct ClassWork {
    descriptor: ValuePayload,
    continuation: ValuePayload,
    keyboard: Vec<u8>,
}
impl ClassWork {
    pub(super) fn prepare(descriptor: KindId, continuation: KindId) -> Self {
        Self {
            descriptor: ValuePayload {
                value_kind: descriptor,
                encoded: Vec::with_capacity(4096),
            },
            continuation: ValuePayload {
                value_kind: continuation,
                encoded: Vec::with_capacity(4096),
            },
            keyboard: Vec::with_capacity(4096),
        }
    }
    pub(super) fn keyboard(&self) -> Option<&[u8]> {
        (!self.keyboard.is_empty()).then_some(self.keyboard.as_slice())
    }
    pub(super) fn route(&mut self, bytes: &[u8]) -> Result<Work<'_>, Refusal> {
        let value = validate_canonical_structured_value(bytes).map_err(Refusal::Value)?;
        if let Some(descriptor) = value
            .variant_payload("descriptor")
            .map_err(Refusal::Value)?
        {
            copy_value(&mut self.descriptor.encoded, descriptor)?;
            Ok(Work::Descriptor(&self.descriptor))
        } else if let Some(keyboard) = value.variant_payload("keyboard").map_err(Refusal::Value)? {
            // Root selects at most one independently admitted keyboard; retain
            // every Source-authored continuation even after that selection.
            if self.keyboard.is_empty() {
                copy_value(&mut self.keyboard, field(keyboard, "input")?)?;
            }
            copy_value(
                &mut self.continuation.encoded,
                field(keyboard, "continuation")?,
            )?;
            Ok(Work::KeyboardRetained(&self.continuation))
        } else {
            Err(Refusal::Binding)
        }
    }
}
fn field<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>, Refusal> {
    value
        .record_field(name)
        .map_err(Refusal::Value)?
        .ok_or(Refusal::Binding)
}
fn copy_value(
    output: &mut Vec<u8>,
    value: ValidatedCanonicalStructuredValue<'_>,
) -> Result<(), Refusal> {
    if value
        .type_bytes()
        .len()
        .checked_add(value.value_node().len())
        .is_none_or(|size| size > output.capacity())
    {
        return Err(Refusal::Binding);
    }
    output.clear();
    output.extend_from_slice(value.type_bytes());
    output.extend_from_slice(value.value_node());
    Ok(())
}

#[cfg(test)]
#[path = "class_work_tests.rs"]
mod tests;
