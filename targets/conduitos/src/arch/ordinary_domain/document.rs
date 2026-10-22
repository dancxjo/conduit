//! Bounded document value copies; authoritative bytes stay in the private owner.
use super::*;
use conduit_semantic_catalog::{TextDocument, TextEditRefusal};

impl TextDomain {
    pub fn document_input(&mut self, operation: u32, input: &[u8]) -> Result<(), DomainRefusal> {
        if !self.editor_initialized
            || operation > 1
            || input.len() > frame::DOCUMENT_EDIT_CAPACITY
            || (operation == 0 && !input.is_empty())
        {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        self.input(&[])?;
        let frame = self.space.frame();
        frame.command = 14;
        frame.operation = operation;
        frame.document_input.fill(0);
        frame.document_output.fill(0);
        frame.document_input[..input.len()].copy_from_slice(input);
        frame.document_input_length = input.len() as u32;
        frame.document_output_length = 0;
        frame.document_refusal = 0;
        self.cost.copied_bytes += input.len() as u64;
        self.cost.shared_peak_bytes = self.cost.shared_peak_bytes.max(input.len() as u32);
        Ok(())
    }

    pub fn document_output(
        &mut self,
        operation: u32,
        output: &mut [u8; frame::DOCUMENT_CAPACITY],
    ) -> Result<Result<usize, TextEditRefusal>, DomainRefusal> {
        let frame = self.space.frame();
        if self.quarantined
            || !self.editor_initialized
            || frame.command != 14
            || frame.operation != operation
            || frame.capacity != TEXT_CAPACITY as u32
            || frame.document_input_length as usize > frame::DOCUMENT_EDIT_CAPACITY
            || (operation == 0 && frame.document_input_length != 0)
            || frame.input_length != 0
            || frame.output_length != 0
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        if frame.status == 1 {
            if frame.document_output_length != 0 {
                return Err(DomainRefusal::InvalidMemory);
            }
            let refusal = match frame.document_refusal {
                1 => TextEditRefusal::StaleRevision,
                2 => TextEditRefusal::InvalidEncoding,
                3 => TextEditRefusal::InvalidRange,
                4 => TextEditRefusal::InvalidUtf8,
                5 => TextEditRefusal::CapacityExhausted,
                6 => TextEditRefusal::RevisionExhausted,
                7 => TextEditRefusal::InvalidStorage,
                8 => TextEditRefusal::UnsupportedMode,
                _ => return Err(DomainRefusal::InvalidMemory),
            };
            return Ok(Err(refusal));
        }
        let length = frame.document_output_length as usize;
        if frame.status != 0 || frame.document_refusal != 0 || length > frame::DOCUMENT_CAPACITY {
            return Err(DomainRefusal::InvalidMemory);
        }
        let bytes = &frame.document_output[..length];
        TextDocument::decode(bytes).map_err(|_| DomainRefusal::InvalidMemory)?;
        output[..length].copy_from_slice(bytes);
        self.cost.copied_bytes += length as u64;
        self.cost.shared_peak_bytes = self
            .cost
            .shared_peak_bytes
            .max(frame.document_input_length + frame.document_output_length);
        Ok(Ok(length))
    }
}
