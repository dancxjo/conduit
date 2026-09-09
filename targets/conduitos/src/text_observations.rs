//! Latest values seen at the admitted text host-operation boundary.
use crate::composition::MachineRunError;

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct TextObservations {
    pub upper_input: ObservedText,
    pub upper_output: ObservedText,
    pub presentation_input: ObservedText,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedText {
    bytes: [u8; conduit_text::MAX_TEXT_BYTES as usize],
    len: Option<u16>,
}

impl Default for ObservedText {
    fn default() -> Self {
        Self {
            bytes: [0; conduit_text::MAX_TEXT_BYTES as usize],
            len: None,
        }
    }
}

impl ObservedText {
    pub(super) fn set(&mut self, value: &[u8]) -> Result<(), MachineRunError> {
        if value.len() > self.bytes.len() {
            return Err(MachineRunError::TextOutputOverflow);
        }
        core::str::from_utf8(value).map_err(|_| MachineRunError::TextMalformedUtf8)?;
        self.bytes[..value.len()].copy_from_slice(value);
        self.len = Some(value.len() as u16);
        Ok(())
    }

    pub fn text(&self) -> Option<&str> {
        self.len
            .and_then(|len| core::str::from_utf8(&self.bytes[..usize::from(len)]).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absence_empty_value_and_refused_values_stay_distinct() {
        let mut observed = ObservedText::default();
        assert_eq!(observed.text(), None);
        observed.set(b"").unwrap();
        assert_eq!(observed.text(), Some(""));
        observed.set(b"observed").unwrap();
        assert!(observed.set(&[0xff]).is_err());
        assert!(
            observed
                .set(&[b'x'; conduit_text::MAX_TEXT_BYTES as usize + 1])
                .is_err()
        );
        assert_eq!(observed.text(), Some("observed"));
    }
}
