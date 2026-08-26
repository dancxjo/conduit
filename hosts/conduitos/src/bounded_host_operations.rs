//! Finite semantic host operations shared by ordinary ConduitOS planned Plays.

use conduit_core::{
    ChordInfo, ConduitIntlKeymap, InfoBool, KeyEvent, KeymapDisposition, KeymapRefusal, Scalar,
};

const OUTPUT_BYTES: usize = conduit_core::JSON_MAXIMUM_ENCODED_BYTES;
const TEXT_OUTPUT_BYTES: usize = conduit_text::MAX_TEXT_BYTES as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundedHostOperationError {
    InvalidInput,
    InvalidConfiguration,
    Overflow,
    Unsupported,
    Json(conduit_core::JsonRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedOutput {
    bytes: [u8; OUTPUT_BYTES],
    len: usize,
}

impl BoundedOutput {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn from_slice(bytes: &[u8]) -> Result<Self, BoundedHostOperationError> {
        if bytes.len() > OUTPUT_BYTES {
            return Err(BoundedHostOperationError::Overflow);
        }
        let mut output = Self {
            bytes: [0; OUTPUT_BYTES],
            len: bytes.len(),
        };
        output.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(output)
    }
}

pub struct BoundedHostOperations {
    keymap: ConduitIntlKeymap,
}

impl Default for BoundedHostOperations {
    fn default() -> Self {
        Self {
            keymap: ConduitIntlKeymap::new(),
        }
    }
}

impl BoundedHostOperations {
    pub fn reset(&mut self) {
        self.keymap.reset();
    }

    pub fn text_join(
        &self,
        prefix: &str,
        input: &[u8],
    ) -> Result<BoundedOutput, BoundedHostOperationError> {
        let text =
            core::str::from_utf8(input).map_err(|_| BoundedHostOperationError::InvalidInput)?;
        let total = prefix
            .len()
            .checked_add(text.len())
            .ok_or(BoundedHostOperationError::Overflow)?;
        if total > TEXT_OUTPUT_BYTES {
            return Err(BoundedHostOperationError::Overflow);
        }
        let mut output = BoundedOutput {
            bytes: [0; OUTPUT_BYTES],
            len: total,
        };
        output.bytes[..prefix.len()].copy_from_slice(prefix.as_bytes());
        output.bytes[prefix.len()..total].copy_from_slice(text.as_bytes());
        Ok(output)
    }

    pub fn decode_bool(&self, input: &[u8]) -> Result<bool, BoundedHostOperationError> {
        InfoBool::decode(input)
            .map(InfoBool::get)
            .map_err(|_| BoundedHostOperationError::InvalidInput)
    }

    pub fn json_encode(&self, input: &[u8]) -> Result<BoundedOutput, BoundedHostOperationError> {
        let output = conduit_core::JsonValue::decode_info(input)
            .and_then(|value| value.encode_text())
            .map_err(BoundedHostOperationError::Json)?;
        BoundedOutput::from_slice(&output)
    }

    pub fn json_decode(&self, input: &[u8]) -> Result<BoundedOutput, BoundedHostOperationError> {
        let output = conduit_core::JsonValue::decode_text(input)
            .and_then(|value| value.encode_info())
            .map_err(BoundedHostOperationError::Json)?;
        BoundedOutput::from_slice(&output)
    }

    pub fn math_scale(
        &self,
        input: &[u8],
        gain: Scalar,
    ) -> Result<BoundedOutput, BoundedHostOperationError> {
        self.math(input, |value| {
            conduit_std_catalog::scale_scalar(value, gain)
        })
    }

    pub fn math_deadband(
        &self,
        input: &[u8],
        radius: Scalar,
    ) -> Result<BoundedOutput, BoundedHostOperationError> {
        self.math(input, |value| {
            conduit_std_catalog::deadband_scalar(value, radius)
        })
    }

    pub fn keymap(
        &mut self,
        input: &[u8],
    ) -> Result<Option<BoundedOutput>, BoundedHostOperationError> {
        let event = KeyEvent::decode(input).map_err(|_| BoundedHostOperationError::InvalidInput)?;
        match self.keymap.apply(event) {
            KeymapDisposition::Text(fragment) => {
                BoundedOutput::from_slice(fragment.as_bytes()).map(Some)
            }
            KeymapDisposition::NoText | KeymapDisposition::Cancelled => Ok(None),
            KeymapDisposition::Refused(
                KeymapRefusal::UnknownComposeSequence
                | KeymapRefusal::EmptyUnicodeEntry
                | KeymapRefusal::UnicodeEntryOverflow
                | KeymapRefusal::InvalidUnicodeScalar,
            ) => Err(BoundedHostOperationError::InvalidInput),
        }
    }

    pub fn chords(&self, input: &[u8]) -> Result<Option<BoundedOutput>, BoundedHostOperationError> {
        let event = KeyEvent::decode(input).map_err(|_| BoundedHostOperationError::InvalidInput)?;
        ChordInfo::from_key_event(event)
            .map(|chord| BoundedOutput::from_slice(&chord.encode()))
            .transpose()
    }

    fn math(
        &self,
        input: &[u8],
        transform: impl FnOnce(Scalar) -> Result<Scalar, conduit_std_catalog::MathScalarError>,
    ) -> Result<BoundedOutput, BoundedHostOperationError> {
        let value = Scalar::decode(input).map_err(|_| BoundedHostOperationError::InvalidInput)?;
        let output = transform(value).map_err(|error| match error {
            conduit_std_catalog::MathScalarError::InvalidConfiguration => {
                BoundedHostOperationError::InvalidConfiguration
            }
            conduit_std_catalog::MathScalarError::Overflow => BoundedHostOperationError::Overflow,
        })?;
        BoundedOutput::from_slice(&output.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{KeyModifiers, KeyTransition};

    #[test]
    fn finite_operations_match_portable_semantics_and_refuse_overflow() {
        let host = BoundedHostOperations::default();
        assert_eq!(
            host.text_join("Hello, ", b"Conduit").unwrap().as_bytes(),
            b"Hello, Conduit"
        );
        assert_eq!(host.decode_bool(&InfoBool::TRUE.encode()), Ok(true));
        assert_eq!(
            Scalar::decode(
                host.math_scale(
                    &Scalar::from_raw_microunits(500_000).encode(),
                    Scalar::from_raw_microunits(2_000_000)
                )
                .unwrap()
                .as_bytes()
            )
            .unwrap(),
            Scalar::from_raw_microunits(1_000_000)
        );
        assert_eq!(
            Scalar::decode(
                host.math_deadband(
                    &Scalar::from_raw_microunits(49_999).encode(),
                    Scalar::from_raw_microunits(50_000)
                )
                .unwrap()
                .as_bytes()
            )
            .unwrap(),
            Scalar::ZERO
        );
        assert_eq!(
            host.text_join(&"x".repeat(TEXT_OUTPUT_BYTES), b"y"),
            Err(BoundedHostOperationError::Overflow)
        );
    }

    #[test]
    fn keymap_and_chords_share_portable_key_meaning_and_reset_state() {
        let event = KeyEvent::new(4, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
        let mut host = BoundedHostOperations::default();
        assert_eq!(
            host.keymap(&event.encode()).unwrap().unwrap().as_bytes(),
            b"a"
        );
        assert!(host.chords(&event.encode()).unwrap().is_none());
        host.reset();
        assert_eq!(
            host.keymap(&event.encode()).unwrap().unwrap().as_bytes(),
            b"a"
        );
        assert_eq!(
            host.keymap(&[0xff]),
            Err(BoundedHostOperationError::InvalidInput)
        );
    }

    #[test]
    fn json_operations_match_the_shared_no_std_semantics() {
        let host = BoundedHostOperations::default();
        let info = host
            .json_decode("{\"z\":1,\"a\":\"世界\"}".as_bytes())
            .unwrap();
        let text = host.json_encode(info.as_bytes()).unwrap();
        assert_eq!(text.as_bytes(), "{\"a\":\"世界\",\"z\":1}".as_bytes());
        assert_eq!(
            host.json_decode(b"{\"a\":1,\"a\":2}"),
            Err(BoundedHostOperationError::Json(
                conduit_core::JsonRefusal::DuplicateKey
            ))
        );
    }
}
