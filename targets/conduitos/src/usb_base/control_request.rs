//! Finite raw control-transfer input, independent of device/class opcodes.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlRequestRefusal {
    DataEnvelope,
    UnexpectedOutputData,
    OutputLengthMismatch,
}

/// One immutable setup word and a borrowed, finite output payload. The setup
/// octets stay opaque except for direction/length needed by transfer machinery.
/// No address, slot, controller, endpoint possession or authority is carried.
pub struct ControlTransferRequest<'a> {
    setup: [u8; 8],
    output: &'a [u8],
}

impl<'a> ControlTransferRequest<'a> {
    /// Validate data geometry before dispatch. `maximum_data_bytes` must be
    /// constrained again by the actual admitted native buffer owner; this
    /// value is a request check, not a constructible resource grant.
    pub fn new(
        setup: [u8; 8],
        output: &'a [u8],
        maximum_data_bytes: u16,
    ) -> Result<Self, ControlRequestRefusal> {
        let request = Self { setup, output };
        if request.length() > maximum_data_bytes {
            return Err(ControlRequestRefusal::DataEnvelope);
        }
        if request.input() {
            if !output.is_empty() {
                return Err(ControlRequestRefusal::UnexpectedOutputData);
            }
        } else if output.len() != usize::from(request.length()) {
            return Err(ControlRequestRefusal::OutputLengthMismatch);
        }
        Ok(request)
    }

    pub const fn setup(&self) -> &[u8; 8] {
        &self.setup
    }

    pub const fn output(&self) -> &'a [u8] {
        self.output
    }

    pub const fn input(&self) -> bool {
        self.setup[0] & 128 != 0
    }

    pub const fn length(&self) -> u16 {
        u16::from_le_bytes([self.setup[6], self.setup[7]])
    }
}

#[cfg(test)]
mod tests;
