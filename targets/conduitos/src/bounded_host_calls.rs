//! Finite semantic Host Calls shared by ordinary ConduitOS planned plays.

use conduit_core::{InfoBool, Scalar};
use conduit_human::{ChordInfo, ConduitIntlKeymap, KeyEvent, KeymapDisposition, KeymapRefusal};

const OUTPUT_BYTES: usize = conduit_web::JSON_MAXIMUM_ENCODED_BYTES;
const TEXT_OUTPUT_BYTES: usize = conduit_text::MAX_TEXT_BYTES as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundedHostCallError {
    InvalidInput,
    InvalidConfiguration,
    Overflow,
    Unsupported,
    Json(conduit_web::JsonRefusal),
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

    fn from_slice(bytes: &[u8]) -> Result<Self, BoundedHostCallError> {
        if bytes.len() > OUTPUT_BYTES {
            return Err(BoundedHostCallError::Overflow);
        }
        let mut output = Self {
            bytes: [0; OUTPUT_BYTES],
            len: bytes.len(),
        };
        output.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(output)
    }
}

pub struct BoundedHostCalls {
    keymap: ConduitIntlKeymap,
}

impl Default for BoundedHostCalls {
    fn default() -> Self {
        Self {
            keymap: ConduitIntlKeymap::new(),
        }
    }
}

impl BoundedHostCalls {
    pub fn reset(&mut self) {
        self.keymap.reset();
    }

    pub fn text_join(
        &self,
        prefix: &str,
        input: &[u8],
    ) -> Result<BoundedOutput, BoundedHostCallError> {
        let text = core::str::from_utf8(input).map_err(|_| BoundedHostCallError::InvalidInput)?;
        let total = prefix
            .len()
            .checked_add(text.len())
            .ok_or(BoundedHostCallError::Overflow)?;
        if total > TEXT_OUTPUT_BYTES {
            return Err(BoundedHostCallError::Overflow);
        }
        let mut output = BoundedOutput {
            bytes: [0; OUTPUT_BYTES],
            len: total,
        };
        output.bytes[..prefix.len()].copy_from_slice(prefix.as_bytes());
        output.bytes[prefix.len()..total].copy_from_slice(text.as_bytes());
        Ok(output)
    }

    pub fn decode_bool(&self, input: &[u8]) -> Result<bool, BoundedHostCallError> {
        InfoBool::decode(input)
            .map(InfoBool::get)
            .map_err(|_| BoundedHostCallError::InvalidInput)
    }

    pub fn json_encode(&self, input: &[u8]) -> Result<BoundedOutput, BoundedHostCallError> {
        let output = conduit_web::JsonValue::decode_info(input)
            .and_then(|value| value.encode_text())
            .map_err(BoundedHostCallError::Json)?;
        BoundedOutput::from_slice(&output)
    }

    pub fn json_decode(&self, input: &[u8]) -> Result<BoundedOutput, BoundedHostCallError> {
        let output = conduit_web::JsonValue::decode_text(input)
            .and_then(|value| value.encode_info())
            .map_err(BoundedHostCallError::Json)?;
        BoundedOutput::from_slice(&output)
    }

    pub fn math_scale(
        &self,
        input: &[u8],
        gain: Scalar,
    ) -> Result<BoundedOutput, BoundedHostCallError> {
        self.math(input, |value| {
            conduit_semantic_catalog::scale_scalar(value, gain)
        })
    }

    pub fn math_deadband(
        &self,
        input: &[u8],
        radius: Scalar,
    ) -> Result<BoundedOutput, BoundedHostCallError> {
        self.math(input, |value| {
            conduit_semantic_catalog::deadband_scalar(value, radius)
        })
    }

    pub fn keymap(&mut self, input: &[u8]) -> Result<Option<BoundedOutput>, BoundedHostCallError> {
        let event = KeyEvent::decode(input).map_err(|_| BoundedHostCallError::InvalidInput)?;
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
            ) => Err(BoundedHostCallError::InvalidInput),
        }
    }

    pub fn chords(&self, input: &[u8]) -> Result<Option<BoundedOutput>, BoundedHostCallError> {
        let event = KeyEvent::decode(input).map_err(|_| BoundedHostCallError::InvalidInput)?;
        ChordInfo::from_key_event(event)
            .map(|chord| BoundedOutput::from_slice(&chord.encode()))
            .transpose()
    }

    fn math(
        &self,
        input: &[u8],
        transform: impl FnOnce(Scalar) -> Result<Scalar, conduit_semantic_catalog::MathScalarError>,
    ) -> Result<BoundedOutput, BoundedHostCallError> {
        let value = Scalar::decode(input).map_err(|_| BoundedHostCallError::InvalidInput)?;
        let output = transform(value).map_err(|error| match error {
            conduit_semantic_catalog::MathScalarError::InvalidConfiguration => {
                BoundedHostCallError::InvalidConfiguration
            }
            conduit_semantic_catalog::MathScalarError::Overflow => BoundedHostCallError::Overflow,
        })?;
        BoundedOutput::from_slice(&output.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_human::{KeyModifiers, KeyTransition};

    #[test]
    fn finite_operations_match_portable_semantics_and_refuse_overflow() {
        let host = BoundedHostCalls::default();
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
            Err(BoundedHostCallError::Overflow)
        );
    }

    #[test]
    fn keymap_and_chords_share_portable_key_meaning_and_reset_state() {
        let event = KeyEvent::new(4, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
        let mut host = BoundedHostCalls::default();
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
            Err(BoundedHostCallError::InvalidInput)
        );
    }

    #[test]
    fn json_operations_match_the_shared_no_std_semantics() {
        let host = BoundedHostCalls::default();
        let info = host
            .json_decode("{\"z\":1,\"a\":\"世界\"}".as_bytes())
            .unwrap();
        let text = host.json_encode(info.as_bytes()).unwrap();
        assert_eq!(text.as_bytes(), "{\"a\":\"世界\",\"z\":1}".as_bytes());
        assert_eq!(
            host.json_decode(b"{\"a\":1,\"a\":2}"),
            Err(BoundedHostCallError::Json(
                conduit_web::JsonRefusal::DuplicateKey
            ))
        );
    }
}
