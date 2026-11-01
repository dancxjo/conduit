#![no_std]
//! Existing CPBR carrier bytes, not a Conduit membership or authority protocol.
//! Bounded framing owns no device, network, GPIO, BOOTSEL or Body permission.
//! Command meaning and admission remain with each actual initialized owner.
pub const HEADER: usize = 12;
pub const MAX_REQUEST: usize = 8192;
const MAGIC: &[u8; 4] = b"CPBR";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Header,
    Version,
    Kind,
    Capacity,
    Pipeline,
    Incomplete,
    Stale,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader {
    pub kind: u8,
    pub id: u32,
    pub length: usize,
}
#[derive(Debug)]
pub struct Frame<'a> {
    pub kind: u8,
    pub id: u32,
    pub payload: &'a [u8],
}
#[inline(never)]
pub fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { core::ptr::write_volatile(byte, 0) }
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
pub fn decode_header(bytes: &[u8; HEADER]) -> Result<FrameHeader, Refusal> {
    if &bytes[..4] != MAGIC {
        return Err(Refusal::Header);
    }
    if bytes[4] != 1 {
        return Err(Refusal::Version);
    }
    Ok(FrameHeader {
        kind: bytes[5],
        id: u32::from_le_bytes(bytes[6..10].try_into().unwrap()),
        length: u16::from_le_bytes(bytes[10..12].try_into().unwrap()) as usize,
    })
}
/// Extraction of the existing decoder's bounded accumulation. A caller supplies
/// its independent command policy, checked before any payload is accumulated.
/// N includes the fixed header. No heap or effectful command is present here.
pub struct FrameDecoder<const N: usize> {
    bytes: [u8; N],
    used: usize,
    expected: Option<usize>,
}
impl<const N: usize> Default for FrameDecoder<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> FrameDecoder<N> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            used: 0,
            expected: None,
        }
    }
    pub fn reset(&mut self) {
        wipe(&mut self.bytes);
        self.used = 0;
        self.expected = None;
    }
    pub fn accept(
        &mut self,
        input: &[u8],
        mut policy: impl FnMut(FrameHeader) -> Result<(), Refusal>,
    ) -> Result<(), Refusal> {
        for &byte in input {
            if self.expected == Some(self.used) {
                return Err(Refusal::Pipeline);
            }
            let out = self.bytes.get_mut(self.used).ok_or(Refusal::Capacity)?;
            *out = byte;
            self.used += 1;
            if self.used == HEADER {
                let parsed = decode_header(self.bytes[..HEADER].try_into().unwrap())?;
                policy(parsed)?;
                if parsed.length > N.saturating_sub(HEADER) {
                    return Err(Refusal::Capacity);
                }
                self.expected = Some(HEADER + parsed.length);
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
impl<const N: usize> Drop for FrameDecoder<N> {
    fn drop(&mut self) {
        self.reset()
    }
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
pub struct Scratch<const N: usize>(pub [u8; N]);
impl<const N: usize> Default for Scratch<N> {
    fn default() -> Self {
        Self([0; N])
    }
}
impl<const N: usize> Drop for Scratch<N> {
    fn drop(&mut self) {
        wipe(&mut self.0)
    }
}
mod information;
pub use information::{InformationDecoder, InformationRequest, InformationSession};
