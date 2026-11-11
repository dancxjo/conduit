//! Structural ranges over an ALREADY admitted compact evidence document.
//! This is not a JSON validator. PreparedEpisodeRetrievalEvidence must accept
//! the exact original bytes before this helper is called. No facts are decoded.
use super::R;
use crate::FixedEvidenceText;

pub(crate) fn locate_validated_items(input: &[u8]) -> Result<[(usize, usize); 8], R> {
    let mut ranges = [(0, 0); 8];
    let mut pos = space(input, 0);
    expect(input, pos, b'{')?;
    pos = space(input, pos + 1);
    let mut scratch = [0; 256];
    while input.get(pos) != Some(&b'}') {
        let key_start = pos;
        let key_end = string_end(input, pos)?;
        let key = serde_json_core::from_slice_escaped::<FixedEvidenceText<256>>(
            input
                .get(key_start..key_end)
                .ok_or(R::InvalidEvidenceJson)?,
            &mut scratch,
        );
        // The metadata owner already admitted every key. A surrogate-pair key
        // fails serde-json-core's decoder but cannot spell the ASCII 'evidence'.
        let evidence = key.is_ok_and(|(key, _)| key.as_str() == "evidence");
        pos = space(input, key_end);
        expect(input, pos, b':')?;
        pos = space(input, pos + 1);
        if evidence {
            expect(input, pos, b'[')?;
            pos = space(input, pos + 1);
            let mut count = 0;
            while input.get(pos) != Some(&b']') {
                let start = pos;
                let end = value_end(input, start)?;
                let slot = ranges.get_mut(count).ok_or(R::InvalidEvidenceJson)?;
                *slot = (start, end);
                count += 1;
                pos = space(input, end);
                match input.get(pos) {
                    Some(b',') => pos = space(input, pos + 1),
                    Some(b']') => break,
                    _ => return Err(R::InvalidEvidenceJson),
                }
            }
            return Ok(ranges);
        }
        pos = space(input, value_end(input, pos)?);
        match input.get(pos) {
            Some(b',') => pos = space(input, pos + 1),
            Some(b'}') => break,
            _ => return Err(R::InvalidEvidenceJson),
        }
    }
    Err(R::InvalidEvidenceJson)
}
fn expect(input: &[u8], at: usize, byte: u8) -> Result<(), R> {
    if input.get(at) == Some(&byte) {
        Ok(())
    } else {
        Err(R::InvalidEvidenceJson)
    }
}
fn space(input: &[u8], mut at: usize) -> usize {
    while input.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}
fn string_end(input: &[u8], mut at: usize) -> Result<usize, R> {
    expect(input, at, b'"')?;
    at += 1;
    while let Some(byte) = input.get(at) {
        match byte {
            b'"' => return Ok(at + 1),
            b'\\' => {
                input.get(at + 1).ok_or(R::InvalidEvidenceJson)?;
                at += 2;
            }
            _ => at += 1,
        }
    }
    Err(R::InvalidEvidenceJson)
}
fn value_end(input: &[u8], mut at: usize) -> Result<usize, R> {
    match input.get(at).ok_or(R::InvalidEvidenceJson)? {
        b'"' => string_end(input, at),
        b'{' | b'[' => {
            let mut nesting = 0usize;
            loop {
                match input.get(at).ok_or(R::InvalidEvidenceJson)? {
                    b'"' => {
                        at = string_end(input, at)?;
                        continue;
                    }
                    b'{' | b'[' => nesting += 1,
                    b'}' | b']' => {
                        nesting = nesting.checked_sub(1).ok_or(R::InvalidEvidenceJson)?;
                        if nesting == 0 {
                            return Ok(at + 1);
                        }
                    }
                    _ => {}
                }
                at += 1;
            }
        }
        _ => {
            let start = at;
            while input
                .get(at)
                .is_some_and(|b| !b.is_ascii_whitespace() && !matches!(b, b',' | b'}' | b']'))
            {
                at += 1;
            }
            if at == start {
                Err(R::InvalidEvidenceJson)
            } else {
                Ok(at)
            }
        }
    }
}

pub(crate) fn text_field<'a>(input: &'a [u8], name: &str) -> Result<&'a str, R> {
    let mut pos = space(input, 0);
    expect(input, pos, b'{')?;
    pos = space(input, pos + 1);
    let mut scratch = [0; 256];
    while input.get(pos) != Some(&b'}') {
        let end = string_end(input, pos)?;
        let key = serde_json_core::from_slice_escaped::<FixedEvidenceText<256>>(
            &input[pos..end],
            &mut scratch,
        );
        let matches = key.is_ok_and(|(key, _)| key.as_str() == name);
        pos = space(input, end);
        expect(input, pos, b':')?;
        pos = space(input, pos + 1);
        let end = value_end(input, pos)?;
        if matches {
            return core::str::from_utf8(&input[pos..end]).map_err(|_| R::InvalidEvidenceJson);
        }
        pos = space(input, end);
        if input.get(pos) == Some(&b',') {
            pos = space(input, pos + 1);
        } else {
            break;
        }
    }
    Ok("")
}

/// Borrow structural values from a document already validated by its owner.
pub(crate) fn array_values(input: &[u8]) -> Result<Values<'_>, R> {
    let pos = space(input, 0);
    expect(input, pos, b'[')?;
    Ok(Values {
        input,
        pos: space(input, pos + 1),
        done: false,
    })
}
pub(crate) struct Values<'a> {
    input: &'a [u8],
    pos: usize,
    done: bool,
}
impl<'a> Iterator for Values<'a> {
    type Item = Result<&'a [u8], R>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.input.get(self.pos) == Some(&b']') {
            return None;
        }
        let start = self.pos;
        let result = (|| {
            let end = value_end(self.input, start)?;
            self.pos = space(self.input, end);
            match self.input.get(self.pos) {
                Some(b',') => self.pos = space(self.input, self.pos + 1),
                Some(b']') => self.done = true,
                _ => return Err(R::InvalidEvidenceJson),
            }
            Ok(&self.input[start..end])
        })();
        if result.is_err() {
            self.done = true;
        }
        Some(result)
    }
}
pub(crate) fn object_fields(input: &[u8]) -> Result<Fields<'_>, R> {
    let pos = space(input, 0);
    expect(input, pos, b'{')?;
    Ok(Fields {
        input,
        pos: space(input, pos + 1),
        done: false,
    })
}
pub(crate) struct Fields<'a> {
    input: &'a [u8],
    pos: usize,
    done: bool,
}
impl<'a> Iterator for Fields<'a> {
    type Item = Result<(&'a [u8], &'a [u8]), R>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.input.get(self.pos) == Some(&b'}') {
            return None;
        }
        let start = self.pos;
        let result = (|| {
            let key_end = string_end(self.input, start)?;
            self.pos = space(self.input, key_end);
            expect(self.input, self.pos, b':')?;
            let value_start = space(self.input, self.pos + 1);
            let end = value_end(self.input, value_start)?;
            self.pos = space(self.input, end);
            match self.input.get(self.pos) {
                Some(b',') => self.pos = space(self.input, self.pos + 1),
                Some(b'}') => self.done = true,
                _ => return Err(R::InvalidEvidenceJson),
            }
            Ok((&self.input[start..key_end], &self.input[value_start..end]))
        })();
        if result.is_err() {
            self.done = true;
        }
        Some(result)
    }
}
