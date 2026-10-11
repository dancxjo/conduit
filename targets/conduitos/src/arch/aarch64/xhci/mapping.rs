//! Root-only finite Device-nGnRnE mappings in the installed low 4 KiB regime.
use super::super::{LOW_L0, LOW_L2, current_el};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappingRefusal {
    Regime,
    Address,
    Occupied,
}
impl MappingRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regime => "aarch64-virt-xhci-mapping-regime-unsupported",
            Self::Address => "aarch64-virt-xhci-mapping-address-invalid",
            Self::Occupied => "aarch64-virt-xhci-mapping-occupied",
        }
    }
}

pub(super) fn map(
    physical: u64,
    image_mapping: fn(u64) -> Option<u64>,
) -> Result<usize, MappingRefusal> {
    let (control, tcr, ttbr0, mair): (u64, u64, u64, u64);
    unsafe {
        core::arch::asm!(
            "mrs {0}, sctlr_el1", "mrs {1}, tcr_el1",
            "mrs {2}, ttbr0_el1", "mrs {3}, mair_el1",
            out(reg) control, out(reg) tcr, out(reg) ttbr0, out(reg) mair,
            options(nostack, preserves_flags)
        );
    }
    let root = image_mapping(LOW_L0.0.get() as u64).ok_or(MappingRefusal::Regime)?;
    if current_el() != 1
        || control & 1 == 0
        || tcr & 0x3f != 16
        || (tcr >> 14) & 3 != 0
        || tcr & (1 << 7) != 0
        || ttbr0 & 0x0000_ffff_ffff_f000 != root
        || mair >> 56 != 0
    {
        return Err(MappingRefusal::Regime);
    }
    // The existing three-level low Root tables cover exactly the first GiB.
    if physical >= 1 << 30 {
        return Err(MappingRefusal::Address);
    }
    let block = physical & !((1 << 21) - 1);
    let descriptor = block | 1 | (1 << 10) | (7 << 2) | (1 << 53) | (1 << 54);
    let slot = (physical >> 21) as usize;
    unsafe {
        let entry = core::ptr::addr_of_mut!((*LOW_L2.0.get())[slot]);
        let prior = entry.read_volatile();
        if prior != 0 && prior != descriptor {
            return Err(MappingRefusal::Occupied);
        }
        entry.write_volatile(descriptor);
        core::arch::asm!("dsb sy", "tlbi vmalle1", "dsb sy", "isb", options(nostack));
    }
    Ok(physical as usize)
}
