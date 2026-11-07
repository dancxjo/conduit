//! Read-only reuse of the existing private network journal; never writes flash.
use conduit_network_credential_journal::{Journal, Refusal, Storage};
use conduit_rp2040_flash_layout::{CREDENTIALS, FLASH_BYTES};
use embassy_rp::{
    flash::{Blocking, Flash},
    peripherals::FLASH,
};
pub type Device = Flash<'static, FLASH, Blocking, { FLASH_BYTES as usize }>;
pub struct Credentials {
    pub bytes: [u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize],
    pub length: usize,
}
impl Drop for Credentials {
    fn drop(&mut self) {
        for byte in &mut self.bytes {
            unsafe { core::ptr::write_volatile(byte, 0) }
        }
    }
}
struct ReadOnly<'a>(&'a mut Device);
impl Storage for ReadOnly<'_> {
    fn capacity(&self) -> u32 {
        CREDENTIALS.bytes()
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        let offset = CREDENTIALS
            .offset()
            .checked_add(at)
            .ok_or(Refusal::Geometry)?;
        if !CREDENTIALS.contains(offset, out.len().try_into().map_err(|_| Refusal::Capacity)?) {
            return Err(Refusal::Geometry);
        }
        self.0
            .blocking_read(offset, out)
            .map_err(|_| Refusal::Media)
    }
    fn program(&mut self, _: u32, _: &[u8]) -> Result<(), Refusal> {
        Err(Refusal::Media)
    }
    fn erase_sector(&mut self, _: u32) -> Result<(), Refusal> {
        Err(Refusal::Media)
    }
}
#[inline(never)]
pub fn restore(flash: &mut Device) -> Option<Credentials> {
    let mut journal = Journal::open(ReadOnly(flash)).ok()?;
    let mut out = Credentials {
        bytes: [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize],
        length: 0,
    };
    let (_, length) = journal.read(&mut out.bytes).ok()?;
    conduit_net::decode_network_join_request(&out.bytes[..length]).ok()?;
    out.length = length;
    Some(out)
}
