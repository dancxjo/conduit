//! Exact bounded abnormal-terminal information for Text data operations.

use crate::{DataLoadTextTerminal, DataSaveTextTerminal};

pub const DATA_SAVE_TEXT_TERMINAL_INFO_ID: &str = "data/save-text-terminal@1";
pub const DATA_LOAD_TEXT_TERMINAL_INFO_ID: &str = "data/load-text-terminal@1";
pub const DATA_TEXT_TERMINAL_ENCODED_LEN: usize = 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataTerminalCodecRefusal {
    WrongLength { actual: usize },
    InvalidTag { actual: u8 },
}

impl DataSaveTextTerminal {
    pub const fn encode(self) -> [u8; DATA_TEXT_TERMINAL_ENCODED_LEN] {
        [match self {
            Self::ValueTooLarge => 0,
            Self::GenerationCapacityExhausted => 1,
            Self::ByteCapacityExhausted => 2,
            Self::WrongContentKind => 3,
        }]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, DataTerminalCodecRefusal> {
        let [tag] = encoded else {
            return Err(DataTerminalCodecRefusal::WrongLength {
                actual: encoded.len(),
            });
        };
        match tag {
            0 => Ok(Self::ValueTooLarge),
            1 => Ok(Self::GenerationCapacityExhausted),
            2 => Ok(Self::ByteCapacityExhausted),
            3 => Ok(Self::WrongContentKind),
            actual => Err(DataTerminalCodecRefusal::InvalidTag { actual: *actual }),
        }
    }
}

impl DataLoadTextTerminal {
    pub const fn encode(self) -> [u8; DATA_TEXT_TERMINAL_ENCODED_LEN] {
        [match self {
            Self::MalformedReference => 0,
            Self::WrongContentKind => 1,
            Self::WrongAccessClass => 2,
            Self::ExpiringGeneration => 3,
            Self::ItemExtent => 4,
            Self::GenerationNotRetained => 5,
            Self::ExtentMismatch => 6,
        }]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, DataTerminalCodecRefusal> {
        let [tag] = encoded else {
            return Err(DataTerminalCodecRefusal::WrongLength {
                actual: encoded.len(),
            });
        };
        match tag {
            0 => Ok(Self::MalformedReference),
            1 => Ok(Self::WrongContentKind),
            2 => Ok(Self::WrongAccessClass),
            3 => Ok(Self::ExpiringGeneration),
            4 => Ok(Self::ItemExtent),
            5 => Ok(Self::GenerationNotRetained),
            6 => Ok(Self::ExtentMismatch),
            actual => Err(DataTerminalCodecRefusal::InvalidTag { actual: *actual }),
        }
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
