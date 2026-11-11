//! Identity mapping for one AArch64 Root and VideoCore's reserved scanout.
const BLOCK_BYTES: u64 = 2 * 1024 * 1024;
const ACCESS: u64 = 1 << 10;
const INNER_SHAREABLE: u64 = 3 << 8;
const EXECUTE_NEVER: u64 = (1 << 53) | (1 << 54);

const fn descriptor(address: u64) -> u64 {
    let attributes = if address < 0x3000_0000 {
        // Normal write-back RAM; firmware exchanges clean/invalidate explicitly.
        INNER_SHAREABLE
    } else if address < 0x3f00_0000 {
        // Normal noncacheable firmware-reserved framebuffer candidates.
        (1 << 2) | INNER_SHAREABLE | EXECUTE_NEVER
    } else {
        // Device-nGnRnE peripherals, never executable.
        (2 << 2) | EXECUTE_NEVER
    };
    address | ACCESS | attributes | 1
}

#[cfg(target_arch = "aarch64")]
pub(super) fn initialize() {
    use core::cell::UnsafeCell;
    #[repr(C, align(4096))]
    struct Table(UnsafeCell<[u64; 512]>);
    unsafe impl Sync for Table {}
    static L1: Table = Table(UnsafeCell::new([0; 512]));
    static L2: Table = Table(UnsafeCell::new([0; 512]));
    unsafe {
        for index in 0..512 {
            (*L2.0.get())[index] = descriptor(index as u64 * BLOCK_BYTES);
        }
        (*L1.0.get())[0] = L2.0.get() as u64 | 3;
        // BCM2836 local interrupt registers start just above the first GiB.
        (*L1.0.get())[1] = descriptor(0x4000_0000);
        let tcr: u64 = (1 << 23) | (25 << 16) | (3 << 12) | (1 << 10) | (1 << 8) | 25;
        core::arch::asm!(
            "dsb sy", "msr mair_el1, {mair}", "msr tcr_el1, {tcr}",
            "msr ttbr0_el1, {root}", "isb", "tlbi vmalle1", "dsb sy", "isb",
            mair = in(reg) 0x0044ff_u64, tcr = in(reg) tcr,
            root = in(reg) L1.0.get(), options(nostack)
        );
        let mut control: u64;
        core::arch::asm!("mrs {0}, sctlr_el1", out(reg) control, options(nostack));
        control |= 1 | (1 << 2) | (1 << 12);
        core::arch::asm!("msr sctlr_el1, {0}", "isb", in(reg) control, options(nostack));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framebuffer_and_mmio_cannot_inherit_cached_executable_ram_attributes() {
        for index in 0..512 {
            let address = index * BLOCK_BYTES;
            let value = descriptor(address);
            assert_eq!(value & 0x0000_ffff_ffe0_0000, address);
            assert_ne!(value & ACCESS, 0);
            let expected = if address < 0x3000_0000 {
                0
            } else if address < 0x3f00_0000 {
                1
            } else {
                2
            };
            assert_eq!((value >> 2) & 7, expected);
            assert_eq!(value & EXECUTE_NEVER != 0, address >= 0x3000_0000);
        }
        assert_eq!((descriptor(0x4000_0000) >> 2) & 7, 2);
    }
}
