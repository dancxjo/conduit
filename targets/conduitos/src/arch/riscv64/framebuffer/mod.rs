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
            .ok_or("riscv64-framebuffer-outside-hhdm")?;
        let bytes = raw.format().byte_len().map_err(|error| error.as_str())?;
        if bytes > extent || !boot::framebuffer_reserved_range(physical_start, bytes as u64) {
            return Err("riscv64-framebuffer-not-reserved");
        }
        let satp: u64;
        unsafe {
            core::arch::asm!("csrr {0}, satp", out(reg) satp, options(nostack));
        }
        let mut observed = None;
        let mapping = mapping::validate(
            virtual_start,
            physical_start,
            bytes as u64,
            satp,
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
        let after: u64;
        unsafe {
            core::arch::asm!("csrr {0}, satp", out(reg) after, options(nostack));
        }
        if satp != after {
            return Err("riscv64-framebuffer-translation-changed");
        }
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("riscv64-framebuffer-already-owned");
        }
        Ok(Self { raw, mapping })
    }
    pub fn mapping(&self) -> &MappingEvidence {
        &self.mapping
    }
    fn complete(&self) {
        // This profile is QEMU TCG with ramfb mapping the same guest RAM, not
        // physical cache-coherence proof. Its pinned implementation models no
        // cache hierarchy and translates FENCE into a full memory barrier.
        unsafe {
            core::arch::asm!("fence iorw, iorw", options(nostack));
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
