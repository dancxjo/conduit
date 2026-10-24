//! One reserved firmware scanout for the reviewed QEMU TCG ramfb profile.
use crate::{
    boot,
    display::{DisplayError, DisplayFormat, PixelTarget, RawDisplay},
};
use core::sync::atomic::{AtomicBool, Ordering};
mod mapping;
pub use mapping::MappingEvidence;
static OWNED: AtomicBool = AtomicBool::new(false);
pub struct FirmwareScanout {
    raw: RawDisplay,
    mapping: MappingEvidence,
}
impl FirmwareScanout {
    pub fn prepare(raw: RawDisplay, record: &boot::BootRecord) -> Result<Self, &'static str> {
        let (virtual_start, extent) = raw.memory_range();
        let physical_start = virtual_start
            .checked_sub(record.hhdm_offset)
            .ok_or("loongarch64-framebuffer-outside-hhdm")?;
        let bytes = raw.format().byte_len().map_err(|error| error.as_str())?;
        if bytes > extent || !boot::framebuffer_reserved_range(physical_start, bytes as u64) {
            return Err("loongarch64-framebuffer-not-reserved");
        }
        let regime = read_regime();
        let mut observed = None;
        let mapping = mapping::validate(
            virtual_start,
            physical_start,
            bytes as u64,
            regime,
            |address| {
                observed = Some((address, None));
                if !boot::bootloader_table_page(address & !4095) {
                    return Err(mapping::Refusal::Table);
                }
                let virtual_address = record
                    .hhdm_offset
                    .checked_add(address)
                    .ok_or(mapping::Refusal::Table)?;
                // Limine's retained HHDM covers bootloader-reclaimable RAM. Only
                // complete reserved table pages are readable; never device memory,
                // runtime arena, arbitrary RAM or a descriptor-provided VA.
                let entry = unsafe { (virtual_address as *const u64).read_volatile() };
                observed = Some((address, Some(entry)));
                Ok(entry)
            },
        )
        .map_err(|error| {
            if let Some((address, entry)) = observed {
                let entry = entry.map_or_else(|| alloc::string::String::from("null"), |value| alloc::format!("{value}"));
                super::present(alloc::format!(
                    "CONDUIT_FRAMEBUFFER_TRANSLATION {{\"descriptor_physical\":{address},\"pte\":{entry}}}\n"
                ).as_bytes());
            }
            error.as_str()
        })?;
        if regime != read_regime() {
            return Err("loongarch64-framebuffer-translation-changed");
        }
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("loongarch64-framebuffer-already-owned");
        }
        Ok(Self { raw, mapping })
    }
    pub fn mapping(&self) -> &MappingEvidence {
        &self.mapping
    }
    fn complete(&self) {
        // This profile is QEMU TCG with ramfb mapping the same guest RAM, not
        // physical cache-coherence proof. Its pinned implementation models no
        // cache hierarchy and translates DBAR into a full memory barrier.
        unsafe {
            core::arch::asm!("dbar 0", options(nostack));
        }
    }
}
impl Drop for FirmwareScanout {
    fn drop(&mut self) {
        self.complete();
        OWNED.store(false, Ordering::Release);
    }
}
impl PixelTarget for FirmwareScanout {
    fn format(&self) -> DisplayFormat {
        self.raw.format()
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        self.raw.write_pixel(x, y, pixel)?;
        self.complete();
        Ok(())
    }
    fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
        self.raw.write_span(x, y, pixels)?;
        self.complete();
        Ok(())
    }
}

/// Observe the actual inherited paging regime; interrupt-enable changes alone
/// do not change translation. No CSR is written and no page table is modified.
fn read_regime() -> mapping::Regime {
    let cpu: usize;
    unsafe {
        core::arch::asm!("cpucfg {0}, {1}", out(reg) cpu, in(reg) 1usize, options(nostack));
    }
    mapping::Regime {
        crmd: super::read_csr::<0>() as u64 & !4,
        pwcl: super::read_csr::<0x1c>() as u64,
        pwch: super::read_csr::<0x1d>() as u64,
        stlbps: super::read_csr::<0x1e>() as u64,
        pgdh: super::read_csr::<0x1a>() as u64,
        direct_windows: [
            super::read_csr::<0x180>() as u64,
            super::read_csr::<0x181>() as u64,
            super::read_csr::<0x182>() as u64,
            super::read_csr::<0x183>() as u64,
        ],
        physical_bits: (((cpu >> 4) & 0xff) + 1) as u8,
        virtual_bits: (((cpu >> 12) & 0xff) + 1) as u8,
        reduced_address_config: super::read_csr::<0x1f>() as u64,
    }
}
