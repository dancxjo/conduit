//! Custody and ordered volatile writes for one observed firmware RAM scanout.
use crate::{
    boot,
    display::{DisplayError, DisplayFormat, PixelTarget, RawDisplay},
};
mod mapping;
pub use mapping::MappingEvidence;

pub struct FirmwareScanout {
    raw: RawDisplay,
    mapping: MappingEvidence,
}
impl FirmwareScanout {
    pub fn prepare(raw: RawDisplay, record: &boot::BootRecord) -> Result<Self, &'static str> {
        let (virtual_start, extent) = raw.memory_range();
        let physical_start = virtual_start
            .checked_sub(record.hhdm_offset)
            .ok_or("aarch64-framebuffer-outside-hhdm")?;
        let bytes = raw.format().byte_len().map_err(|e| e.as_str())?;
        if bytes > extent || !boot::framebuffer_reserved_range(physical_start, bytes as u64) {
            return Err("aarch64-framebuffer-not-reserved");
        }
        let (control, tcr): (u64, u64);
        unsafe {
            core::arch::asm!("mrs {0}, sctlr_el1", "mrs {1}, tcr_el1", out(reg) control,out(reg) tcr,options(nostack,preserves_flags));
        }
        if super::current_el() != 1
            || control & 1 == 0
            || (tcr >> 30) & 3 != 2
            || (tcr >> 32) & 7 > 5
        {
            return Err("aarch64-framebuffer-translation-regime-unsupported");
        }
        let mut observed = None;
        let mapping = mapping::validate(virtual_start,physical_start,bytes as u64,|address| {
            let par: u64;
            unsafe { core::arch::asm!("at s1e1w, {address}","isb","mrs {par}, par_el1",address=in(reg) address,par=out(reg) par,options(nostack)); }
            observed = Some((address, par));
            par
        }).map_err(|error| {
            if let Some((address, par)) = observed {
                super::present(alloc::format!(
                    "CONDUIT_FRAMEBUFFER_TRANSLATION {{\"virtual_address\":{address},\"par_el1\":{par},\"par_memory_attribute\":{}}}\n",
                    par >> 56).as_bytes());
            }
            error.as_str()
        })?;
        Ok(Self { raw, mapping })
    }
    pub fn mapping(&self) -> &MappingEvidence {
        &self.mapping
    }
    fn complete_stores(&self) {
        // Normal Non-cacheable scanout needs no data-cache clean. Completion
        // precedes the renderer Host Call return and subsequent Show ack.
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
}
impl PixelTarget for FirmwareScanout {
    fn format(&self) -> DisplayFormat {
        self.raw.format()
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        self.raw.write_pixel(x, y, pixel)?;
        self.complete_stores();
        Ok(())
    }
    fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
        self.raw.write_span(x, y, pixels)?;
        self.complete_stores();
        Ok(())
    }
}
