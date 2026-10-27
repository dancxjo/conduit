//! Bounded lossless framing for authenticated private restart JSON. This codec
//! provides no authentication: the existing journal hashes the complete frame.
use core::fmt;
pub const MAGIC: &[u8; 8] = b"BDYLZ002";
const WINDOW: usize = 1024;
const MATCH: usize = 65;
const EOS: u16 = 0xffff;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("restart compression refused")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lengths {
    pub plain: usize,
    pub encoded: usize,
}
fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}
pub struct Encoder<W> {
    write: W,
    history: [u8; WINDOW],
    look: [u8; MATCH],
    look_len: usize,
    consumed: usize,
    plain: usize,
    encoded: usize,
    max_plain: usize,
    max_encoded: usize,
    group: [u8; 273],
    group_len: usize,
    tokens: u8,
    begun: bool,
    terminal: bool,
}
impl<W> Drop for Encoder<W> {
    fn drop(&mut self) {
        wipe(&mut self.history);
        wipe(&mut self.look);
        wipe(&mut self.group);
    }
}
impl<W: FnMut(&[u8]) -> Result<(), ()>> Encoder<W> {
    pub fn new(write: W, max_plain: usize, max_encoded: usize) -> Self {
        Self {
            write,
            history: [0; WINDOW],
            look: [0; MATCH],
            look_len: 0,
            consumed: 0,
            plain: 0,
            encoded: 0,
            max_plain,
            max_encoded,
            group: [0; 273],
            group_len: 1,
            tokens: 0,
            begun: false,
            terminal: false,
        }
    }
    fn emit(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let next = self.encoded.checked_add(bytes.len()).ok_or(Error)?;
        if next > self.max_encoded {
            return Err(Error);
        }
        (self.write)(bytes).map_err(|_| Error)?;
        self.encoded = next;
        Ok(())
    }
    fn begin(&mut self) -> Result<(), Error> {
        if !self.begun {
            self.emit(MAGIC)?;
            self.begun = true;
        }
        Ok(())
    }
    fn flush_group(&mut self) -> Result<(), Error> {
        // Split field borrowing without copying private bytes onto another frame.
        let next = self.encoded.checked_add(self.group_len).ok_or(Error)?;
        if next > self.max_encoded {
            return Err(Error);
        }
        (self.write)(&self.group[..self.group_len]).map_err(|_| Error)?;
        self.encoded = next;
        wipe(&mut self.group);
        self.group_len = 1;
        self.tokens = 0;
        Ok(())
    }
    fn token(&mut self, word: Option<u16>, literal: u8) -> Result<(), Error> {
        if let Some(word) = word {
            self.group[0] |= 1 << self.tokens;
            let encoded = word.to_le_bytes();
            self.group[self.group_len..self.group_len + 2].copy_from_slice(&encoded);
            self.group_len += 2;
        } else {
            self.group[self.group_len] = literal;
            self.group_len += 1;
        }
        self.tokens += 1;
        if self.tokens == 8 {
            self.flush_group()?;
        }
        Ok(())
    }
    fn packed_token(&mut self, word: u16, payload: &[u8]) -> Result<(), Error> {
        self.group[0] |= 1 << self.tokens;
        self.group[self.group_len..self.group_len + 2].copy_from_slice(&word.to_le_bytes());
        self.group_len += 2;
        self.group[self.group_len..self.group_len + payload.len()].copy_from_slice(payload);
        self.group_len += payload.len();
        self.tokens += 1;
        if self.tokens == 8 {
            self.flush_group()?;
        }
        Ok(())
    }
    fn consume(&mut self) -> Result<(), Error> {
        let mut best = 0;
        let mut best_distance = 0;
        for distance in 1..=self.consumed.min(WINDOW) {
            let mut length = 0;
            while length < self.look_len {
                let byte = if length >= distance {
                    self.look[length - distance]
                } else {
                    self.history[(self.consumed + length - distance) % WINDOW]
                };
                if byte != self.look[length] {
                    break;
                }
                length += 1;
            }
            if length > best {
                best = length;
                best_distance = distance;
            }
            if best == self.look_len {
                break;
            }
        }
        let ordinary_used = if best >= 3 { best } else { 1 };
        let ordinary_cost = if best >= 3 { 2 } else { 1 };
        let mut packed = [0u8; 32];
        let mut packed_len = 0;
        let mut packed_used = 0;
        let mut packed_word = 0;
        let mut hex_len = 0;
        while hex_len < self.look_len.min(64)
            && self.look[hex_len].is_ascii_hexdigit()
            && !self.look[hex_len].is_ascii_uppercase()
        {
            hex_len += 1;
        }
        hex_len &= !1;
        if hex_len >= 16 {
            for (i, out) in packed[..hex_len / 2].iter_mut().enumerate() {
                fn nibble(b: u8) -> u8 {
                    if b <= b'9' {
                        b - b'0'
                    } else {
                        b - b'a' + 10
                    }
                }
                *out = nibble(self.look[i * 2]) * 16 + nibble(self.look[i * 2 + 1]);
            }
            packed_len = hex_len / 2;
            packed_used = hex_len;
            packed_word = 0xfd00 | packed_len as u16;
        }
        let mut numbers = [0u8; 16];
        let mut count = 0;
        let mut at = 0;
        while count < 16 && at < self.look_len {
            let start = at;
            let mut value = 0u16;
            while at < self.look_len && self.look[at].is_ascii_digit() {
                value = value * 10 + u16::from(self.look[at] - b'0');
                at += 1;
                if value > 255 || at - start > 3 {
                    break;
                }
            }
            if at == start
                || value > 255
                || at - start > 3
                || (at - start > 1 && self.look[start] == b'0')
            {
                break;
            }
            // A full lookahead ending inside a number cannot establish its spelling.
            if at == self.look_len && self.look_len == MATCH {
                break;
            }
            numbers[count] = value as u8;
            count += 1;
            if count >= 4 && (packed_used == 0 || at * (packed_len + 2) > packed_used * (count + 2))
            {
                packed[..count].copy_from_slice(&numbers[..count]);
                packed_len = count;
                packed_used = at;
                packed_word = 0xfc00 | count as u16;
            }
            if at == self.look_len || self.look[at] != b',' {
                break;
            }
            at += 1;
        }
        wipe(&mut numbers);
        let used =
            if packed_used != 0 && packed_used * ordinary_cost > ordinary_used * (packed_len + 2) {
                self.packed_token(packed_word, &packed[..packed_len])?;
                packed_used
            } else if best >= 3 {
                let word = (((best - 3) as u16) << 10) | (best_distance - 1) as u16;
                self.token(Some(word), 0)?;
                best
            } else {
                self.token(None, self.look[0])?;
                1
            };
        wipe(&mut packed);
        for byte in &self.look[..used] {
            self.history[self.consumed % WINDOW] = *byte;
            self.consumed += 1;
        }
        self.look.copy_within(used..self.look_len, 0);
        wipe(&mut self.look[self.look_len - used..]);
        self.look_len -= used;
        Ok(())
    }
    fn write_inner(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.begin()?;
        self.plain = self.plain.checked_add(bytes.len()).ok_or(Error)?;
        if self.plain > self.max_plain || self.plain > u32::MAX as usize {
            return Err(Error);
        }
        for byte in bytes {
            self.look[self.look_len] = *byte;
            self.look_len += 1;
            if self.look_len == MATCH {
                self.consume()?;
            }
        }
        Ok(())
    }
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.terminal {
            return Err(Error);
        }
        let result = self.write_inner(bytes);
        if result.is_err() {
            self.terminal = true;
        }
        result
    }
    pub fn finish(&mut self) -> Result<Lengths, Error> {
        if self.terminal {
            return Err(Error);
        }
        self.terminal = true;
        self.begin()?;
        while self.look_len != 0 {
            self.consume()?;
        }
        self.token(Some(EOS), 0)?;
        if self.tokens != 0 {
            self.flush_group()?;
        }
        self.emit(&(self.plain as u32).to_le_bytes())?;
        Ok(Lengths {
            plain: self.plain,
            encoded: self.encoded,
        })
    }
}
pub struct Decoder<R> {
    read: R,
    history: [u8; WINDOW],
    input: [u8; 128],
    position: usize,
    available: usize,
    encoded: usize,
    produced: usize,
    max_plain: usize,
    max_encoded: usize,
    flags: u8,
    flag_index: u8,
    distance: usize,
    remaining: usize,
    begun: bool,
    done: bool,
    failed: bool,
    v2: bool,
    expansion: [u8; 64],
    expansion_at: usize,
    expansion_len: usize,
}
impl<R> Drop for Decoder<R> {
    fn drop(&mut self) {
        wipe(&mut self.history);
        wipe(&mut self.input);
        wipe(&mut self.expansion);
    }
}
impl<R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> Decoder<R> {
    pub fn new(read: R, max_plain: usize, max_encoded: usize) -> Self {
        Self {
            read,
            history: [0; WINDOW],
            input: [0; 128],
            position: 0,
            available: 0,
            encoded: 0,
            produced: 0,
            max_plain,
            max_encoded,
            flags: 0,
            flag_index: 8,
            distance: 0,
            remaining: 0,
            begun: false,
            done: false,
            failed: false,
            v2: false,
            expansion: [0; 64],
            expansion_at: 0,
            expansion_len: 0,
        }
    }
    fn byte(&mut self) -> Result<Option<u8>, Error> {
        if self.position == self.available {
            wipe(&mut self.input);
            let count = (self.read)(&mut self.input).map_err(|_| Error)?;
            if count > self.input.len() {
                return Err(Error);
            }
            self.encoded = self.encoded.checked_add(count).ok_or(Error)?;
            if self.encoded > self.max_encoded {
                return Err(Error);
            }
            self.available = count;
            self.position = 0;
            if count == 0 {
                return Ok(None);
            }
        }
        let byte = self.input[self.position];
        self.position += 1;
        Ok(Some(byte))
    }
    fn required(&mut self) -> Result<u8, Error> {
        self.byte()?.ok_or(Error)
    }
    fn begin(&mut self) -> Result<(), Error> {
        if !self.begun {
            for byte in &MAGIC[..7] {
                if self.required()? != *byte {
                    return Err(Error);
                }
            }
            self.v2 = match self.required()? {
                b'1' => false,
                b'2' => true,
                _ => return Err(Error),
            };
            self.begun = true;
        }
        Ok(())
    }
    fn next_plain(&mut self) -> Result<Option<u8>, Error> {
        if self.expansion_at < self.expansion_len {
            let byte = self.expansion[self.expansion_at];
            self.expansion[self.expansion_at] = 0;
            self.expansion_at += 1;
            return Ok(Some(byte));
        }
        if self.remaining != 0 {
            let byte = self.history[(self.produced - self.distance) % WINDOW];
            self.remaining -= 1;
            return Ok(Some(byte));
        }
        if self.flag_index == 8 {
            self.flags = self.required()?;
            self.flag_index = 0;
        }
        let matched = self.flags & (1 << self.flag_index) != 0;
        self.flag_index += 1;
        if !matched {
            return self.required().map(Some);
        }
        let word = u16::from_le_bytes([self.required()?, self.required()?]);
        if word == EOS {
            if self.flag_index < 8 && self.flags >> self.flag_index != 0 {
                return Err(Error);
            }
            let length = u32::from_le_bytes([
                self.required()?,
                self.required()?,
                self.required()?,
                self.required()?,
            ]) as usize;
            if length != self.produced || self.byte()?.is_some() {
                return Err(Error);
            }
            self.done = true;
            return Ok(None);
        }
        if self.v2 && (word & 0xff00 == 0xfc00 || word & 0xff00 == 0xfd00) {
            let count = usize::from(word & 255);
            self.expansion_at = 0;
            self.expansion_len = 0;
            if word & 0xff00 == 0xfd00 {
                if !(8..=32).contains(&count) {
                    return Err(Error);
                }
                for _ in 0..count {
                    let b = self.required()?;
                    for n in [b >> 4, b & 15] {
                        self.expansion[self.expansion_len] =
                            if n < 10 { b'0' + n } else { b'a' + n - 10 };
                        self.expansion_len += 1;
                    }
                }
            } else {
                if !(4..=16).contains(&count) {
                    return Err(Error);
                }
                for i in 0..count {
                    let b = self.required()?;
                    if i != 0 {
                        self.expansion[self.expansion_len] = b',';
                        self.expansion_len += 1;
                    }
                    for digit in [b / 100, (b / 10) % 10, b % 10].iter().enumerate() {
                        if digit.0 == 0 && b < 100 || digit.0 == 1 && b < 10 {
                            continue;
                        }
                        self.expansion[self.expansion_len] = b'0' + *digit.1;
                        self.expansion_len += 1;
                    }
                }
            }
            if self.produced.checked_add(self.expansion_len).ok_or(Error)? > self.max_plain {
                return Err(Error);
            }
            return self.next_plain();
        }
        let length = usize::from(word >> 10) + 3;
        let distance = usize::from(word & 1023) + 1;
        if length > MATCH || distance > self.produced.min(WINDOW) {
            return Err(Error);
        }
        if self.produced.checked_add(length).ok_or(Error)? > self.max_plain {
            return Err(Error);
        }
        self.distance = distance;
        self.remaining = length;
        self.next_plain()
    }
    fn read_inner(&mut self, output: &mut [u8; 128]) -> Result<usize, Error> {
        if self.done {
            return Ok(0);
        }
        self.begin()?;
        let mut count = 0;
        while count < output.len() {
            let Some(byte) = self.next_plain()? else {
                break;
            };
            if self.produced >= self.max_plain {
                return Err(Error);
            }
            self.history[self.produced % WINDOW] = byte;
            self.produced += 1;
            output[count] = byte;
            count += 1;
        }
        Ok(count)
    }
    pub fn read(&mut self, output: &mut [u8; 128]) -> Result<usize, Error> {
        if self.failed {
            return Err(Error);
        }
        let result = self.read_inner(output);
        if result.is_err() {
            self.failed = true;
            wipe(output);
        }
        result
    }
}
