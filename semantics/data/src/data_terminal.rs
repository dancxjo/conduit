//! Exact bounded abnormal-terminal information for Text data operations.

use crate::{
    DataLoadTextTerminal, DataLoadTextTerminalForm, DataSaveTextTerminal, DataSaveTextTerminalForm,
};

pub const DATA_SAVE_TEXT_TERMINAL_INFO_ID: &str = DataSaveTextTerminalForm::IDENTITY;
pub const DATA_LOAD_TEXT_TERMINAL_INFO_ID: &str = DataLoadTextTerminalForm::IDENTITY;
pub const DATA_TEXT_TERMINAL_ENCODED_LEN: usize = DataSaveTextTerminalForm::EXACT_BYTES;
pub use conduit_plot::rust_binding::NativeFormRefusal as DataTerminalCodecRefusal;

impl DataSaveTextTerminal {
    pub const fn encode(self) -> [u8; DATA_TEXT_TERMINAL_ENCODED_LEN] {
        DataSaveTextTerminalForm::encode(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, DataTerminalCodecRefusal> {
        DataSaveTextTerminalForm::decode(encoded)
    }
}

impl DataLoadTextTerminal {
    pub const fn encode(self) -> [u8; DATA_TEXT_TERMINAL_ENCODED_LEN] {
        DataLoadTextTerminalForm::encode(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, DataTerminalCodecRefusal> {
        DataLoadTextTerminalForm::decode(encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_terminal_is_one_exact_byte() {
        let outcomes = [
            DataSaveTextTerminal::ValueTooLarge,
            DataSaveTextTerminal::GenerationCapacityExhausted,
            DataSaveTextTerminal::ByteCapacityExhausted,
            DataSaveTextTerminal::WrongContentKind,
        ];
        for outcome in outcomes {
            assert_eq!(DataSaveTextTerminal::decode(&outcome.encode()), Ok(outcome));
        }
        assert!(DataSaveTextTerminal::decode(&[]).is_err());
        assert!(DataSaveTextTerminal::decode(&[0, 0]).is_err());
        assert!(DataSaveTextTerminal::decode(&[4]).is_err());
    }

    #[test]
    fn load_terminal_is_one_exact_byte() {
        let outcomes = [
            DataLoadTextTerminal::MalformedReference,
            DataLoadTextTerminal::WrongContentKind,
            DataLoadTextTerminal::WrongAccessClass,
            DataLoadTextTerminal::ExpiringGeneration,
            DataLoadTextTerminal::ItemExtent,
            DataLoadTextTerminal::GenerationNotRetained,
            DataLoadTextTerminal::ExtentMismatch,
        ];
        for outcome in outcomes {
            assert_eq!(DataLoadTextTerminal::decode(&outcome.encode()), Ok(outcome));
        }
        assert!(DataLoadTextTerminal::decode(&[]).is_err());
        assert!(DataLoadTextTerminal::decode(&[0, 0]).is_err());
        assert!(DataLoadTextTerminal::decode(&[7]).is_err());
    }
}
