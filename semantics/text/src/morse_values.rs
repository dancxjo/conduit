//! Canonical bounded intermediate values for compositional Morse realization.
//!
//! Each public transform is one statically typed finite verb. None may grow the
//! graph or iterate beyond the semantic 32-byte input bound during Play.

use alloc::{vec, vec::Vec};

use crate::{
    morse_table, MorseError, MorsePattern, MorseSegment, MAXIMUM_MORSE_INPUT_BYTES,
    MAXIMUM_MORSE_SEGMENTS,
};

pub const MORSE_CHARACTERS_VALUE_KIND: &str = "value/morse-characters@1";
pub const MORSE_SYMBOL_GROUPS_VALUE_KIND: &str = "value/morse-symbol-groups@1";
pub const MORSE_GAPPED_GROUPS_VALUE_KIND: &str = "value/morse-gapped-groups@1";
pub const MORSE_SYMBOLS_VALUE_KIND: &str = "value/morse-symbols@1";

pub const MAXIMUM_MORSE_CHARACTERS_BYTES: usize = 2 + MAXIMUM_MORSE_INPUT_BYTES;
pub const MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES: usize = 2 + MAXIMUM_MORSE_INPUT_BYTES * 6;
pub const MAXIMUM_MORSE_GAPPED_GROUPS_BYTES: usize = 2 + MAXIMUM_MORSE_INPUT_BYTES * 7;
pub const MAXIMUM_MORSE_SYMBOLS_BYTES: usize = 3 + MAXIMUM_MORSE_SEGMENTS;

const VERSION: u8 = 1;
const DOT: u8 = 1;
const DASH: u8 = 2;
const INTRA_GAP: u8 = 3;
const LETTER_GAP: u8 = 4;
const WORD_GAP: u8 = 5;

pub fn morse_characters_from_text(text: &str) -> Result<Vec<u8>, MorseError> {
    if text.is_empty() {
        return Err(MorseError::Empty);
    }
    if text.len() > MAXIMUM_MORSE_INPUT_BYTES {
        return Err(MorseError::TextTooLong);
    }
    let mut encoded = Vec::with_capacity(MAXIMUM_MORSE_CHARACTERS_BYTES);
    encoded.extend_from_slice(&[VERSION, text.len() as u8]);
    let mut previous_space = true;
    for byte in text.bytes() {
        if byte == b' ' {
            if previous_space {
                return Err(MorseError::InvalidWordGap);
            }
            previous_space = true;
            encoded.push(byte);
            continue;
        }
        let normalized = byte.to_ascii_uppercase();
        if morse_table::symbols(normalized).is_none() {
            return Err(MorseError::UnsupportedCharacter);
        }
        previous_space = false;
        encoded.push(normalized);
    }
    if previous_space {
        return Err(MorseError::InvalidWordGap);
    }
    Ok(encoded)
}

pub fn morse_lookup_characters(input: &[u8]) -> Result<Vec<u8>, MorseError> {
    let characters = decode_characters(input)?;
    let mut encoded = Vec::with_capacity(MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES);
    encoded.extend_from_slice(&[VERSION, characters.len() as u8]);
    for character in characters {
        if *character == b' ' {
            encoded.push(0);
            continue;
        }
        let symbols = morse_table::symbols(*character).ok_or(MorseError::UnsupportedCharacter)?;
        encoded.push(symbols.len() as u8);
        encoded.extend(
            symbols
                .iter()
                .map(|symbol| if *symbol == b'.' { DOT } else { DASH }),
        );
    }
    Ok(encoded)
}

pub fn morse_intersperse_gaps(input: &[u8]) -> Result<Vec<u8>, MorseError> {
    let groups = decode_symbol_groups(input)?;
    let letters = groups.iter().filter(|group| !group.is_empty()).count();
    let mut encoded = Vec::with_capacity(MAXIMUM_MORSE_GAPPED_GROUPS_BYTES);
    encoded.extend_from_slice(&[VERSION, letters as u8]);
    let mut gap_before = 0_u8;
    let mut saw_letter = false;
    for group in groups {
        if group.is_empty() {
            if !saw_letter || gap_before == 7 {
                return Err(MorseError::InvalidWordGap);
            }
            gap_before = 7;
            continue;
        }
        encoded.push(if saw_letter { gap_before.max(3) } else { 0 });
        encoded.push(group.len() as u8);
        encoded.extend_from_slice(group);
        saw_letter = true;
        gap_before = 0;
    }
    if gap_before == 7 || !saw_letter {
        return Err(MorseError::InvalidWordGap);
    }
    Ok(encoded)
}

pub fn morse_flatten_groups(input: &[u8]) -> Result<Vec<u8>, MorseError> {
    let groups = decode_gapped_groups(input)?;
    let mut tokens = Vec::with_capacity(MAXIMUM_MORSE_SEGMENTS);
    for (gap, symbols) in groups {
        if gap != 0 {
            tokens.push(if gap == 3 { LETTER_GAP } else { WORD_GAP });
        }
        for (index, symbol) in symbols.iter().enumerate() {
            tokens.push(*symbol);
            if index + 1 < symbols.len() {
                tokens.push(INTRA_GAP);
            }
        }
    }
    if tokens.is_empty() || tokens.len() > MAXIMUM_MORSE_SEGMENTS {
        return Err(MorseError::SegmentCapacity);
    }
    let count = u16::try_from(tokens.len()).map_err(|_| MorseError::SegmentCapacity)?;
    let mut encoded = Vec::with_capacity(3 + tokens.len());
    encoded.push(VERSION);
    encoded.extend_from_slice(&count.to_le_bytes());
    encoded.extend_from_slice(&tokens);
    decode_symbols(&encoded)?;
    Ok(encoded)
}

pub fn morse_symbols_to_pattern(input: &[u8], unit_millis: u16) -> Result<Vec<u8>, MorseError> {
    let tokens = decode_symbols(input)?;
    let mut segments = Vec::with_capacity(tokens.len());
    for token in tokens {
        let segment = match *token {
            DOT => MorseSegment {
                level: true,
                units: 1,
            },
            DASH => MorseSegment {
                level: true,
                units: 3,
            },
            INTRA_GAP => MorseSegment {
                level: false,
                units: 1,
            },
            LETTER_GAP => MorseSegment {
                level: false,
                units: 3,
            },
            WORD_GAP => MorseSegment {
                level: false,
                units: 7,
            },
            _ => return Err(MorseError::NonCanonicalEncoding),
        };
        segments.push(segment);
    }
    MorsePattern {
        unit_millis,
        segments,
    }
    .encode()
}

pub fn morse_pattern_to_symbols(input: &[u8]) -> Result<Vec<u8>, MorseError> {
    let pattern = MorsePattern::decode(input)?;
    let mut tokens = Vec::with_capacity(pattern.segments.len());
    for segment in pattern.segments {
        tokens.push(match (segment.level, segment.units) {
            (true, 1) => DOT,
            (true, 3) => DASH,
            (false, 1) => INTRA_GAP,
            (false, 3) => LETTER_GAP,
            (false, 7) => WORD_GAP,
            _ => return Err(MorseError::InvalidPattern),
        });
    }
    let count = u16::try_from(tokens.len()).map_err(|_| MorseError::SegmentCapacity)?;
    let mut encoded = vec![VERSION];
    encoded.extend_from_slice(&count.to_le_bytes());
    encoded.extend_from_slice(&tokens);
    Ok(encoded)
}

pub fn morse_symbols_to_text(input: &[u8]) -> Result<Vec<u8>, MorseError> {
    let tokens = decode_symbols(input)?;
    let mut output = Vec::with_capacity(MAXIMUM_MORSE_INPUT_BYTES);
    let mut symbol_buffer = [0_u8; 5];
    let mut symbol_count = 0;
    for token in tokens {
        match *token {
            DOT | DASH if symbol_count < symbol_buffer.len() => {
                symbol_buffer[symbol_count] = if *token == DOT { b'.' } else { b'-' };
                symbol_count += 1;
            }
            INTRA_GAP if symbol_count > 0 => {}
            LETTER_GAP | WORD_GAP if symbol_count > 0 => {
                output.push(
                    morse_table::character(&symbol_buffer[..symbol_count])
                        .ok_or(MorseError::InvalidPattern)?,
                );
                symbol_count = 0;
                if *token == WORD_GAP {
                    output.push(b' ');
                }
            }
            _ => return Err(MorseError::InvalidPattern),
        }
    }
    if symbol_count == 0 {
        return Err(MorseError::InvalidPattern);
    }
    output.push(
        morse_table::character(&symbol_buffer[..symbol_count]).ok_or(MorseError::InvalidPattern)?,
    );
    if output.len() > MAXIMUM_MORSE_INPUT_BYTES {
        return Err(MorseError::OutputCapacity);
    }
    Ok(output)
}

pub fn composed_morse_from_text(text: &str, unit_millis: u16) -> Result<MorsePattern, MorseError> {
    let characters = morse_characters_from_text(text)?;
    let groups = morse_lookup_characters(&characters)?;
    let gapped = morse_intersperse_gaps(&groups)?;
    let symbols = morse_flatten_groups(&gapped)?;
    MorsePattern::decode(&morse_symbols_to_pattern(&symbols, unit_millis)?)
}

fn decode_characters(input: &[u8]) -> Result<&[u8], MorseError> {
    if input.len() < 3 || input[0] != VERSION || usize::from(input[1]) + 2 != input.len() {
        return Err(MorseError::MalformedEncoding);
    }
    let text = core::str::from_utf8(&input[2..]).map_err(|_| MorseError::MalformedEncoding)?;
    if morse_characters_from_text(text)?.as_slice() != input {
        return Err(MorseError::NonCanonicalEncoding);
    }
    Ok(&input[2..])
}

fn decode_symbol_groups(input: &[u8]) -> Result<Vec<&[u8]>, MorseError> {
    if input.len() < 3 || input[0] != VERSION {
        return Err(MorseError::MalformedEncoding);
    }
    let expected = usize::from(input[1]);
    let mut groups = Vec::with_capacity(expected);
    let mut cursor = 2;
    while cursor < input.len() {
        let length = usize::from(input[cursor]);
        cursor += 1;
        let end = cursor
            .checked_add(length)
            .filter(|end| *end <= input.len())
            .ok_or(MorseError::MalformedEncoding)?;
        let group = &input[cursor..end];
        if !group.iter().all(|symbol| matches!(*symbol, DOT | DASH)) || length > 5 {
            return Err(MorseError::NonCanonicalEncoding);
        }
        groups.push(group);
        cursor = end;
    }
    if groups.len() != expected {
        return Err(MorseError::MalformedEncoding);
    }
    Ok(groups)
}

fn decode_gapped_groups(input: &[u8]) -> Result<Vec<(u8, &[u8])>, MorseError> {
    if input.len() < 4 || input[0] != VERSION {
        return Err(MorseError::MalformedEncoding);
    }
    let expected = usize::from(input[1]);
    let mut groups = Vec::with_capacity(expected);
    let mut cursor = 2;
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
            || (groups.is_empty() && gap != 0)
            || (!groups.is_empty() && !matches!(gap, 3 | 7))
        {
            return Err(MorseError::NonCanonicalEncoding);
        }
        groups.push((gap, symbols));
        cursor = end;
    }
    if groups.len() != expected {
        return Err(MorseError::MalformedEncoding);
    }
    Ok(groups)
}

fn decode_symbols(input: &[u8]) -> Result<&[u8], MorseError> {
    if input.len() < 4 || input[0] != VERSION {
        return Err(MorseError::MalformedEncoding);
    }
    let count = usize::from(u16::from_le_bytes([input[1], input[2]]));
    let tokens = &input[3..];
    if count != tokens.len()
        || tokens.is_empty()
        || count > MAXIMUM_MORSE_SEGMENTS
        || !tokens
            .first()
            .is_some_and(|token| matches!(*token, DOT | DASH))
        || !tokens
            .last()
            .is_some_and(|token| matches!(*token, DOT | DASH))
    {
        return Err(MorseError::MalformedEncoding);
    }
    for pair in tokens.windows(2) {
        let valid = matches!(pair, [DOT | DASH, INTRA_GAP | LETTER_GAP | WORD_GAP])
            || matches!(pair, [INTRA_GAP | LETTER_GAP | WORD_GAP, DOT | DASH]);
        if !valid {
            return Err(MorseError::NonCanonicalEncoding);
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stage_has_a_distinct_canonical_value_and_matches_direct_morse() {
        let characters = morse_characters_from_text("hello 2026").unwrap();
        let groups = morse_lookup_characters(&characters).unwrap();
        let gapped = morse_intersperse_gaps(&groups).unwrap();
        let symbols = morse_flatten_groups(&gapped).unwrap();
        let encoded = morse_symbols_to_pattern(&symbols, 80).unwrap();
        let pattern = MorsePattern::decode(&encoded).unwrap();
        assert_eq!(pattern.to_text().unwrap(), "HELLO 2026");
        assert_eq!(morse_pattern_to_symbols(&encoded).unwrap(), symbols);
        assert_eq!(morse_symbols_to_text(&symbols).unwrap(), b"HELLO 2026");
    }

    #[test]
    fn intermediate_decoders_refuse_noncanonical_or_unbounded_values() {
        assert_eq!(
            morse_characters_from_text(" A"),
            Err(MorseError::InvalidWordGap)
        );
        assert_eq!(
            morse_characters_from_text("A  B"),
            Err(MorseError::InvalidWordGap)
        );
        assert_eq!(
            morse_lookup_characters(&[1, 1, b'?']),
            Err(MorseError::UnsupportedCharacter)
        );
        assert!(morse_flatten_groups(&[1, 1, 3, 1, DOT]).is_err());
        assert!(morse_symbols_to_pattern(&[1, 1, 0, 9], 120).is_err());
    }
}
