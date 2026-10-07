//! One entry performs the selected keyboard chain's two pure implementations.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyboardChainError {
    InputRefused,
    Execution(MachineRunError),
}
impl From<MachineRunError> for KeyboardChainError {
    fn from(error: MachineRunError) -> Self {
        Self::Execution(error)
    }
}

pub(crate) struct PureKeyboardOutput {
    source: [u8; 4],
    source_length: usize,
    upper: UppercaseText,
}
impl PureKeyboardOutput {
    pub fn source(&self) -> &[u8] {
        &self.source[..self.source_length]
    }
    pub fn upper(&self) -> &[u8] {
        self.upper.as_bytes()
    }
}

impl<I: TextOwner> ProtectedText<I> {
    pub fn keymap_chain(
        &mut self,
        input: &[u8],
    ) -> Result<Option<PureKeyboardOutput>, KeyboardChainError> {
        self.region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .keymap_chain_input(input)
            .map_err(MachineRunError::ProtectionDomain)?;
        self.return_from_pure()?;
        let backend = self
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?;
        match backend.status() {
            0 => {}
            1 => return Err(KeyboardChainError::InputRefused),
            2 => return Err(MachineRunError::TextOutputOverflow.into()),
            _ => return Err(MachineRunError::KernelFailure.into()),
        }
        let mut result = PureKeyboardOutput {
            source: [0; 4],
            source_length: 0,
            upper: UppercaseText {
                bytes: [0; MAXIMUM_BYTES],
                len: 0,
            },
        };
        result.source_length = backend
            .intermediate(&mut result.source)
            .map_err(MachineRunError::ProtectionDomain)?;
        result.upper.len = backend
            .output(&mut result.upper.bytes)
            .map_err(MachineRunError::ProtectionDomain)?;
        let source_valid = core::str::from_utf8(result.source())
            .is_ok_and(|source| source.chars().count() == usize::from(result.source_length != 0));
        if !source_valid
            || core::str::from_utf8(result.upper()).is_err()
            || (result.source_length == 0) != (result.upper.len == 0)
        {
            self.region.fault(
                crate::protected_region::DomainFault::InvalidGate,
                &mut self.capabilities,
            );
            return Err(MachineRunError::ProtectionFault(
                crate::protected_region::DomainFault::InvalidGate,
            )
            .into());
        }
        Ok((result.source_length != 0).then_some(result))
    }
}
