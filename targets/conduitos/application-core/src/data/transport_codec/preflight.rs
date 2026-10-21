//! Count and validate framed JSON without a dynamic parse tree or allocation.
use super::{MAX_BYTES, Refusal};
const MAX_DEPTH: usize = 128;

pub(super) fn validate(bytes: &[u8]) -> Result<(), Refusal> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(Refusal::Bounds);
    }
    core::str::from_utf8(bytes).map_err(|_| Refusal::Malformed)?;
    let mut scan = Scan { bytes, at: 0 };
    scan.value(0)?;
    scan.space();
    if scan.at != bytes.len() {
        return Err(Refusal::Malformed);
    }
    Ok(())
}
struct Scan<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Scan<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.at += 1;
        }
    }
    fn take(&mut self, byte: u8) -> Result<(), Refusal> {
        self.space();
        if self.bytes.get(self.at) != Some(&byte) {
            return Err(Refusal::Malformed);
        }
        self.at += 1;
        Ok(())
    }
    fn value(&mut self, depth: usize) -> Result<(), Refusal> {
        self.space();
        match self.bytes.get(self.at).copied().ok_or(Refusal::Malformed)? {
            b'[' => {
                if depth == MAX_DEPTH {
                    return Err(Refusal::Bounds);
                }
                self.array(depth + 1)
            }
            b'{' => {
                if depth == MAX_DEPTH {
                    return Err(Refusal::Bounds);
                }
                self.object(depth + 1)
            }
            b'"' => self.string(),
            b't' => self.literal(b"true"),
            b'f' => self.literal(b"false"),
            b'n' => self.literal(b"null"),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(Refusal::Malformed),
        }
    }
    fn literal(&mut self, value: &[u8]) -> Result<(), Refusal> {
        if self.bytes.get(self.at..self.at + value.len()) != Some(value) {
            return Err(Refusal::Malformed);
        }
        self.at += value.len();
        Ok(())
    }
    fn array(&mut self, depth: usize) -> Result<(), Refusal> {
        self.take(b'[')?;
        self.space();
        let start = self.at;
        let mut count = 0usize;
        while let Some(b'0'..=b'9') = self.bytes.get(self.at) {
            count = count
                .checked_mul(10)
                .and_then(|n| n.checked_add((self.bytes[self.at] - b'0') as usize))
                .ok_or(Refusal::Bounds)?;
            if count > MAX_BYTES {
                return Err(Refusal::Bounds);
            }
            self.at += 1;
        }
        if self.at == start || (self.at - start > 1 && self.bytes[start] == b'0') {
            return Err(Refusal::Malformed);
        }
        for _ in 0..count {
            self.take(b',')?;
            self.value(depth)?;
        }
        self.take(b']')
    }
    fn object(&mut self, depth: usize) -> Result<(), Refusal> {
        self.take(b'{')?;
        self.space();
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(());
        }
        loop {
            self.string()?;
            self.take(b':')?;
            self.value(depth)?;
            self.space();
            match self.bytes.get(self.at) {
                Some(b'}') => {
                    self.at += 1;
                    return Ok(());
                }
                Some(b',') => {
                    self.at += 1;
                    self.space();
                }
                _ => return Err(Refusal::Malformed),
            }
        }
    }
    fn hex4(&mut self) -> Result<u16, Refusal> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = match self.bytes.get(self.at).copied() {
                Some(b'0'..=b'9') => self.bytes[self.at] - b'0',
                Some(b'a'..=b'f') => self.bytes[self.at] - b'a' + 10,
                Some(b'A'..=b'F') => self.bytes[self.at] - b'A' + 10,
                _ => return Err(Refusal::Malformed),
            };
            value = (value << 4) | u16::from(digit);
            self.at += 1;
        }
        Ok(value)
    }
    fn string(&mut self) -> Result<(), Refusal> {
        self.take(b'"')?;
        loop {
            let byte = *self.bytes.get(self.at).ok_or(Refusal::Malformed)?;
            self.at += 1;
            match byte {
                b'"' => return Ok(()),
                0..=31 => return Err(Refusal::Malformed),
                b'\\' => {
                    let escape = *self.bytes.get(self.at).ok_or(Refusal::Malformed)?;
                    self.at += 1;
                    match escape {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                        b'u' => match self.hex4()? {
                            0xD800..=0xDBFF => {
                                if self.bytes.get(self.at..self.at + 2) != Some(b"\\u") {
                                    return Err(Refusal::Malformed);
                                }
                                self.at += 2;
                                if !(0xDC00..=0xDFFF).contains(&self.hex4()?) {
                                    return Err(Refusal::Malformed);
                                }
                            }
                            0xDC00..=0xDFFF => return Err(Refusal::Malformed),
                            _ => {}
                        },
                        _ => return Err(Refusal::Malformed),
                    }
                }
                _ => {}
            }
        }
    }
    fn number(&mut self) -> Result<(), Refusal> {
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        match self.bytes.get(self.at) {
            Some(b'0') => {
                self.at += 1;
            }
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(Refusal::Malformed),
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            if self.digits() == 0 {
                return Err(Refusal::Malformed);
            }
        }
        if self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b'e' | b'E'))
        {
            self.at += 1;
            if self
                .bytes
                .get(self.at)
                .is_some_and(|b| matches!(b, b'+' | b'-'))
            {
                self.at += 1;
            }
            if self.digits() == 0 {
                return Err(Refusal::Malformed);
            }
        }
        Ok(())
    }
    fn digits(&mut self) -> usize {
        let start = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        self.at - start
    }
}
