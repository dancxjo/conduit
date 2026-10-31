//! Bounded projection of an already typed/authenticated private envelope.
//! This is not a general JSON parser or an authority constructor. The source
//! owner must authenticate all bytes; only its sealed archive span is removed.
use super::*;
struct Input<R> {
    read: R,
    bytes: [u8; 128],
    at: usize,
    len: usize,
    offset: usize,
}
impl<R> Drop for Input<R> {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}
impl<R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> Input<R> {
    fn peek(&mut self) -> Result<Option<u8>, ()> {
        if self.at == self.len {
            self.bytes.fill(0);
            self.len = (self.read)(&mut self.bytes)?;
            self.at = 0;
            if self.len > 128 {
                return Err(());
            }
        }
        Ok(self
            .bytes
            .get(self.at)
            .copied()
            .filter(|_| self.at < self.len))
    }
    fn next(&mut self) -> Result<u8, ()> {
        let byte = self.peek()?.ok_or(())?;
        self.at += 1;
        self.offset = self
            .offset
            .checked_add(1)
            .filter(|n| *n <= MAX_PLAIN_BYTES)
            .ok_or(())?;
        Ok(byte)
    }
    fn whitespace(&mut self) -> Result<(), ()> {
        while self.peek()?.is_some_and(|b| b.is_ascii_whitespace()) {
            self.next()?;
        }
        Ok(())
    }
    fn expect(&mut self, byte: u8) -> Result<(), ()> {
        if self.next()? == byte {
            Ok(())
        } else {
            Err(())
        }
    }
    fn key(&mut self) -> Result<usize, ()> {
        self.expect(b'"')?;
        let mut key = [0; 32];
        let mut len = 0;
        loop {
            let byte = self.next()?;
            if byte == b'"' {
                break;
            }
            if !byte.is_ascii_lowercase() && byte != b'_' {
                return Err(());
            }
            *key.get_mut(len).ok_or(())? = byte;
            len += 1;
        }
        [
            b"schema".as_slice(),
            b"evidence",
            b"biography_prefix",
            b"observation_layouts",
            b"network_cursor",
            b"archives",
            b"admissions",
        ]
        .iter()
        .position(|expected| *expected == &key[..len])
        .ok_or(())
    }
    /// Copies a complete value without retaining tokens, strings or archives.
    fn value(
        &mut self,
        output: &mut dyn FnMut(&[u8]) -> Result<(), ()>,
    ) -> Result<archive::Span, ()> {
        let start = self.offset;
        let first = self.peek()?.ok_or(())?;
        let mut stack = [0; 128];
        let mut depth = 0;
        let mut string = false;
        let mut escape = false;
        loop {
            let byte = self.next()?;
            output(&[byte])?;
            if string {
                if escape {
                    escape = false;
                } else if byte == b'\\' {
                    escape = true;
                } else if byte == b'"' {
                    string = false;
                    if depth == 0 {
                        break;
                    }
                }
            } else {
                match byte {
                    b'"' => string = true,
                    b'{' | b'[' => {
                        *stack.get_mut(depth).ok_or(())? = if byte == b'{' { b'}' } else { b']' };
                        depth += 1;
                    }
                    b'}' | b']' => {
                        depth = depth.checked_sub(1).ok_or(())?;
                        if stack[depth] != byte {
                            return Err(());
                        }
                        if depth == 0 {
                            break;
                        }
                    }
                    _ if depth == 0
                        && first != b'"'
                        && self
                            .peek()?
                            .is_none_or(|b| b == b',' || b == b'}' || b.is_ascii_whitespace()) =>
                    {
                        break;
                    }
                    _ => {}
                }
            }
        }
        Ok(archive::Span {
            start,
            end: self.offset,
        })
    }
}
pub(super) fn rewrite(
    read: &mut dyn FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    receipt: &wire::Receipt,
    old_span: archive::Span,
    old_prefix: bool,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), ()>,
) -> Result<archive::Span, ()> {
    let mut input = Input {
        read,
        bytes: [0; 128],
        at: 0,
        len: 0,
        offset: 0,
    };
    let count = core::cell::Cell::new(0usize);
    let mut output = |bytes: &[u8]| {
        count.set(
            count
                .get()
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_PLAIN_BYTES)
                .ok_or(())?,
        );
        sink(bytes)
    };
    let mut seen = 0u8;
    let mut first = true;
    let mut span = None;
    input.whitespace()?;
    input.expect(b'{')?;
    output(b"{")?;
    loop {
        input.whitespace()?;
        if input.peek()? == Some(b'}') {
            input.next()?;
            break;
        }
        if !first {
            input.expect(b',')?;
            input.whitespace()?;
            output(b",")?;
        }
        first = false;
        let field = input.key()?;
        let bit = 1u8 << field;
        if seen & bit != 0 {
            return Err(());
        }
        seen |= bit;
        input.whitespace()?;
        input.expect(b':')?;
        input.whitespace()?;
        if field == 5 && !old_prefix {
            output(b"\"biography_prefix\":")?;
            json::stream(receipt, &mut |b: &[u8]| {
                output(b).map_err(|_| json::CodecError::Sink)
            })
            .map_err(|_| ())?;
            output(b",")?;
        }
        let key = [
            b"schema".as_slice(),
            b"evidence",
            b"biography_prefix",
            b"observation_layouts",
            b"network_cursor",
            b"archives",
            b"admissions",
        ][field];
        output(b"\"")?;
        output(key)?;
        output(b"\":")?;
        match field {
            0 => {
                let mut schema = [0; 64];
                let mut n = 0usize;
                input.value(&mut |b| {
                    let end = n.checked_add(b.len()).ok_or(())?;
                    schema.get_mut(n..end).ok_or(())?.copy_from_slice(b);
                    n = end;
                    Ok(())
                })?;
                let schema: &str = serde_json_core::from_slice(&schema[..n]).map_err(|_| ())?.0;
                if ![SCHEMA, SCHEMA_V2, SCHEMA_V3, SCHEMA_V4].contains(&schema) {
                    return Err(());
                }
                json::stream(
                    &if schema == SCHEMA { SCHEMA_V2 } else { schema },
                    &mut |b: &[u8]| output(b).map_err(|_| json::CodecError::Sink),
                )
                .map_err(|_| ())?;
            }
            2 => {
                input.value(&mut |_| Ok(()))?;
                json::stream(receipt, &mut |b: &[u8]| {
                    output(b).map_err(|_| json::CodecError::Sink)
                })
                .map_err(|_| ())?;
            }
            5 => {
                if input.peek()? != Some(b'[') {
                    return Err(());
                }
                let removed = input.value(&mut |_| Ok(()))?;
                if removed.start.checked_add(1) != Some(old_span.start)
                    || removed.end.checked_sub(1) != Some(old_span.end)
                {
                    return Err(());
                }
                let start = count.get().checked_add(1).ok_or(())?;
                output(b"[]")?;
                span = Some(archive::Span { start, end: start });
            }
            _ => {
                input.value(&mut output)?;
            }
        }
    }
    input.whitespace()?;
    if input.peek()?.is_some() || seen & 0b1100011 != 0b1100011 || (seen & 4 != 0) != old_prefix {
        return Err(());
    }
    output(b"}")?;
    span.ok_or(())
}
