//! Supported Pico W/W25Q16JV profile only. No clock, region or ID grants access.
//! Reviewed chip maxima: 400ms sector erase + five 3ms page programs; allow
//! 335ms for bounded ROM/XIP setup, readback/CRC at the fixed125MHz system clock.
//! https://datasheets.raspberrypi.com/picow/pico-w-datasheet.pdf
//! https://cdn-learn.adafruit.com/assets/assets/000/116/709/original/datasheetQspi.pdf
//! This is an in-spec transaction envelope, not recovery from a stuck ROM/chip.
pub(super) use super::policy::BUDGET_MS;
use super::Device;
use conduit_monotonic_wait::flash_window::FlashPermit;
use conduit_network_join_call::credential_journal::{Refusal, Storage};
use conduit_rp2040_flash_layout::{CREDENTIALS, ERASE_BYTES};
pub(super) struct Access<'a, 'p, 'w, 'g> {
    flash: &'a mut Device,
    permit: Option<&'p FlashPermit<'w, 'g>>,
}
impl<'a, 'p, 'w, 'g> Access<'a, 'p, 'w, 'g> {
    pub(super) fn read_only(flash: &'a mut Device) -> Self {
        Self {
            flash,
            permit: None,
        }
    }
    pub(super) fn writable(flash: &'a mut Device, permit: &'p FlashPermit<'w, 'g>) -> Self {
        Self {
            flash,
            permit: Some(permit),
        }
    }
    fn offset(at: u32, bytes: usize) -> Result<u32, Refusal> {
        let bytes = u32::try_from(bytes).map_err(|_| Refusal::Capacity)?;
        let offset = CREDENTIALS
            .offset()
            .checked_add(at)
            .ok_or(Refusal::Geometry)?;
        if !CREDENTIALS.contains(offset, bytes) {
            return Err(Refusal::Geometry);
        }
        Ok(offset)
    }
}
impl Storage for Access<'_, '_, '_, '_> {
    fn capacity(&self) -> u32 {
        CREDENTIALS.bytes()
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        self.flash
            .blocking_read(Self::offset(at, out.len())?, out)
            .map_err(|_| Refusal::Media)
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        if self.permit.is_none() {
            return Err(Refusal::Media);
        }
        self.flash
            .blocking_write(Self::offset(at, bytes.len())?, bytes)
            .map_err(|_| Refusal::Media)
    }
    fn erase_sector(&mut self, at: u32) -> Result<(), Refusal> {
        if self.permit.is_none() || !at.is_multiple_of(ERASE_BYTES) {
            return Err(Refusal::Geometry);
        }
        let offset = Self::offset(at, ERASE_BYTES as usize)?;
        self.flash
            .blocking_erase(offset, offset + ERASE_BYTES)
            .map_err(|_| Refusal::Media)
    }
}
/// Called in the SAME outer critical section as the complete synchronous commit.
/// Valid only for this initialized, pinned profile: DMA0/1 and optional DMA2
/// use non-chaining PIO transfers; no DMA control-register writer exists.
/// Embassy initialization resets the other channels. CORE1 remains unstarted
/// and interrupts remain masked through commit. Thus BUSY can only transition
/// true to false during this sequential scan; observed idle channels stay idle.
/// A changed DMA or core1 profile requires renewed qualification.
pub(super) fn quiescent() -> bool {
    use embassy_rp::pac;
    pac::SIO.cpuid().read() == 0
        && (0..12).all(|n| !pac::DMA.ch(n).ctrl_trig().read().busy())
        && pac::XIP_CTRL.stream_ctr().read().0 == 0
        && embassy_rp::clocks::clk_sys_freq() == 125_000_000
}
