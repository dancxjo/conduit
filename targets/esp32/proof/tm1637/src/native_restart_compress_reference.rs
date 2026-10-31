//! Frozen pre-optimization encoder oracle. Test-only; never production linked.
use core::fmt;
pub const MAGIC: &[u8; 8] = b"BDYLZ004";
const WINDOW: usize = 2048;
const NEAR_WINDOW: usize = 1024;
const V4_HISTORY: usize = 1408;
const CACHE_ENTRIES: usize = (WINDOW - V4_HISTORY) / 32;
const MATCH: usize = 65;
const EOS: u16 = 0xffff;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Capacity,
}
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
// V4 partitions the same 2 KiB arena: a byte history and a FIFO of exact
// packed 64-character lowercase hex runs. Older frames retain their 2 KiB
// history interpretation. Cache entries carry bytes, never semantic identities.
fn cache_insert(history: &mut [u8; WINDOW], len: &mut u8, next: &mut u8, packed: &[u8]) {
    if (0..usize::from(*len))
        .any(|i| history[V4_HISTORY + i * 32..V4_HISTORY + (i + 1) * 32] == *packed)
    {
        return;
    }
    let at = V4_HISTORY + usize::from(*next) * 32;
    history[at..at + 32].copy_from_slice(packed);
    *next = (*next + 1) % CACHE_ENTRIES as u8;
    *len = (*len + 1).min(CACHE_ENTRIES as u8);
}
fn cache_observe(
    history: &mut [u8; WINDOW],
    run: &mut u8,
    len: &mut u8,
    next: &mut u8,
    produced: usize,
    byte: u8,
) {
    if byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase() {
        *run += 1;
    } else {
        *run = 0;
    }
    if *run != 64 {
        return;
    }
    let mut packed = [0; 32];
    let nibble = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
    for (i, out) in packed.iter_mut().enumerate() {
        *out = nibble(history[(produced - 64 + i * 2) % V4_HISTORY]) * 16
            + nibble(history[(produced - 63 + i * 2) % V4_HISTORY]);
    }
    cache_insert(history, len, next, &packed);
    wipe(&mut packed);
    *run = 0;
}
fn hex_digit(n: u8) -> u8 {
    if n < 10 {
        b'0' + n
    } else {
        b'a' + n - 10
    }
}
pub struct Encoder<W> {
    write: W,
    history: [u8; WINDOW],
    cache_len: u8,
    cache_next: u8,
    hex_run: u8,
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
            cache_len: 0,
            cache_next: 0,
            hex_run: 0,
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
        let next = self
            .encoded
            .checked_add(bytes.len())
            .ok_or(Error::Invalid)?;
        if next > self.max_encoded {
            return Err(Error::Capacity);
        }
        (self.write)(bytes).map_err(|_| Error::Invalid)?;
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
        let next = self
            .encoded
            .checked_add(self.group_len)
            .ok_or(Error::Invalid)?;
        if next > self.max_encoded {
            return Err(Error::Capacity);
        }
        (self.write)(&self.group[..self.group_len]).map_err(|_| Error::Invalid)?;
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
        let mut near = 0;
        let mut near_distance = 0;
        for distance in 1..=self.consumed.min(V4_HISTORY) {
            let mut length = 0;
            while length < self.look_len {
                let byte = if length >= distance {
                    self.look[length - distance]
                } else {
                    self.history[(self.consumed + length - distance) % V4_HISTORY]
                };
                if byte != self.look[length] {
                    break;
                }
                length += 1;
            }
            if distance <= NEAR_WINDOW && length > near {
                near = length;
                near_distance = distance;
            }
            if length > best {
                best = length;
                best_distance = distance;
            }
            if best == self.look_len {
                break;
            }
        }
        let mut ordinary_used = if near >= 3 { near } else { 1 };
        let mut ordinary_cost = if near >= 3 { 2 } else { 1 };
        let mut ordinary_distance = near_distance;
        if best_distance > NEAR_WINDOW && best >= 3 && best * ordinary_cost > ordinary_used * 4 {
            ordinary_used = best;
            ordinary_cost = 4;
            ordinary_distance = best_distance;
        }
        if (3..=16).contains(&ordinary_used) && self.look_len > ordinary_used + 2 {
            let mut lazy = false;
            for distance in 1..=(self.consumed + 1).min(V4_HISTORY) {
                let mut length = 0;
                while length + 1 < self.look_len {
                    let byte = if length + 1 >= distance {
                        self.look[length + 1 - distance]
                    } else {
                        self.history[(self.consumed + 1 + length - distance) % V4_HISTORY]
                    };
                    if byte != self.look[length + 1] {
                        break;
                    }
                    length += 1;
                }
                let cost = if distance <= NEAR_WINDOW { 2 } else { 4 };
                if length >= 3 && (length + 1) * ordinary_cost > ordinary_used * (cost + 1) {
                    lazy = true;
                    break;
                }
            }
            if lazy {
                ordinary_used = 1;
                ordinary_cost = 1;
            }
        }
        // Do not consume the beginning of a long hex run as a short suffix of
        // an unrelated match. Align its next token so the exact cached run or
        // nibble packing can cover it completely, independent of write chunks.
        let mut scan = 0;
        while scan < self.look_len {
            let start = scan;
            while scan < self.look_len
                && self.look[scan].is_ascii_hexdigit()
                && !self.look[scan].is_ascii_uppercase()
            {
                scan += 1;
            }
            if start > 0 && scan - start >= 16 && start < ordinary_used {
                ordinary_used = start;
                ordinary_cost = if start < 3 {
                    1
                } else if ordinary_distance > NEAR_WINDOW {
                    4
                } else {
                    2
                };
                break;
            }
            scan += 1;
        }
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
        let mut numbers = [0u8; 21];
        let mut count = 0;
        let mut at = 0;
        while count < numbers.len() && at < self.look_len {
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
        let cached = if self.look_len >= 64 {
            (0..usize::from(self.cache_len)).find(|&i| {
                self.history[V4_HISTORY + i * 32..V4_HISTORY + (i + 1) * 32]
                    .iter()
                    .enumerate()
                    .all(|(at, byte)| {
                        self.look[at * 2] == hex_digit(byte >> 4)
                            && self.look[at * 2 + 1] == hex_digit(byte & 15)
                    })
            })
        } else {
            None
        };
        let used = if let Some(index) = cached.filter(|_| {
            64 * ordinary_cost > ordinary_used * 2
                && (packed_used == 0 || 64 * (packed_len + 2) >= packed_used * 2)
        }) {
            self.token(Some(0xfd80 | index as u16), 0)?;
            64
        } else if packed_used != 0 && packed_used * ordinary_cost > ordinary_used * (packed_len + 2)
        {
            self.packed_token(packed_word, &packed[..packed_len])?;
            packed_used
        } else if ordinary_used >= 3 {
            if ordinary_distance > NEAR_WINDOW {
                self.packed_token(
                    0xfe00 | ordinary_used as u16,
                    &(ordinary_distance as u16).to_le_bytes(),
                )?;
            } else {
                let word = (((ordinary_used - 3) as u16) << 10) | (ordinary_distance - 1) as u16;
                self.token(Some(word), 0)?;
            }
            ordinary_used
        } else {
            self.token(None, self.look[0])?;
            1
        };
        wipe(&mut packed);
        for byte in &self.look[..used] {
            self.history[self.consumed % V4_HISTORY] = *byte;
            self.consumed += 1;
            cache_observe(
                &mut self.history,
                &mut self.hex_run,
                &mut self.cache_len,
                &mut self.cache_next,
                self.consumed,
                *byte,
            );
        }
        self.look.copy_within(used..self.look_len, 0);
        wipe(&mut self.look[self.look_len - used..]);
        self.look_len -= used;
        Ok(())
    }
    fn write_inner(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.begin()?;
        self.plain = self.plain.checked_add(bytes.len()).ok_or(Error::Invalid)?;
        if self.plain > self.max_plain || self.plain > u32::MAX as usize {
            return Err(Error::Capacity);
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
            return Err(Error::Invalid);
        }
        let result = self.write_inner(bytes);
        if result.is_err() {
            self.terminal = true;
        }
        result
    }
    pub fn finish(&mut self) -> Result<Lengths, Error> {
        if self.terminal {
            return Err(Error::Invalid);
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
