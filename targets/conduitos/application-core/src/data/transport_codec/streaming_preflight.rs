//! Incremental validation of CAPSEQ1's count-prefixed JSON.
//! Each step inspects at most `budget` input bytes. Stack storage is fixed.
use super::Refusal;
const DEPTH: usize = 128;
#[derive(Clone, Copy)]
enum Frame {
    Array { left: usize, state: u8 },
    Object(u8),
}
#[derive(Clone, Copy)]
enum Token {
    None,
    Count {
        value: usize,
        digits: usize,
        zero: bool,
    },
    String {
        key: bool,
        escape: u8,
        hex: u16,
        high: bool,
    },
    Literal {
        word: &'static [u8],
        at: usize,
    },
    Number(u8),
}
pub(crate) struct Scanner {
    at: usize,
    maximum: usize,
    stack: [Frame; DEPTH],
    depth: usize,
    root_done: bool,
    token: Token,
    utf_left: u8,
    utf_min: u8,
    utf_max: u8,
    failed: bool,
}
impl Scanner {
    pub(crate) fn new(maximum: usize) -> Self {
        Self {
            at: 0,
            maximum,
            stack: [Frame::Object(0); DEPTH],
            depth: 0,
            root_done: false,
            token: Token::None,
            utf_left: 0,
            utf_min: 0x80,
            utf_max: 0xbf,
            failed: false,
        }
    }
    pub(super) fn utf8_finished(&self) -> bool {
        self.utf_left == 0
    }
    pub(crate) fn offset(&self) -> usize {
        self.at
    }
    /// The caller retains exactly the same immutable `bytes` across turns.
    /// true means the entire document has passed, including EOF and UTF-8.
    pub(crate) fn step(&mut self, bytes: &[u8], budget: usize) -> Result<bool, Refusal> {
        if self.failed {
            return Err(Refusal::Order);
        }
        let result = self.advance(bytes, budget);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn advance(&mut self, bytes: &[u8], budget: usize) -> Result<bool, Refusal> {
        if bytes.is_empty() || bytes.len() > self.maximum || budget == 0 {
            return Err(Refusal::Bounds);
        }
        let end = self.at.saturating_add(budget).min(bytes.len());
        while self.at < end {
            let b = bytes[self.at];
            // A number/count delimiter is handled by the grammar in the same
            // iteration: no repeated scanning and no zero-byte work loop.
            if self.consume(b)? {
                self.utf8(b)?;
                self.at += 1;
            } else {
                return Err(Refusal::Malformed);
            }
        }
        if self.at != bytes.len() {
            return Ok(false);
        }
        if let Token::Number(state) = self.token {
            if !matches!(state, 2 | 3 | 5 | 8) {
                return Err(Refusal::Malformed);
            }
            self.token = Token::None;
            self.complete()?;
        }
        if !matches!(self.token, Token::None)
            || self.utf_left != 0
            || self.depth != 0
            || !self.root_done
        {
            return Err(Refusal::Malformed);
        }
        Ok(true)
    }
    pub(super) fn utf8(&mut self, b: u8) -> Result<(), Refusal> {
        if self.utf_left != 0 {
            if !(self.utf_min..=self.utf_max).contains(&b) {
                return Err(Refusal::Malformed);
            }
            self.utf_left -= 1;
            self.utf_min = 0x80;
            self.utf_max = 0xbf;
            return Ok(());
        }
        (self.utf_left, self.utf_min, self.utf_max) = match b {
            0..=0x7f => (0, 0x80, 0xbf),
            0xc2..=0xdf => (1, 0x80, 0xbf),
            0xe0 => (2, 0xa0, 0xbf),
            0xe1..=0xec | 0xee..=0xef => (2, 0x80, 0xbf),
            0xed => (2, 0x80, 0x9f),
            0xf0 => (3, 0x90, 0xbf),
            0xf1..=0xf3 => (3, 0x80, 0xbf),
            0xf4 => (3, 0x80, 0x8f),
            _ => return Err(Refusal::Malformed),
        };
        Ok(())
    }
    fn consume(&mut self, b: u8) -> Result<bool, Refusal> {
        match self.token {
            Token::None => self.syntax(b),
            Token::Literal { word, at } => {
                if word.get(at) != Some(&b) {
                    return Err(Refusal::Malformed);
                }
                if at + 1 == word.len() {
                    self.token = Token::None;
                    self.complete()?;
                } else {
                    self.token = Token::Literal { word, at: at + 1 };
                }
                Ok(true)
            }
            Token::Count {
                value,
                digits,
                zero,
            } => {
                if b.is_ascii_digit() {
                    let value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add((b - b'0') as usize))
                        .ok_or(Refusal::Bounds)?;
                    if value > self.maximum {
                        return Err(Refusal::Bounds);
                    }
                    self.token = Token::Count {
                        value,
                        digits: digits + 1,
                        zero: zero || (digits == 0 && b == b'0'),
                    };
                    Ok(true)
                } else {
                    if digits == 0 || (digits > 1 && zero) {
                        return Err(Refusal::Malformed);
                    }
                    self.stack[self.depth - 1] = Frame::Array {
                        left: value,
                        state: 0,
                    };
                    self.token = Token::None;
                    self.syntax(b)
                }
            }
            Token::String {
                key,
                escape,
                hex,
                high,
            } => self.string(b, key, escape, hex, high),
            Token::Number(state) => {
                let next = match (state, b) {
                    (0, b'-') => Some(1),
                    (0 | 1, b'0') => Some(2),
                    (0 | 1, b'1'..=b'9') => Some(3),
                    (3, b'0'..=b'9') => Some(3),
                    (2 | 3, b'.') => Some(4),
                    (4 | 5, b'0'..=b'9') => Some(5),
                    (2 | 3 | 5, b'e' | b'E') => Some(6),
                    (6, b'+' | b'-') => Some(7),
                    (6..=8, b'0'..=b'9') => Some(8),
                    _ => None,
                };
                if let Some(next) = next {
                    self.token = Token::Number(next);
                    Ok(true)
                } else if matches!(state, 2 | 3 | 5 | 8) {
                    self.token = Token::None;
                    self.complete()?;
                    self.syntax(b)
                } else {
                    Err(Refusal::Malformed)
                }
            }
        }
    }
    fn string(
        &mut self,
        b: u8,
        key: bool,
        escape: u8,
        hex: u16,
        high: bool,
    ) -> Result<bool, Refusal> {
        let mut next = Token::String {
            key,
            escape: 0,
            hex: 0,
            high: false,
        };
        match escape {
            0 => match b {
                b'"' => {
                    self.token = Token::None;
                    if key {
                        self.stack[self.depth - 1] = Frame::Object(2);
                    } else {
                        self.complete()?;
                    }
                    return Ok(true);
                }
                b'\\' => {
                    next = Token::String {
                        key,
                        escape: 1,
                        hex: 0,
                        high: false,
                    }
                }
                0..=31 => return Err(Refusal::Malformed),
                _ => {}
            },
            1 => match b {
                b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                b'u' => {
                    next = Token::String {
                        key,
                        escape: 2,
                        hex: 0,
                        high: false,
                    }
                }
                _ => return Err(Refusal::Malformed),
            },
            2..=5 | 8..=11 => {
                let digit = match b {
                    b'0'..=b'9' => b - b'0',
                    b'a'..=b'f' => b - b'a' + 10,
                    b'A'..=b'F' => b - b'A' + 10,
                    _ => return Err(Refusal::Malformed),
                };
                let hex = (hex << 4) | u16::from(digit);
                if matches!(escape, 5 | 11) {
                    if high {
                        if !(0xdc00..=0xdfff).contains(&hex) {
                            return Err(Refusal::Malformed);
                        }
                    } else if (0xd800..=0xdbff).contains(&hex) {
                        next = Token::String {
                            key,
                            escape: 6,
                            hex: 0,
                            high: true,
                        };
                    } else if (0xdc00..=0xdfff).contains(&hex) {
                        return Err(Refusal::Malformed);
                    }
                } else {
                    next = Token::String {
                        key,
                        escape: escape + 1,
                        hex,
                        high,
                    };
                }
            }
            6 if b == b'\\' => {
                next = Token::String {
                    key,
                    escape: 7,
                    hex: 0,
                    high: true,
                }
            }
            7 if b == b'u' => {
                next = Token::String {
                    key,
                    escape: 8,
                    hex: 0,
                    high: true,
                }
            }
            _ => return Err(Refusal::Malformed),
        }
        self.token = next;
        Ok(true)
    }
    fn syntax(&mut self, b: u8) -> Result<bool, Refusal> {
        if matches!(b, b' ' | b'\n' | b'\r' | b'\t') {
            return Ok(true);
        }
        if self.depth == 0 {
            if self.root_done {
                return Err(Refusal::Malformed);
            }
            return self.value(b);
        }
        match self.stack[self.depth - 1] {
            Frame::Array { left, state: 0 } => {
                if left == 0 && b == b']' {
                    self.depth -= 1;
                    self.complete()?;
                    Ok(true)
                } else if left != 0 && b == b',' {
                    self.stack[self.depth - 1] = Frame::Array { left, state: 1 };
                    Ok(true)
                } else {
                    Err(Refusal::Malformed)
                }
            }
            Frame::Array { state: 1, .. } => self.value(b),
            Frame::Array { state: 2, .. } => {
                if !b.is_ascii_digit() {
                    return Err(Refusal::Malformed);
                }
                self.token = Token::Count {
                    value: 0,
                    digits: 0,
                    zero: false,
                };
                self.consume(b)
            }
            Frame::Object(0) if b == b'}' => {
                self.depth -= 1;
                self.complete()?;
                Ok(true)
            }
            Frame::Object(0 | 1) if b == b'"' => {
                self.token = Token::String {
                    key: true,
                    escape: 0,
                    hex: 0,
                    high: false,
                };
                Ok(true)
            }
            Frame::Object(2) if b == b':' => {
                self.stack[self.depth - 1] = Frame::Object(3);
                Ok(true)
            }
            Frame::Object(3) => self.value(b),
            Frame::Object(4) if b == b',' => {
                self.stack[self.depth - 1] = Frame::Object(1);
                Ok(true)
            }
            Frame::Object(4) if b == b'}' => {
                self.depth -= 1;
                self.complete()?;
                Ok(true)
            }
            _ => Err(Refusal::Malformed),
        }
    }
    fn value(&mut self, b: u8) -> Result<bool, Refusal> {
        match b {
            b'[' | b'{' => {
                if self.depth == DEPTH {
                    return Err(Refusal::Bounds);
                }
                self.stack[self.depth] = if b == b'[' {
                    Frame::Array { left: 0, state: 2 }
                } else {
                    Frame::Object(0)
                };
                self.depth += 1;
                Ok(true)
            }
            b'"' => {
                self.token = Token::String {
                    key: false,
                    escape: 0,
                    hex: 0,
                    high: false,
                };
                Ok(true)
            }
            b't' => {
                self.token = Token::Literal {
                    word: b"true",
                    at: 1,
                };
                Ok(true)
            }
            b'f' => {
                self.token = Token::Literal {
                    word: b"false",
                    at: 1,
                };
                Ok(true)
            }
            b'n' => {
                self.token = Token::Literal {
                    word: b"null",
                    at: 1,
                };
                Ok(true)
            }
            b'-' | b'0'..=b'9' => {
                self.token = Token::Number(0);
                self.consume(b)
            }
            _ => Err(Refusal::Malformed),
        }
    }
    fn complete(&mut self) -> Result<(), Refusal> {
        if self.depth == 0 {
            self.root_done = true;
            return Ok(());
        }
        match self.stack[self.depth - 1] {
            Frame::Array { left, state: 1 } if left > 0 => {
                self.stack[self.depth - 1] = Frame::Array {
                    left: left - 1,
                    state: 0,
                };
                Ok(())
            }
            Frame::Object(3) => {
                self.stack[self.depth - 1] = Frame::Object(4);
                Ok(())
            }
            _ => Err(Refusal::Malformed),
        }
    }
}

pub(crate) const STEP_BYTES: usize = 4096;
#[cfg(test)]
mod tests;
