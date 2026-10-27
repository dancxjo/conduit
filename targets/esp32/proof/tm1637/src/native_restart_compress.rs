//! Bounded lossless framing for authenticated private restart JSON. This codec
//! provides no authentication: the existing journal hashes the complete frame.
use core::fmt;
pub const MAGIC: &[u8; 8] = b"BDYLZ001";
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
    group: [u8; 17],
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
            group: [0; 17],
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
        let used = if best >= 3 {
            let word = (((best - 3) as u16) << 10) | (best_distance - 1) as u16;
            self.token(Some(word), 0)?;
            best
        } else {
            self.token(None, self.look[0])?;
            1
        };
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
}
impl<R> Drop for Decoder<R> {
    fn drop(&mut self) {
        wipe(&mut self.history);
        wipe(&mut self.input);
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
            for byte in MAGIC {
                if self.required()? != *byte {
                    return Err(Error);
                }
            }
            self.begun = true;
        }
        Ok(())
    }
    fn next_plain(&mut self) -> Result<Option<u8>, Error> {
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
