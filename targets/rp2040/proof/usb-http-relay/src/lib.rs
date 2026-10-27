//! Bounded transport framing only. No Body, native capability or HTTP authority
//! is established here. The native endpoint is fixed to 192.168.4.1:80.
#![no_std]
pub const ROLE: &str = "conduit.pico-usb-http-relay@1";
pub const HEADER: usize = 12;
pub const MAX_REQUEST: usize = 8192;
pub const MAX_RESPONSE: usize = 65536;
pub const CHUNK: usize = 512;
const MAGIC: &[u8; 4] = b"CPBR";
pub const JOIN: u8 = 1;
pub const REQUEST: u8 = 2;
pub const STATUS: u8 = 3;
pub const DATA: u8 = 128;
pub const DONE: u8 = 129;
pub const ERROR: u8 = 130;
pub const INFORMATION: u8 = 131;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Header,
    Version,
    Kind,
    Capacity,
    Pipeline,
    Incomplete,
    Stale,
    Credentials,
}
#[inline(never)]
pub fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { core::ptr::write_volatile(byte, 0) }
    }
}
pub struct Decoder {
    bytes: [u8; HEADER + MAX_REQUEST],
    used: usize,
    expected: Option<usize>,
}
impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}
impl Decoder {
    pub const fn new() -> Self {
        Self {
            bytes: [0; HEADER + MAX_REQUEST],
            used: 0,
            expected: None,
        }
    }
    pub fn reset(&mut self) {
        wipe(&mut self.bytes);
        self.used = 0;
        self.expected = None;
    }
    pub fn accept(&mut self, input: &[u8]) -> Result<(), Refusal> {
        for &byte in input {
            if self.expected == Some(self.used) {
                return Err(Refusal::Pipeline);
            }
            let out = self.bytes.get_mut(self.used).ok_or(Refusal::Capacity)?;
            *out = byte;
            self.used += 1;
            if self.used == HEADER {
                if &self.bytes[..4] != MAGIC {
                    return Err(Refusal::Header);
                }
                if self.bytes[4] != 1 {
                    return Err(Refusal::Version);
                }
                let length = u16::from_le_bytes([self.bytes[10], self.bytes[11]]) as usize;
                match self.bytes[5] {
                    JOIN if (2..=97).contains(&length) => {}
                    REQUEST if (1..=MAX_REQUEST).contains(&length) => {}
                    STATUS if length == 0 => {}
                    JOIN | REQUEST | STATUS => return Err(Refusal::Capacity),
                    _ => return Err(Refusal::Kind),
                }
                self.expected = Some(HEADER + length);
            }
        }
        Ok(())
    }
    pub fn has_partial(&self) -> bool {
        self.used != 0
    }
    pub fn complete(&self) -> bool {
        self.expected == Some(self.used)
    }
    pub fn frame(&self) -> Result<Frame<'_>, Refusal> {
        if !self.complete() {
            return Err(Refusal::Incomplete);
        }
        Ok(Frame {
            kind: self.bytes[5],
            id: u32::from_le_bytes(self.bytes[6..10].try_into().unwrap()),
            payload: &self.bytes[HEADER..self.used],
        })
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        self.reset();
    }
}
#[derive(Debug)]
pub struct Frame<'a> {
    pub kind: u8,
    pub id: u32,
    pub payload: &'a [u8],
}
pub struct Credentials<'a> {
    pub ssid: &'a str,
    pub password: &'a [u8],
}
pub fn credentials(bytes: &[u8]) -> Result<Credentials<'_>, Refusal> {
    let (&ssid_len, tail) = bytes.split_first().ok_or(Refusal::Credentials)?;
    let (&password_len, tail) = tail.split_first().ok_or(Refusal::Credentials)?;
    let (ssid_len, password_len) = (usize::from(ssid_len), usize::from(password_len));
    if !(1..=32).contains(&ssid_len)
        || password_len > 63
        || (password_len != 0 && password_len < 8)
        || tail.len() != ssid_len + password_len
    {
        return Err(Refusal::Credentials);
    }
    let ssid = core::str::from_utf8(&tail[..ssid_len]).map_err(|_| Refusal::Credentials)?;
    if ssid.as_bytes().contains(&0) || tail[ssid_len..].contains(&0) {
        return Err(Refusal::Credentials);
    }
    Ok(Credentials {
        ssid,
        password: &tail[ssid_len..],
    })
}
#[derive(Default)]
pub struct Session {
    last: u32,
}
impl Session {
    pub fn admit(&mut self, id: u32) -> Result<(), Refusal> {
        if id == 0 || id <= self.last {
            return Err(Refusal::Stale);
        }
        self.last = id;
        Ok(())
    }
    pub fn reset(&mut self) {
        self.last = 0;
    }
}
pub fn header(kind: u8, id: u32, length: usize) -> Result<[u8; HEADER], Refusal> {
    if length > MAX_REQUEST {
        return Err(Refusal::Capacity);
    }
    let mut out = [0; HEADER];
    out[..4].copy_from_slice(MAGIC);
    out[4] = 1;
    out[5] = kind;
    out[6..10].copy_from_slice(&id.to_le_bytes());
    out[10..12].copy_from_slice(&(length as u16).to_le_bytes());
    Ok(out)
}

/// Require the exact on-link ESP AP subnet; a default router is unnecessary.
/// Refuse an advertised router other than the fixed ESP peer.
pub fn peer_network(address: [u8; 4], prefix: u8, gateway: Option<[u8; 4]>) -> bool {
    address[..3] == [192, 168, 4]
        && (2..=254).contains(&address[3])
        && prefix == 24
        && (gateway.is_none() || gateway == Some([192, 168, 4, 1]))
}

/// Fixed sensitive scratch, also wiped when a cancelled future drops it.
pub struct Scratch<const N: usize>(pub [u8; N]);
impl<const N: usize> Default for Scratch<N> {
    fn default() -> Self {
        Self([0; N])
    }
}
impl<const N: usize> Drop for Scratch<N> {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}
