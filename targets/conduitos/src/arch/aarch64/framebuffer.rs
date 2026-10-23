//! Custody and ordered volatile writes for one observed firmware RAM scanout.
use crate::{
    boot,
    display::{DisplayError, DisplayFormat, PixelTarget, RawDisplay},
};
mod cache;
mod mapping;
pub use cache::CachePolicy;
pub use mapping::MappingEvidence;

pub struct FirmwareScanout {
    raw: RawDisplay,
    mapping: MappingEvidence,
    cache: CachePolicy,
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
        let ctr: u64;
        unsafe {
            core::arch::asm!("mrs {0}, ctr_el0",out(reg) ctr,options(nostack));
        }
        let cache = CachePolicy::prepare(
            mapping.virtual_start,
            mapping.byte_length,
            mapping.par_memory_attribute,
            ctr,
        )?;
        Ok(Self {
            raw,
            mapping,
            cache,
        })
    }
    pub fn mapping(&self) -> &MappingEvidence {
        &self.mapping
    }
    pub fn cache_policy(&self) -> &CachePolicy {
        &self.cache
    }
    fn store_lines(
        &self,
        x: u32,
        y: u32,
        pixels: usize,
    ) -> Result<Option<core::ops::Range<u64>>, DisplayError> {
        let format = self.raw.format();
        let offset = (y as usize)
            .checked_mul(format.pitch as usize)
            .and_then(|row| {
                (x as usize)
                    .checked_mul(4)
                    .and_then(|col| row.checked_add(col))
            })
            .ok_or(DisplayError::InvalidExtent)?;
        let bytes = pixels.checked_mul(4).ok_or(DisplayError::InvalidExtent)?;
        self.cache.lines(offset, bytes)
    }
    fn complete_stores(&self, lines: Option<core::ops::Range<u64>>) {
        if let Some(lines) = lines {
            for address in lines.step_by(usize::from(self.cache.cache_line_bytes)) {
                unsafe {
                    core::arch::asm!("dc cvac, {0}",in(reg) address,options(nostack,preserves_flags));
                }
            }
        }
        // Both cached and NC stores complete before Host Call return/Show ack.
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
        let lines = self.store_lines(x, y, 1)?;
        self.raw.write_pixel(x, y, pixel)?;
        self.complete_stores(lines);
        Ok(())
    }
    fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
        let lines = self.store_lines(x, y, pixels.len())?;
        self.raw.write_span(x, y, pixels)?;
        self.complete_stores(lines);
        Ok(())
    }
}
