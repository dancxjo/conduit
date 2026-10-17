//! Canonical uppercase and Morse consume the same Source input in one user entry.
use crate::{
    frame::{MORSE_CAPACITY, TEXT_CAPACITY, TextFrame},
    gate, text_transform,
};
use conduit_text::{
    MorseError, morse_characters_from_text_into_slice, morse_flatten_groups_into_slice,
    morse_intersperse_gaps_into_slice, morse_lookup_characters_into_slice,
    morse_symbols_to_pattern_into_slice,
};

pub fn chain(frame: &mut TextFrame) -> ! {
    frame.output_length = 0;
    frame.morse_length = 0;
    frame.morse_status = 0;
    let length = frame.input_length as usize;
    if length > TEXT_CAPACITY || frame.capacity != TEXT_CAPACITY as u32 {
        gate::finish(3);
    }
    frame.status = match text_transform::uppercase_into(&frame.input[..length], &mut frame.output) {
        Ok(length) => {
            frame.output_length = length as u32;
            0
        }
        Err(text_transform::UppercaseError::MalformedUtf8) => 1,
        Err(text_transform::UppercaseError::OutputOverflow) => 2,
    };
    let output = encode(&frame.input[..length], frame.unit_millis, &mut frame.morse);
    match output {
        Ok(length) => frame.morse_length = length as u32,
        Err(error) => frame.morse_status = refusal_code(error),
    }
    gate::finish(frame.status)
}

fn encode(input: &[u8], unit: u32, output: &mut [u8; MORSE_CAPACITY]) -> Result<usize, MorseError> {
    let text = core::str::from_utf8(input).map_err(|_| MorseError::UnsupportedCharacter)?;
    let unit = u16::try_from(unit).map_err(|_| MorseError::InvalidUnitMillis)?;
    let mut first = [0; MORSE_CAPACITY];
    let mut second = [0; MORSE_CAPACITY];
    let length = morse_characters_from_text_into_slice(text, &mut first)?;
    let length = morse_lookup_characters_into_slice(&first[..length], &mut second)?;
    let length = morse_intersperse_gaps_into_slice(&second[..length], &mut first)?;
    let length = morse_flatten_groups_into_slice(&first[..length], &mut second)?;
    morse_symbols_to_pattern_into_slice(&second[..length], unit, output)
}

fn refusal_code(error: MorseError) -> u32 {
    match error {
        MorseError::Empty => 1,
        MorseError::TextTooLong => 2,
        MorseError::UnsupportedCharacter => 3,
        MorseError::InvalidWordGap => 4,
        MorseError::InvalidUnitMillis => 5,
        MorseError::SegmentCapacity => 6,
        MorseError::OutputCapacity => 7,
        MorseError::MalformedEncoding => 8,
        MorseError::NonCanonicalEncoding => 9,
        MorseError::InvalidPattern => 10,
    }
}
