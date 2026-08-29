//! Canonical bounded International Morse representation and text transforms.

#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal,
};

pub const MORSE_PATTERN_VALUE_KIND: &str = "value/morse-pattern@1";
pub const TEXT_MORSE_KIND: &str = "text/morse";
pub const TEXT_MORSE_CONTRACT_REVISION: &str = "conduit.text/morse@1";
pub const MORSE_TEXT_KIND: &str = "morse/text";
pub const MORSE_TEXT_CONTRACT_REVISION: &str = "conduit.morse/text@1";
pub const MORSE_UNIT_MILLIS_KEY: &str = "unit-ms";
pub const DEFAULT_MORSE_UNIT_MILLIS: u16 = 120;
pub const MINIMUM_MORSE_UNIT_MILLIS: u16 = 40;
pub const MAXIMUM_MORSE_UNIT_MILLIS: u16 = 2_000;
pub const MAXIMUM_MORSE_INPUT_BYTES: usize = 32;
pub const MAXIMUM_MORSE_SEGMENTS: usize = 320;
pub const MAXIMUM_MORSE_PATTERN_BYTES: usize = 5 + MAXIMUM_MORSE_SEGMENTS * 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MorseSegment {
    pub level: bool,
    pub units: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MorsePattern {
    pub unit_millis: u16,
    pub segments: Vec<MorseSegment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MorseError {
    Empty,
    TextTooLong,
    UnsupportedCharacter,
    InvalidWordGap,
    InvalidUnitMillis,
    SegmentCapacity,
    OutputCapacity,
    MalformedEncoding,
    NonCanonicalEncoding,
    InvalidPattern,
}

pub fn text_morse_semantics() -> MorseKindContract {
    MorseKindContract {
        kind_id: kind_id(TEXT_MORSE_KIND),
        kind_contract_revision: KindContractRevision::from(TEXT_MORSE_CONTRACT_REVISION),
        inputs: vec![text_port(PortDirection::Input)],
        outputs: vec![morse_port(PortDirection::Output)],
        configuration: vec![(
            MORSE_UNIT_MILLIS_KEY,
            ConfigurationValue::U64(u64::from(DEFAULT_MORSE_UNIT_MILLIS)),
        )],
        limits: morse_limits(),
    }
}

pub fn morse_text_semantics() -> MorseKindContract {
    MorseKindContract {
        kind_id: kind_id(MORSE_TEXT_KIND),
        kind_contract_revision: KindContractRevision::from(MORSE_TEXT_CONTRACT_REVISION),
        inputs: vec![morse_port(PortDirection::Input)],
        outputs: vec![text_port(PortDirection::Output)],
        configuration: Vec::new(),
        limits: morse_limits(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MorseKindContract {
    pub kind_id: conduit_core::KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<(&'static str, ConfigurationValue)>,
    pub limits: CapabilityLimits,
}

#[cfg(feature = "form-catalog")]
pub fn install_morse_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    startup.insert(KindSignature {
        kind: TEXT_MORSE_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: MORSE_UNIT_MILLIS_KEY.into(),
            value_type: "Count".into(),
            default: Some(DEFAULT_MORSE_UNIT_MILLIS.to_string()),
        }],
    })?;
    startup.insert(KindSignature {
        kind: MORSE_TEXT_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    let encoder = text_morse_semantics();
    profile
        .insert(KindDefinition {
            kind_id: encoder.kind_id,
            kind_contract_revision: encoder.kind_contract_revision,
            inputs: encoder.inputs,
            outputs: encoder.outputs,
            configuration: vec![ConfigurationField {
                key: MORSE_UNIT_MILLIS_KEY.into(),
                default_value: ConfigurationValue::U64(u64::from(DEFAULT_MORSE_UNIT_MILLIS)),
                validation: ConfigurationRule::U64Range {
                    minimum: u64::from(MINIMUM_MORSE_UNIT_MILLIS),
                    maximum: u64::from(MAXIMUM_MORSE_UNIT_MILLIS),
                },
            }],
        })
        .map_err(|error| error.to_string())?;
    let decoder = morse_text_semantics();
    profile
        .insert(KindDefinition {
            kind_id: decoder.kind_id,
            kind_contract_revision: decoder.kind_contract_revision,
            inputs: decoder.inputs,
            outputs: decoder.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    crate::morse_catalog::install_morse_composition_catalogs(startup, profile)
}

impl MorsePattern {
    pub fn from_text(text: &str, unit_millis: u16) -> Result<Self, MorseError> {
        crate::composed_morse_from_text(text, unit_millis)
    }

    pub fn to_text(&self) -> Result<String, MorseError> {
        let symbols = crate::morse_pattern_to_symbols(&self.encode()?)?;
        String::from_utf8(crate::morse_symbols_to_text(&symbols)?)
            .map_err(|_| MorseError::InvalidPattern)
    }

    pub fn encode(&self) -> Result<Vec<u8>, MorseError> {
        self.validate()?;
        let length = 5_usize
            .checked_add(
                self.segments
                    .len()
                    .checked_mul(2)
                    .ok_or(MorseError::OutputCapacity)?,
            )
            .ok_or(MorseError::OutputCapacity)?;
        if length > MAXIMUM_MORSE_PATTERN_BYTES {
            return Err(MorseError::OutputCapacity);
        }
        let count = u16::try_from(self.segments.len()).map_err(|_| MorseError::OutputCapacity)?;
        let mut encoded = Vec::with_capacity(length);
        encoded.push(1);
        encoded.extend_from_slice(&self.unit_millis.to_le_bytes());
        encoded.extend_from_slice(&count.to_le_bytes());
        for segment in &self.segments {
            encoded.push(u8::from(segment.level));
            encoded.push(segment.units);
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, MorseError> {
        if encoded.len() < 7 || encoded[0] != 1 {
            return Err(MorseError::MalformedEncoding);
        }
        let unit_millis = u16::from_le_bytes([encoded[1], encoded[2]]);
        let count = usize::from(u16::from_le_bytes([encoded[3], encoded[4]]));
        let expected = 5_usize
            .checked_add(count.checked_mul(2).ok_or(MorseError::MalformedEncoding)?)
            .ok_or(MorseError::MalformedEncoding)?;
        if expected != encoded.len() || count > MAXIMUM_MORSE_SEGMENTS {
            return Err(MorseError::MalformedEncoding);
        }
        let mut segments = Vec::with_capacity(count);
        for pair in encoded[5..].as_chunks::<2>().0 {
            let level = match pair[0] {
                0 => false,
                1 => true,
                _ => return Err(MorseError::MalformedEncoding),
            };
            segments.push(MorseSegment {
                level,
                units: pair[1],
            });
        }
        let pattern = Self {
            unit_millis,
            segments,
        };
        pattern.validate()?;
        if pattern.encode()?.as_slice() != encoded {
            return Err(MorseError::NonCanonicalEncoding);
        }
        Ok(pattern)
    }

    fn validate(&self) -> Result<(), MorseError> {
        valid_unit_millis(self.unit_millis)?;
        if self.segments.is_empty() || self.segments.len() > MAXIMUM_MORSE_SEGMENTS {
            return Err(MorseError::InvalidPattern);
        }
        if !self.segments[0].level || !self.segments.last().is_some_and(|value| value.level) {
            return Err(MorseError::InvalidPattern);
        }
        for (index, segment) in self.segments.iter().enumerate() {
            if segment.units == 0
                || (segment.level && !matches!(segment.units, 1 | 3))
                || (!segment.level && !matches!(segment.units, 1 | 3 | 7))
                || self
                    .segments
                    .get(index + 1)
                    .is_some_and(|next| next.level == segment.level)
            {
                return Err(MorseError::InvalidPattern);
            }
        }
        Ok(())
    }
}

fn valid_unit_millis(value: u16) -> Result<(), MorseError> {
    (MINIMUM_MORSE_UNIT_MILLIS..=MAXIMUM_MORSE_UNIT_MILLIS)
        .contains(&value)
        .then_some(())
        .ok_or(MorseError::InvalidUnitMillis)
}

fn text_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("text"),
        value_kind: kind_id(super::TEXT_VALUE_KIND),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn morse_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("pattern"),
        value_kind: kind_id(MORSE_PATTERN_VALUE_KIND),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn morse_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 4,
        max_queue_items: 1,
        max_queue_bytes: MAXIMUM_MORSE_PATTERN_BYTES as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sos_has_exact_units_and_round_trips() {
        let pattern = MorsePattern::from_text("SOS", 120).unwrap();
        assert_eq!(
            pattern
                .segments
                .iter()
                .map(|value| (value.level, value.units))
                .collect::<Vec<_>>(),
            vec![
                (true, 1),
                (false, 1),
                (true, 1),
                (false, 1),
                (true, 1),
                (false, 3),
                (true, 3),
                (false, 1),
                (true, 3),
                (false, 1),
                (true, 3),
                (false, 3),
                (true, 1),
                (false, 1),
                (true, 1),
                (false, 1),
                (true, 1),
            ]
        );
        assert_eq!(pattern.to_text().unwrap(), "SOS");
        assert_eq!(
            MorsePattern::decode(&pattern.encode().unwrap()).unwrap(),
            pattern
        );
    }

    #[test]
    fn every_admitted_character_and_words_round_trip() {
        for character in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars() {
            let text = character.to_string();
            let pattern = MorsePattern::from_text(&text, 80).unwrap();
            assert_eq!(pattern.to_text().unwrap(), text);
        }
        let words = MorsePattern::from_text("HELLO 2026", 80).unwrap();
        assert_eq!(words.to_text().unwrap(), "HELLO 2026");
    }

    #[test]
    fn invalid_text_units_and_encodings_fail_closed() {
        assert_eq!(MorsePattern::from_text("", 120), Err(MorseError::Empty));
        assert_eq!(
            MorsePattern::from_text(" SOS", 120),
            Err(MorseError::InvalidWordGap)
        );
        assert_eq!(
            MorsePattern::from_text("SOS ", 120),
            Err(MorseError::InvalidWordGap)
        );
        assert_eq!(
            MorsePattern::from_text("S  O", 120),
            Err(MorseError::InvalidWordGap)
        );
        assert_eq!(
            MorsePattern::from_text("?", 120),
            Err(MorseError::UnsupportedCharacter)
        );
        assert_eq!(
            MorsePattern::from_text("SOS", 39),
            Err(MorseError::InvalidUnitMillis)
        );
        assert_eq!(
            MorsePattern::decode(&[1, 120, 0, 1, 0, 2, 1]),
            Err(MorseError::MalformedEncoding)
        );
    }
}
