//! Fixed Morse window; existing text copy bounds and ABI offsets stay intact.
use super::*;
use frame::MORSE_CAPACITY;

impl TextDomain {
    pub fn morse_chain_input(&mut self, input: &[u8], unit: u16) -> Result<(), DomainRefusal> {
        self.input(input)?;
        let frame = self.space.frame();
        frame.command = 8;
        frame.unit_millis = u32::from(unit);
        frame.morse.fill(0);
        frame.morse_length = 0;
        frame.morse_status = 0;
        Ok(())
    }
    pub fn morse_output(
        &mut self,
        output: &mut [u8; MORSE_CAPACITY],
    ) -> Result<(usize, u32), DomainRefusal> {
        let frame = self.space.frame();
        let length = frame.morse_length as usize;
        if self.quarantined
            || frame.command != 8
            || frame.status > 2
            || length > MORSE_CAPACITY
            || frame.morse_status > 10
            || (frame.morse_status == 0 && length == 0)
            || (frame.morse_status != 0 && length != 0)
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        output[..length].copy_from_slice(&frame.morse[..length]);
        self.cost.copied_bytes += length as u64;
        self.cost.shared_peak_bytes = self.cost.shared_peak_bytes.max(
            frame.input_length.min(TEXT_CAPACITY as u32)
                + frame.output_length.min(TEXT_CAPACITY as u32)
                + length as u32,
        );
        Ok((length, frame.morse_status))
    }
    pub fn morse_presentation(&mut self, input: &[u8], handle: u64) -> Result<(), DomainRefusal> {
        if input.len() > MORSE_CAPACITY {
            return Err(DomainRefusal::InvalidMemory);
        }
        self.input(&[])?;
        let frame = self.space.frame();
        frame.command = 9;
        frame.capacity = MORSE_CAPACITY as u32;
        frame.morse.fill(0);
        frame.morse[..input.len()].copy_from_slice(input);
        frame.morse_length = input.len() as u32;
        frame.capability = handle;
        frame.operation = crate::domain_serial_scope::SERIAL_PRESENT_OPERATION;
        frame.work_units = 1;
        self.cost.copied_bytes += input.len() as u64;
        self.cost.shared_peak_bytes = self.cost.shared_peak_bytes.max(input.len() as u32);
        Ok(())
    }
    pub fn morse_effect_request(
        &mut self,
        output: &mut [u8; MORSE_CAPACITY],
    ) -> Result<(u64, u32, u32, usize), DomainRefusal> {
        let frame = self.space.frame();
        let length = frame.morse_length as usize;
        if self.quarantined
            || frame.command != 9
            || frame.capacity != MORSE_CAPACITY as u32
            || frame.input_length != 0
            || length > MORSE_CAPACITY
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        output[..length].copy_from_slice(&frame.morse[..length]);
        self.cost.copied_bytes += length as u64;
        Ok((frame.capability, frame.operation, frame.work_units, length))
    }
}
