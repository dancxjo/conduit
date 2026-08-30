//! Tooling-only receipt for the bounded receive-only electrical diagnostic.

pub const RX_DIAGNOSTIC_REQUEST: &[u8; 8] = b"RXDIAG01";
pub const RX_DIAGNOSTIC_SAMPLES: u16 = 2_048;
pub const RX_DIAGNOSTIC_DURATION_US: u32 = 2_048;
pub const RX_DIAGNOSTIC_RECEIPT_BYTES: usize = 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RxDiagnosticEvidence {
    pub high_samples: u16,
    pub low_samples: u16,
    pub transitions: u16,
}

impl RxDiagnosticEvidence {
    pub const fn new() -> Self {
        Self {
            high_samples: 0,
            low_samples: 0,
            transitions: 0,
        }
    }

    pub fn push(&mut self, high: bool, previous: Option<bool>) {
        if previous.is_some_and(|previous| previous != high) {
            self.transitions = self.transitions.saturating_add(1);
        }
        if high {
            self.high_samples = self.high_samples.saturating_add(1);
        } else {
            self.low_samples = self.low_samples.saturating_add(1);
        }
    }

    pub fn encode(self) -> [u8; RX_DIAGNOSTIC_RECEIPT_BYTES] {
        let mut bytes = [0; RX_DIAGNOSTIC_RECEIPT_BYTES];
        bytes[..8].copy_from_slice(b"CNDRX001");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(RX_DIAGNOSTIC_RECEIPT_BYTES as u16).to_le_bytes());
        bytes[12..14].copy_from_slice(&RX_DIAGNOSTIC_SAMPLES.to_le_bytes());
        bytes[14..16].copy_from_slice(&self.high_samples.to_le_bytes());
        bytes[16..18].copy_from_slice(&self.low_samples.to_le_bytes());
        bytes[18..20].copy_from_slice(&self.transitions.to_le_bytes());
        bytes[20..24].copy_from_slice(&RX_DIAGNOSTIC_DURATION_US.to_le_bytes());
        // These are mechanism facts, not inferred observations: D1 is input,
        // USART1 is disabled, and this image contains no Create transmit call.
        bytes[24] = 0;
        bytes[25] = 0;
        bytes[26..28].copy_from_slice(&0_u16.to_le_bytes());
        bytes
    }
}

impl Default for RxDiagnosticEvidence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_accounting_and_isolation_facts_are_encoded_once() {
        let mut evidence = RxDiagnosticEvidence::new();
        evidence.push(true, None);
        evidence.push(false, Some(true));
        let bytes = evidence.encode();
        assert_eq!(&bytes[..8], b"CNDRX001");
        assert_eq!(u16::from_le_bytes([bytes[14], bytes[15]]), 1);
        assert_eq!(u16::from_le_bytes([bytes[16], bytes[17]]), 1);
        assert_eq!(u16::from_le_bytes([bytes[18], bytes[19]]), 1);
        assert_eq!(&bytes[24..28], &[0, 0, 0, 0]);
    }
}
