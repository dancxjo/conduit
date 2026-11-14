//! ARM1176 short-descriptor and control policy, independent of register access.
const ARM_RAM_END: u32 = 0x1c00_0000; // Firmware image reserves the upper 64 MiB.
const RAM_END: u32 = 0x2000_0000;
const SECTION: u32 = 0b10;
const PRIVILEGED_READ_WRITE: u32 = 1 << 10;
const NORMAL_NONCACHEABLE: u32 = 1 << 12;
const NORMAL_WRITE_THROUGH: u32 = 1 << 3;
const SHARED_DEVICE: u32 = 1 << 2;
const EXECUTE_NEVER: u32 = 1 << 4;

pub(super) const fn section_descriptor(address: u32) -> u32 {
    let attributes = if address < ARM_RAM_END {
        NORMAL_WRITE_THROUGH
    } else if address < RAM_END {
        NORMAL_NONCACHEABLE
    } else {
        SHARED_DEVICE | EXECUTE_NEVER
    };
    address | attributes | PRIVILEGED_READ_WRITE | SECTION
}
pub(super) const fn control(previous: u32) -> u32 {
    // MMU, extended tables, unaligned accesses and both caches. ARM RAM is
    // write-through; mailbox responses explicitly invalidate cached lines.
    (previous | 1 | (1 << 23) | (1 << 22) | (1 << 12) | (1 << 2))
        & !((1 << 13) | (1 << 28) | (1 << 29))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arm_code_can_use_instruction_cache_without_caching_gpu_ram() {
        for address in [0, 0x0080_0000, ARM_RAM_END - 0x10_0000] {
            let entry = section_descriptor(address);
            assert_eq!(entry & 0xfff0_0000, address);
            assert_eq!(entry & 0x700c, NORMAL_WRITE_THROUGH);
            assert_eq!(entry & (3 << 10), PRIVILEGED_READ_WRITE);
        }
        for address in [ARM_RAM_END, RAM_END - 0x10_0000] {
            assert_eq!(section_descriptor(address) & 0x700c, NORMAL_NONCACHEABLE);
        }
    }
    #[test]
    fn peripherals_remain_uncached_while_arm_data_cache_is_enabled() {
        for address in [RAM_END, 0x2010_0000, 0xfff0_0000] {
            let entry = section_descriptor(address);
            assert_eq!(entry & 0x700c, SHARED_DEVICE);
            assert_ne!(entry & EXECUTE_NEVER, 0);
        }
        for previous in [0, u32::MAX] {
            assert_ne!(control(previous) & (1 << 12), 0);
            assert_ne!(control(previous) & (1 << 2), 0);
            assert_eq!(control(previous) & ((1 << 13) | (1 << 28)), 0);
            assert_ne!(control(previous) & 1, 0);
        }
    }
}
