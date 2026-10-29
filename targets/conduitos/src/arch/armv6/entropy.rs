//! BCM2835 physical hardware RNG mapping for the freestanding ARMv6 Host.
//!
//! Linux bcm2835-common.dtsi locates RNG at bus 0x7e104000; bcm2835.dtsi
//! maps bus 0x7e000000 to CPU physical 0x20000000, yielding 0x20104000.
//! Register-level conformance does not establish physical entropy quality.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::cryptographic_entropy::{
    EntropyProvider, EntropyRefusal,
    bcm2835::{Generator, Registers},
};

const RNG_BASE: usize = 0x2010_4000;
const REQUEST_POLLS: u32 = 1_000_000;
static CLAIMED: AtomicBool = AtomicBool::new(false);

pub struct NativeRngRegisters {
    _exclusive: (),
}

pub type Bcm2835Entropy = Generator<NativeRngRegisters>;

/// Admit the actual BCM2835 hardware source with no deterministic fallback.
///
/// # Safety
/// The caller must establish a BCM2835 Host with the peripheral window mapped
/// as ordered device memory (or MMU/caches disabled), and exclusive ownership
/// against firmware and all other controllers/interrupts. The local lease
/// prevents duplicate admission through this mapping, not foreign access.
/// Hardware entropy quality is a platform assumption, not tested here.
pub unsafe fn detect(provider_generation: u64) -> Result<Bcm2835Entropy, EntropyRefusal> {
    if provider_generation == 0 {
        return Err(EntropyRefusal::InvalidProvider);
    }
    CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| EntropyRefusal::Unavailable)?;
    Generator::admit(
        NativeRngRegisters { _exclusive: () },
        EntropyProvider {
            base_id: "conduitos/base/cryptographic-entropy/bcm2835-rng",
            provider_instance_id: "conduitos/armv6/bcm2835/rng-0",
            provider_generation,
        },
        REQUEST_POLLS,
    )
}

unsafe impl Registers for NativeRngRegisters {
    fn read(&mut self, offset: usize) -> u32 {
        debug_assert!(matches!(offset, 0 | 4 | 8));
        barrier();
        let value = unsafe { core::ptr::read_volatile((RNG_BASE + offset) as *const u32) };
        barrier();
        value
    }

    fn write(&mut self, offset: usize, value: u32) {
        debug_assert!(matches!(offset, 0 | 4));
        barrier();
        unsafe { core::ptr::write_volatile((RNG_BASE + offset) as *mut u32, value) };
        barrier();
    }
}

impl Drop for NativeRngRegisters {
    fn drop(&mut self) {
        CLAIMED.store(false, Ordering::Release);
    }
}

fn barrier() {
    // Drain the ARM1176 write buffer across the peripheral access boundary.
    unsafe {
        core::arch::asm!(
            "mcr p15, 0, {zero}, c7, c10, 4",
            zero = in(reg) 0u32,
            options(nostack, preserves_flags),
        );
    }
}
