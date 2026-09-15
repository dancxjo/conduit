//! Allocation-free encoders for reviewed Morse Back leaf Gears.

use alloc::vec::Vec;

use crate::morse_values::decode_symbols;
use crate::{
    morse_table, MorseError, MAXIMUM_MORSE_CHARACTERS_BYTES, MAXIMUM_MORSE_GAPPED_GROUPS_BYTES,
    MAXIMUM_MORSE_INPUT_BYTES, MAXIMUM_MORSE_PATTERN_BYTES, MAXIMUM_MORSE_SEGMENTS,
    MAXIMUM_MORSE_SYMBOLS_BYTES, MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES,
};

const VERSION: u8 = 1;
const DOT: u8 = 1;
const DASH: u8 = 2;
const INTRA_GAP: u8 = 3;
const LETTER_GAP: u8 = 4;
const WORD_GAP: u8 = 5;
/// Encode one reviewed Morse-composition leaf into caller-admitted storage.
///
/// These entrances perform no allocation when `output` was prepared with the
/// named maximum capacity. Hosted implementations use them during Play.
pub fn morse_characters_from_text_into(text: &str, output: &mut Vec<u8>) -> Result<(), MorseError> {
    require_capacity(output, MAXIMUM_MORSE_CHARACTERS_BYTES)?;
    if text.is_empty() {
        return Err(MorseError::Empty);
    }
    if text.len() > MAXIMUM_MORSE_INPUT_BYTES {
        return Err(MorseError::TextTooLong);
    }
    output.clear();
    output.extend_from_slice(&[VERSION, text.len() as u8]);
    let mut previous_space = true;
    for byte in text.bytes() {
        if byte == b' ' {
            if previous_space {
                return Err(MorseError::InvalidWordGap);
            }
            previous_space = true;
            output.push(byte);
        } else {
            let normalized = byte.to_ascii_uppercase();
            if morse_table::symbols(normalized).is_none() {
                return Err(MorseError::UnsupportedCharacter);
            }
            previous_space = false;
            output.push(normalized);
        }
    }
    if previous_space {
        return Err(MorseError::InvalidWordGap);
    }
    Ok(())
}

pub fn morse_lookup_characters_into(input: &[u8], output: &mut Vec<u8>) -> Result<(), MorseError> {
    require_capacity(output, MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES)?;
    let characters = checked_characters(input)?;
    output.clear();
    output.extend_from_slice(&[VERSION, characters.len() as u8]);
    for character in characters {
        if *character == b' ' {
            output.push(0);
        } else {
            let symbols =
                morse_table::symbols(*character).ok_or(MorseError::UnsupportedCharacter)?;
            output.push(symbols.len() as u8);
            output.extend(
                symbols
                    .iter()
                    .map(|symbol| if *symbol == b'.' { DOT } else { DASH }),
            );
        }
    }
    Ok(())
}

pub fn morse_intersperse_gaps_into(input: &[u8], output: &mut Vec<u8>) -> Result<(), MorseError> {
    require_capacity(output, MAXIMUM_MORSE_GAPPED_GROUPS_BYTES)?;
    if input.len() < 3 || input[0] != VERSION {
        return Err(MorseError::MalformedEncoding);
    }
    output.clear();
    output.extend_from_slice(&[VERSION, 0]);
    let expected = usize::from(input[1]);
    let mut cursor = 2;
    let mut groups = 0;
    let mut letters = 0_u8;
    let mut gap_before = 0_u8;
    while cursor < input.len() {
        let length = usize::from(input[cursor]);
        cursor += 1;
        let end = cursor
            .checked_add(length)
            .filter(|end| *end <= input.len())
            .ok_or(MorseError::MalformedEncoding)?;
        let group = &input[cursor..end];
        if length > 5 || !group.iter().all(|symbol| matches!(*symbol, DOT | DASH)) {
            return Err(MorseError::NonCanonicalEncoding);
        }
        groups += 1;
        if group.is_empty() {
            if letters == 0 || gap_before == 7 {
                return Err(MorseError::InvalidWordGap);
            }
            gap_before = 7;
        } else {
            output.push(if letters == 0 { 0 } else { gap_before.max(3) });
            output.push(length as u8);
            output.extend_from_slice(group);
            letters = letters.saturating_add(1);
            gap_before = 0;
        }
        cursor = end;
    }
    if groups != expected || gap_before == 7 || letters == 0 {
        return Err(MorseError::MalformedEncoding);
    }
    output[1] = letters;
    Ok(())
}

pub fn morse_flatten_groups_into(input: &[u8], output: &mut Vec<u8>) -> Result<(), MorseError> {
    require_capacity(output, MAXIMUM_MORSE_SYMBOLS_BYTES)?;
    if input.len() < 4 || input[0] != VERSION {
        return Err(MorseError::MalformedEncoding);
    }
    output.clear();
    output.extend_from_slice(&[VERSION, 0, 0]);
    let expected = usize::from(input[1]);
    let mut cursor = 2;
    let mut groups = 0;
    while cursor < input.len() {
        let gap = input[cursor];
        let length = usize::from(*input.get(cursor + 1).ok_or(MorseError::MalformedEncoding)?);
        cursor += 2;
        let end = cursor
            .checked_add(length)
            .filter(|end| *end <= input.len())
            .ok_or(MorseError::MalformedEncoding)?;
        let symbols = &input[cursor..end];
        if symbols.is_empty()
            || symbols.len() > 5
            || !symbols.iter().all(|symbol| matches!(*symbol, DOT | DASH))
            || (groups == 0 && gap != 0)
            || (groups > 0 && !matches!(gap, 3 | 7))
        {
            return Err(MorseError::NonCanonicalEncoding);
        }
        if gap != 0 {
            output.push(if gap == 3 { LETTER_GAP } else { WORD_GAP });
        }
        for (index, symbol) in symbols.iter().enumerate() {
            output.push(*symbol);
            if index + 1 < symbols.len() {
                output.push(INTRA_GAP);
            }
        }
        groups += 1;
        cursor = end;
    }
    if groups != expected || output.len() == 3 || output.len() - 3 > MAXIMUM_MORSE_SEGMENTS {
        return Err(MorseError::MalformedEncoding);
    }
    let count = u16::try_from(output.len() - 3).map_err(|_| MorseError::SegmentCapacity)?;
    output[1..3].copy_from_slice(&count.to_le_bytes());
    decode_symbols(output)?;
    Ok(())
}

pub fn morse_symbols_to_pattern_into(
    input: &[u8],
    unit_millis: u16,
    output: &mut Vec<u8>,
) -> Result<(), MorseError> {
    require_capacity(output, MAXIMUM_MORSE_PATTERN_BYTES)?;
    if !(crate::MINIMUM_MORSE_UNIT_MILLIS..=crate::MAXIMUM_MORSE_UNIT_MILLIS).contains(&unit_millis)
    {
        return Err(MorseError::InvalidUnitMillis);
    }
    let tokens = decode_symbols(input)?;
    output.clear();
    output.push(VERSION);
    output.extend_from_slice(&unit_millis.to_le_bytes());
    output.extend_from_slice(&(tokens.len() as u16).to_le_bytes());
    for token in tokens {
        let (level, units) = match *token {
            DOT => (1, 1),
            DASH => (1, 3),
            INTRA_GAP => (0, 1),
            LETTER_GAP => (0, 3),
            WORD_GAP => (0, 7),
            _ => return Err(MorseError::NonCanonicalEncoding),
        };
        output.extend_from_slice(&[level, units]);
    }
    Ok(())
}

fn require_capacity(output: &Vec<u8>, required: usize) -> Result<(), MorseError> {
    if output.capacity() < required {
        Err(MorseError::OutputCapacity)
    } else {
        Ok(())
    }
}

fn checked_characters(input: &[u8]) -> Result<&[u8], MorseError> {
    if input.len() < 3 || input[0] != VERSION || usize::from(input[1]) + 2 != input.len() {
        return Err(MorseError::MalformedEncoding);
    }
    let text = core::str::from_utf8(&input[2..]).map_err(|_| MorseError::MalformedEncoding)?;
    let mut previous_space = true;
    for byte in text.bytes() {
        if byte == b' ' {
            if previous_space {
                return Err(MorseError::InvalidWordGap);
            }
            previous_space = true;
        } else if morse_table::symbols(byte).is_none() || byte != byte.to_ascii_uppercase() {
            return Err(MorseError::NonCanonicalEncoding);
        } else {
            previous_space = false;
        }
    }
    if previous_space {
        return Err(MorseError::InvalidWordGap);
    }
    Ok(&input[2..])
}
