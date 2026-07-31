use crate::Utf8State;

/// Normative length prefix width for `conduit.std.data/length-u32be`.
pub const LENGTH_U32BE_PREFIX_BYTES: usize = 4;
/// Maximum frame retained by the portable reference provider.
pub const DATA_MAX_FRAME_BYTES: usize = 4096;
/// Maximum structural fields examined by the reference validator.
pub const DATA_MAX_RECORD_FIELDS: usize = 32;
/// Maximum UTF-8 bytes in one structural field name.
pub const DATA_MAX_FIELD_NAME_BYTES: usize = 64;
/// Maximum bytes examined in one structural field value.
pub const DATA_MAX_FIELD_VALUE_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataBoundaryError {
    FrameTooLarge,
    OutputTooSmall,
    PartialFrameAtTerminal,
    OutputPending,
    InvalidUtf8,
    TooManyFields,
    FieldNameTooLarge,
    FieldValueTooLarge,
    DuplicateField,
}

impl DataBoundaryError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::FrameTooLarge => "CND-DAT-001",
            Self::OutputTooSmall => "CND-DAT-002",
            Self::PartialFrameAtTerminal => "CND-DAT-003",
            Self::OutputPending => "CND-DAT-004",
            Self::InvalidUtf8 => "CND-DAT-005",
            Self::TooManyFields => "CND-DAT-006",
            Self::FieldNameTooLarge => "CND-DAT-007",
            Self::FieldValueTooLarge => "CND-DAT-008",
            Self::DuplicateField => "CND-DAT-009",
        }
    }
}

/// Writes one exact big-endian 32-bit length-delimited frame.
///
/// The output contains the four-byte length followed by the payload. No
/// checksum, codec detection, or host-language representation is implied.
pub fn encode_length_u32be(
    payload: &[u8],
    output: &mut [u8],
    maximum_frame_bytes: usize,
) -> Result<usize, DataBoundaryError> {
    if payload.len() > maximum_frame_bytes || payload.len() > u32::MAX as usize {
        return Err(DataBoundaryError::FrameTooLarge);
    }
    let required = LENGTH_U32BE_PREFIX_BYTES
        .checked_add(payload.len())
        .ok_or(DataBoundaryError::FrameTooLarge)?;
    if output.len() < required {
        return Err(DataBoundaryError::OutputTooSmall);
    }
    output[..LENGTH_U32BE_PREFIX_BYTES].copy_from_slice(&(payload.len() as u32).to_be_bytes());
    output[LENGTH_U32BE_PREFIX_BYTES..required].copy_from_slice(payload);
    Ok(required)
}

/// Allocator-free incremental decoder for `conduit.std.data/length-u32be`.
///
/// Input chunk boundaries are irrelevant because callers supply bytes one at a
/// time. One complete frame must be consumed with [`Self::take_ready`] before
/// another input byte is admitted.
pub struct LengthU32BeDecoder<const N: usize> {
    prefix: [u8; LENGTH_U32BE_PREFIX_BYTES],
    prefix_len: usize,
    payload: [u8; N],
    payload_len: usize,
    expected_len: Option<usize>,
    ready: bool,
    finished: bool,
    maximum_frame_bytes: usize,
}

impl<const N: usize> LengthU32BeDecoder<N> {
    #[must_use]
    pub const fn new(maximum_frame_bytes: usize) -> Self {
        Self {
            prefix: [0; LENGTH_U32BE_PREFIX_BYTES],
            prefix_len: 0,
            payload: [0; N],
            payload_len: 0,
            expected_len: None,
            ready: false,
            finished: false,
            maximum_frame_bytes,
        }
    }

    /// Accepts one byte and reports whether a complete frame is ready.
    pub fn push_byte(&mut self, byte: u8) -> Result<bool, DataBoundaryError> {
        if self.ready || self.finished {
            return Err(DataBoundaryError::OutputPending);
        }
        if self.expected_len.is_none() {
            self.prefix[self.prefix_len] = byte;
            self.prefix_len += 1;
            if self.prefix_len == LENGTH_U32BE_PREFIX_BYTES {
                let expected = u32::from_be_bytes(self.prefix) as usize;
                if expected > self.maximum_frame_bytes || expected > N {
                    return Err(DataBoundaryError::FrameTooLarge);
                }
                self.expected_len = Some(expected);
                self.ready = expected == 0;
            }
            return Ok(self.ready);
        }

        let expected = self
            .expected_len
            .expect("length prefix establishes expected length");
        self.payload[self.payload_len] = byte;
        self.payload_len += 1;
        self.ready = self.payload_len == expected;
        Ok(self.ready)
    }

    #[must_use]
    pub const fn ready_len(&self) -> Option<usize> {
        if self.ready { self.expected_len } else { None }
    }

    /// Copies and clears one complete frame.
    pub fn take_ready(&mut self, output: &mut [u8]) -> Result<Option<usize>, DataBoundaryError> {
        let Some(length) = self.ready_len() else {
            return Ok(None);
        };
        if output.len() < length {
            return Err(DataBoundaryError::OutputTooSmall);
        }
        output[..length].copy_from_slice(&self.payload[..length]);
        self.prefix_len = 0;
        self.payload_len = 0;
        self.expected_len = None;
        self.ready = false;
        Ok(Some(length))
    }

    /// Completes the stream only between frames.
    pub fn finish(&mut self) -> Result<(), DataBoundaryError> {
        if self.ready || self.prefix_len != 0 || self.expected_len.is_some() {
            return Err(DataBoundaryError::PartialFrameAtTerminal);
        }
        self.finished = true;
        Ok(())
    }

    /// Cancellation explicitly discards the bounded partial frame.
    pub fn cancel(&mut self) {
        self.prefix_len = 0;
        self.payload_len = 0;
        self.expected_len = None;
        self.ready = false;
        self.finished = true;
    }
}

/// Copies exact UTF-8 text bytes into caller-owned storage.
pub fn encode_utf8(text: &str, output: &mut [u8]) -> Result<usize, DataBoundaryError> {
    if output.len() < text.len() {
        return Err(DataBoundaryError::OutputTooSmall);
    }
    output[..text.len()].copy_from_slice(text.as_bytes());
    Ok(text.len())
}

/// Validates exact UTF-8 bytes incrementally and copies them when valid.
pub fn decode_utf8(bytes: &[u8], output: &mut [u8]) -> Result<usize, DataBoundaryError> {
    if output.len() < bytes.len() {
        return Err(DataBoundaryError::OutputTooSmall);
    }
    let mut state = Utf8State::new();
    for &byte in bytes {
        state
            .push_byte(byte)
            .map_err(|_| DataBoundaryError::InvalidUtf8)?;
    }
    state.finish().map_err(|_| DataBoundaryError::InvalidUtf8)?;
    output[..bytes.len()].copy_from_slice(bytes);
    Ok(bytes.len())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralField<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequiredField<'a> {
    pub name: &'a str,
    pub maximum_value_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralRejection {
    MissingRequiredField,
    UnknownField,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralDecision {
    Accepted,
    Rejected {
        field_index: Option<usize>,
        reason: StructuralRejection,
    },
}

/// Validates a deliberately small closed-record descriptor.
///
/// Rejection is a typed domain decision. Malformed or out-of-bound input is a
/// provider error and remains distinct from a valid rejected record.
pub fn validate_closed_record(
    fields: &[StructuralField<'_>],
    required: &[RequiredField<'_>],
) -> Result<StructuralDecision, DataBoundaryError> {
    if fields.len() > DATA_MAX_RECORD_FIELDS || required.len() > DATA_MAX_RECORD_FIELDS {
        return Err(DataBoundaryError::TooManyFields);
    }
    for (index, field) in fields.iter().enumerate() {
        if field.name.len() > DATA_MAX_FIELD_NAME_BYTES {
            return Err(DataBoundaryError::FieldNameTooLarge);
        }
        if field.value.len() > DATA_MAX_FIELD_VALUE_BYTES {
            return Err(DataBoundaryError::FieldValueTooLarge);
        }
        if fields[..index]
            .iter()
            .any(|previous| previous.name == field.name)
        {
            return Err(DataBoundaryError::DuplicateField);
        }
        let Some(descriptor) = required
            .iter()
            .find(|descriptor| descriptor.name == field.name)
        else {
            return Ok(StructuralDecision::Rejected {
                field_index: Some(index),
                reason: StructuralRejection::UnknownField,
            });
        };
        if field.value.len() > descriptor.maximum_value_bytes {
            return Err(DataBoundaryError::FieldValueTooLarge);
        }
    }
    for descriptor in required {
        if descriptor.name.len() > DATA_MAX_FIELD_NAME_BYTES
            || descriptor.maximum_value_bytes > DATA_MAX_FIELD_VALUE_BYTES
        {
            return Err(DataBoundaryError::FieldValueTooLarge);
        }
        if !fields.iter().any(|field| field.name == descriptor.name) {
            return Ok(StructuralDecision::Rejected {
                field_index: None,
                reason: StructuralRejection::MissingRequiredField,
            });
        }
    }
    Ok(StructuralDecision::Accepted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_ignores_input_chunk_boundaries() {
        let mut encoded = [0; 32];
        let first = encode_length_u32be(b"one", &mut encoded, 8).unwrap();
        let second = encode_length_u32be(b"two", &mut encoded[first..], 8).unwrap();
        let mut decoder = LengthU32BeDecoder::<8>::new(8);
        let mut outputs = [[0; 8]; 2];
        let mut lengths = [0; 2];
        let mut output_index = 0;
        for chunk in encoded[..first + second].chunks(2) {
            for &byte in chunk {
                if decoder.push_byte(byte).unwrap() {
                    lengths[output_index] = decoder
                        .take_ready(&mut outputs[output_index])
                        .unwrap()
                        .unwrap();
                    output_index += 1;
                }
            }
        }
        decoder.finish().unwrap();
        assert_eq!(&outputs[0][..lengths[0]], b"one");
        assert_eq!(&outputs[1][..lengths[1]], b"two");
    }

    #[test]
    fn framing_rejects_oversize_and_partial_terminal() {
        let mut decoder = LengthU32BeDecoder::<4>::new(4);
        for byte in 5_u32.to_be_bytes() {
            let result = decoder.push_byte(byte);
            if result.is_err() {
                assert_eq!(result, Err(DataBoundaryError::FrameTooLarge));
                break;
            }
        }
        let mut partial = LengthU32BeDecoder::<4>::new(4);
        partial.push_byte(0).unwrap();
        assert_eq!(
            partial.finish(),
            Err(DataBoundaryError::PartialFrameAtTerminal)
        );
    }

    #[test]
    fn utf8_round_trip_and_malformed_rejection_are_exact() {
        let mut encoded = [0; 16];
        let length = encode_utf8("hé", &mut encoded).unwrap();
        let mut decoded = [0; 16];
        assert_eq!(decode_utf8(&encoded[..length], &mut decoded), Ok(length));
        assert_eq!(&decoded[..length], "hé".as_bytes());
        assert_eq!(
            decode_utf8(&[0xf0, 0x28, 0x8c, 0x28], &mut decoded),
            Err(DataBoundaryError::InvalidUtf8)
        );
    }

    #[test]
    fn structural_rejection_is_not_a_provider_error() {
        let descriptor = [
            RequiredField {
                name: "name",
                maximum_value_bytes: 8,
            },
            RequiredField {
                name: "count",
                maximum_value_bytes: 4,
            },
        ];
        let accepted = [
            StructuralField {
                name: "name",
                value: b"sample",
            },
            StructuralField {
                name: "count",
                value: b"12",
            },
        ];
        assert_eq!(
            validate_closed_record(&accepted, &descriptor),
            Ok(StructuralDecision::Accepted)
        );
        assert_eq!(
            validate_closed_record(&accepted[..1], &descriptor),
            Ok(StructuralDecision::Rejected {
                field_index: None,
                reason: StructuralRejection::MissingRequiredField,
            })
        );
    }
}
