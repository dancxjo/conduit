//! Tegra210 native retained-display access. All controller writes are limited
//! to the two read selectors restored by display_boot's transaction.
//!
//! Register provenance: Linux arch/arm64/boot/dts/nvidia/tegra210.dtsi assigns
//! DISP1/DC0 clock and reset 27, DISP2/DC1 clock and reset 26, HOST1X 28.
//! drivers/clk/tegra/clk.h defines RST_DEVICES_L=0x004, CLK_OUT_ENB_L=0x010;
//! drivers/clk/tegra/clk.c maps reset IDs 0..31 to that register's bit positions.
//! https://github.com/torvalds/linux/blob/v6.12/arch/arm64/boot/dts/nvidia/tegra210.dtsi
//! https://github.com/torvalds/linux/blob/v6.12/drivers/clk/tegra/clk.h
//! https://github.com/torvalds/linux/blob/v6.12/drivers/clk/tegra/clk.c

use super::display::Descriptor;
use super::display_boot::{self, Error, Registers, DC_BASES, MC_BASE};
use super::memory::{Region, Regions};

const CAR_BASE: u64 = 0x6000_6000;
const RESET_LOW: u64 = 0x004;
const CLOCK_LOW: u64 = 0x010;

fn barrier() {
    #[cfg(target_arch = "aarch64")]
    // SAFETY: DSB orders/completes this Root-owned MMIO transaction; it does
    // not change memory mappings or architectural privilege state.
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags))
    };
    #[cfg(not(target_arch = "aarch64"))]
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

unsafe fn read(address: u64) -> u32 {
    barrier();
    // SAFETY: The private caller derives aligned addresses inside the exact
    // device apertures required by probe's external safety contract.
    let value = unsafe { core::ptr::read_volatile(address as *const u32) };
    barrier();
    u32::from_le(value)
}

unsafe fn write_selector(address: u64, value: u32) {
    barrier();
    // SAFETY: The private caller admits only aligned DC selector registers.
    unsafe { core::ptr::write_volatile(address as *mut u32, value.to_le()) };
    barrier();
}

fn eligible(controller: usize, clocks: u32, resets: u32) -> bool {
    let bit = match controller {
        0 => 27,
        1 => 26,
        _ => return false,
    };
    let required = (1 << bit) | (1 << 28); // DC plus its HOST1X register bus.
    clocks & required == required && resets & required == 0
}

struct NativeRegisters;

impl Registers for NativeRegisters {
    fn controller_accessible(&mut self, controller: usize) -> bool {
        // SAFETY: NativeRegisters exists only during the unsafe probe below;
        // its contract provides CAR access and quiescent firmware clocks.
        let clocks = unsafe { read(CAR_BASE + CLOCK_LOW) };
        // SAFETY: Same admitted CAR aperture and ordering contract.
        let resets = unsafe { read(CAR_BASE + RESET_LOW) };
        eligible(controller, clocks, resets)
    }

    fn read_dc(&mut self, controller: usize, word: u32) -> u32 {
        // display_boot's finite register set ends at word 0x80d; this guard
        // also fences future callers away from an unadmitted DC aperture.
        let Some(base) = DC_BASES.get(controller) else {
            return u32::MAX;
        };
        if word > 0x80d {
            return u32::MAX;
        }
        // SAFETY: display_boot checks accessibility before any DC read and
        // probe's caller retains the complete register aperture as device MMIO.
        unsafe { read(*base + u64::from(word) * 4) }
    }

    fn write_dc(&mut self, controller: usize, word: u32, value: u32) {
        let Some(base) = DC_BASES.get(controller) else {
            return;
        };
        if !matches!(word, 0x040 | 0x042) {
            return;
        }
        // SAFETY: Only admitted state-read/window selectors are writable;
        // display_boot saves/restores them while firmware/other cores quiesce.
        unsafe { write_selector(*base + u64::from(word) * 4, value) };
    }

    fn read_mc(&mut self, byte: u32) -> u32 {
        if !matches!(byte, 0x010 | 0x228 | 0x240 | 0x244) {
            return u32::MAX;
        }
        // SAFETY: SMMU global/client/ASID registers are inside the admitted MC aperture.
        unsafe { read(MC_BASE + u64::from(byte)) }
    }
}

/// Discover one retained firmware scanout without changing clocks or modes.
/// `framebuffer` must come from the actual boot DTB/bootargs; `protected` must
/// contain every image/stack/DTB/page-table/live firmware range except this
/// exact framebuffer allocation, whose ownership is explicitly transferred.
///
/// # Safety
/// Run only on an admitted Tegra210 Nano primary core in little-endian Root,
/// with no concurrent firmware/secondary-core/controller users. CAR
/// 0x60006000..0x60007000, MC 0x70019000..0x7001a000 and both DC register
/// apertures (DC_BASES, 0x40000 bytes each) must be safely identity-accessible
/// device MMIO. Live firmware must have initialized/preserved power, parent
/// clocks and HDMI scanout; no power/clock owner may change them during or
/// after discovery. The framebuffer must be CPU-accessible physical memory,
/// not an unverified device virtual address or arbitrary RAM supplied as one.
/// The caller must retain all firmware reservations and read access authority.
pub unsafe fn probe(framebuffer: Region, protected: &Regions) -> Result<Descriptor, Error> {
    display_boot::probe(&mut NativeRegisters, framebuffer, protected)
}

/// Paint bounded early-boot text to an admitted retained scanout.
///
/// # Safety
/// `descriptor` must be the unchanged successful result of this module's
/// `probe` for the current boot. Its complete framebuffer must remain uniquely
/// CPU-writable at the same identity address, accept aligned volatile 32-bit
/// stores, and be coherent with display DMA (uncached with the data cache
/// disabled is sufficient). Preserve all clock, mapping and exclusive-owner
/// guarantees from `probe`; exclude ordinary Face/compositor writes during
/// this diagnostic. This function supplies no allocation or memory authority.
pub unsafe fn diagnostic(descriptor: Descriptor, message: &str) -> Result<(), Error> {
    let address =
        usize::try_from(descriptor.memory.start()).map_err(|_| Error::DiagnosticBounds)?;
    let bytes = usize::try_from(descriptor.memory.bytes()).map_err(|_| Error::DiagnosticBounds)?;
    if address & 3 != 0 || address.checked_add(bytes).is_none() {
        return Err(Error::DiagnosticBounds);
    }
    barrier();
    let result = display_boot::diagnostic(descriptor, message, |offset, pixel| {
        // SAFETY: diagnostic checks each aligned pixel against the admitted
        // allocation; external contract establishes mapping and sole ownership.
        unsafe { core::ptr::write_volatile((address + offset) as *mut u32, pixel.to_le()) };
    });
    barrier();
    result
}

#[cfg(test)]
mod tests {
    use super::eligible;

    #[test]
    fn dc_and_host_bus_must_both_be_clocked_and_out_of_reset() {
        let both_heads = (1 << 26) | (1 << 27) | (1 << 28);
        assert!(eligible(0, both_heads, 0));
        assert!(eligible(1, both_heads, 0));
        assert!(!eligible(2, both_heads, 0));
        for controller in 0..2 {
            let bit = if controller == 0 { 27 } else { 26 };
            assert!(!eligible(controller, both_heads & !(1 << bit), 0));
            assert!(!eligible(controller, both_heads & !(1 << 28), 0));
            assert!(!eligible(controller, both_heads, 1 << bit));
            assert!(!eligible(controller, both_heads, 1 << 28));
        }
    }
}
