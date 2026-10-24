//! Exclusive firmware scanout under the normal IA-32 flat protected-mode Root.
use crate::{
    boot,
    display::{DisplayError, DisplayFormat, PixelTarget, RawDisplay},
};
use core::{
    ops::Range,
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};
mod policy;
pub use policy::ScanoutPolicy;
static CLAIMED: AtomicBool = AtomicBool::new(false);

pub struct FirmwareScanout {
    raw: RawDisplay,
    policy: ScanoutPolicy,
    memory: boot::ScanoutMemoryEvidence,
}
impl FirmwareScanout {
    /// # Safety
    /// Invoke after `arch::initialize_machine` with the original identity-mapped
    /// Multiboot handoff. `owned` must include all Root image/arena and module
    /// storage. No other writer may use the firmware framebuffer while held.
    pub unsafe fn prepare(
        magic: u32,
        information: u32,
        owned: &[Range<u64>],
    ) -> Result<Self, &'static str> {
        let facts = unsafe { boot::framebuffer_from_multiboot1(magic, information) }
            .map_err(|error| error.as_str())?
            .ok_or("display-absent")?;
        let memory =
            unsafe { boot::validate_multiboot1_scanout_memory(magic, information, &facts, owned) }?;
        let cr0: u32;
        let (cs, ds, es, ss): (u16, u16, u16, u16);
        unsafe {
            core::arch::asm!("mov {0:e}, cr0",out(reg)cr0,options(nomem,nostack,preserves_flags));
            core::arch::asm!("mov {0:x}, cs","mov {1:x}, ds","mov {2:x}, es","mov {3:x}, ss",
                out(reg)cs,out(reg)ds,out(reg)es,out(reg)ss,options(nomem,nostack,preserves_flags));
        }
        let cpu = core::arch::x86::__cpuid(1);
        let policy = ScanoutPolicy::prepare(
            facts.physical_start,
            facts.byte_len,
            cr0,
            [cs, ds, es, ss],
            cpu.edx,
            cpu.ebx,
        )?;
        CLAIMED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "ia32-scanout-already-owned")?;
        // All fallible geometry and CPU checks precede custody. On a constructor
        // refusal, release only this claim; successful Root retains the value.
        let raw = unsafe {
            RawDisplay::new_word_writable(
                NonNull::new(facts.physical_start as usize as *mut u8).unwrap(),
                facts.byte_len as usize,
                facts.format,
            )
        };
        match raw {
            Ok(raw) => Ok(Self {
                raw,
                policy,
                memory,
            }),
            Err(error) => {
                CLAIMED.store(false, Ordering::Release);
                Err(error.as_str())
            }
        }
    }
    pub fn policy(&self) -> &ScanoutPolicy {
        &self.policy
    }
    pub fn memory(&self) -> &boot::ScanoutMemoryEvidence {
        &self.memory
    }
    fn complete(&self, lines: Option<Range<u64>>) {
        let Some(lines) = lines else {
            return;
        };
        // Intel SDM Vol2A CLFLUSH: own CPUID feature and line-size fields,
        // cache writeback independent of memory type; MFENCE orders stores.
        // https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2a-manual.pdf
        unsafe {
            core::arch::asm!("mfence", options(nostack, preserves_flags));
        }
        for address in lines.step_by(usize::from(self.policy.cache_line_bytes)) {
            unsafe {
                core::arch::asm!("clflush [{0}]",in(reg)address as usize,options(nostack,preserves_flags));
            }
        }
        unsafe {
            core::arch::asm!("mfence", options(nostack, preserves_flags));
        }
    }
}
impl Drop for FirmwareScanout {
    fn drop(&mut self) {
        CLAIMED.store(false, Ordering::Release);
    }
}
impl PixelTarget for FirmwareScanout {
    fn format(&self) -> DisplayFormat {
        self.raw.format()
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        let lines = self.policy.lines(self.raw.format(), x, y, 1)?;
        self.raw.write_pixel(x, y, pixel)?;
        self.complete(lines);
        Ok(())
    }
    fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
        let lines = self.policy.lines(self.raw.format(), x, y, pixels.len())?;
        self.raw.write_span(x, y, pixels)?;
        self.complete(lines);
        Ok(())
    }
}
