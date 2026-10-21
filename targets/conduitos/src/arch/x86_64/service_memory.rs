//! Five profile-bound independently owned service address spaces, separate from the
//! ordinary text/timer pool. Every private byte and page-table page is charged.
use super::domain_transition::EntryContext;
use crate::{
    boot,
    protected_region::DomainRefusal,
    service_memory::{MemoryRange, ServiceImage},
};
use core::arch::asm;
#[path = "service_memory/storage.rs"]
mod storage;
use storage::{Lease, View};
const PAGE: usize = 4096;
const NX: u64 = 1 << 63;

pub(super) struct AddressSpace {
    cr3: u64,
    entry: u64,
    cleared: bool,
    lease: Lease,
}
impl AddressSpace {
    /// Compatibility charge for the unchanged compositor profile only.
    pub const RESERVED_BYTES: u32 =
        storage::reserved(crate::service_memory::ServiceProfile::Compositor);
    pub fn reserved_bytes(&self) -> u32 {
        storage::reserved(self.profile())
    }
    pub fn profile(&self) -> crate::service_memory::ServiceProfile {
        self.lease.profile()
    }
    pub fn install(image: &ServiceImage<'_>) -> Result<Self, DomainRefusal> {
        let mut lease = Lease::claim(image.profile())?;
        let (cr3, entry) = install(lease.view(), image)?;
        Ok(Self {
            cr3,
            entry,
            cleared: false,
            lease,
        })
    }
    pub fn exchange(&mut self) -> Result<&mut [u8], DomainRefusal> {
        if self.cleared {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        // SAFETY: the unique AddressSpace owns this slot and mutable window.
        Ok(self.lease.view().exchange)
    }
    /// Exclusive Root staging while the service is stopped. Direction and valid
    /// length belong to the checked request protocol, never to guest pointers.
    pub fn transfer_window(&mut self) -> Result<&mut [u8], DomainRefusal> {
        if self.cleared {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        // SAFETY: this address space is exclusively owned and no service runs
        // concurrently with its Root caller. The mutable borrow prevents entry.
        Ok(self.lease.view().output)
    }
    pub fn output(&self) -> Result<&[u8], DomainRefusal> {
        if self.cleared {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        // SAFETY: no domain runs concurrently with its Root caller.
        Ok(self.lease.output())
    }
    pub fn context(&self) -> Result<EntryContext, DomainRefusal> {
        if self.cleared {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        Ok(EntryContext {
            entry: self.entry,
            cr3: self.cr3,
            frame: self.profile().layout().exchange().start(),
            stack: self.profile().layout().stack().end() - 8,
            // SAFETY: this slot's trap stack is exclusively owned and remains
            // supervisor-only in every user mapping.
            trap_stack: self.lease.trap_top(),
        })
    }
    pub fn quarantine(&mut self) -> u32 {
        if self.cleared {
            return 0;
        }
        // SAFETY: entry returned to Root before quarantine; ownership is held
        // through clearing. No sibling can acquire this slot until Drop.
        self.lease.clear();
        self.cleared = true;
        self.reserved_bytes()
    }
}
impl Drop for AddressSpace {
    fn drop(&mut self) {
        self.quarantine();
    }
}

fn install(mut slot: View<'_>, image: &ServiceImage<'_>) -> Result<(u64, u64), DomainRefusal> {
    let layout = image.profile().layout();
    let expected = [
        layout.image().bytes(),
        layout.exchange().bytes(),
        layout.output().bytes(),
        layout.stack().bytes(),
        layout.metadata().bytes(),
        layout.pixels().map_or(0, |range| range.bytes()),
        layout.scratch().map_or(0, |range| range.bytes()),
    ];
    let actual = [
        slot.image.len(),
        slot.exchange.len(),
        slot.output.len(),
        slot.stack.len(),
        slot.metadata.len(),
        slot.pixels.len(),
        slot.scratch.len(),
    ];
    if image.layout() != layout
        || expected
            .into_iter()
            .zip(actual)
            .any(|(expected, actual)| expected != actual as u64)
        || slot.pt.len() != (layout.end() as usize).div_ceil(2 * 1024 * 1024)
        || slot.trap.len() != 16384
    {
        return Err(DomainRefusal::InvalidMemory);
    }
    slot.clear();
    let hhdm = super::domain_memory::hhdm_offset()?;
    let current_cr3: u64;
    // SAFETY: installation runs only in Root.
    unsafe {
        asm!("mov {}, cr3", out(reg) current_cr3, options(nomem, nostack, preserves_flags));
    }
    let root = hhdm
        .checked_add(current_cr3 & !0xfff)
        .ok_or(DomainRefusal::InvalidMemory)? as *const u64;
    for index in 0..512 {
        // SAFETY: the current CR3 is a live Root page table mapped by the HHDM.
        slot.pml4.0[index] = unsafe { root.add(index).read() } & !4;
    }
    slot.pml4.0[0] = physical(&*slot.pdpt)? | 7;
    slot.pdpt.0[0] = physical(&*slot.pd)? | 7;
    let image_physical = physical(&*slot.image)?;
    for segment in &image.image().segments {
        let offset = usize::try_from(
            segment
                .address
                .checked_sub(layout.image().start())
                .ok_or(DomainRefusal::InvalidMemory)?,
        )
        .map_err(|_| DomainRefusal::InvalidMemory)?;
        let end = offset
            .checked_add(segment.bytes.len())
            .ok_or(DomainRefusal::InvalidMemory)?;
        slot.image
            .get_mut(offset..end)
            .ok_or(DomainRefusal::InvalidMemory)?
            .copy_from_slice(segment.bytes);
        map(
            &mut slot,
            segment.address,
            segment.mapped_bytes(),
            image_physical + offset as u64,
            5 | if segment.executable { 0 } else { NX },
        )?;
    }
    if let Some(data) = image.private_data() {
        // The ELF template initializes private metadata, never executable image
        // storage. The complete slot was zeroed above, including the BSS tail.
        if !matches!(
            image.profile(),
            crate::service_memory::ServiceProfile::Draft
                | crate::service_memory::ServiceProfile::Home
                | crate::service_memory::ServiceProfile::Runtime
                | crate::service_memory::ServiceProfile::Application
        ) || data.address != layout.metadata().start()
            || data.memory_bytes > PAGE as u64
            || data.bytes.len() as u64 > data.memory_bytes
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        let target = slot
            .metadata
            .get_mut(..data.memory_bytes as usize)
            .ok_or(DomainRefusal::InvalidMemory)?;
        target[..data.bytes.len()].copy_from_slice(data.bytes);
        target[data.bytes.len()..].fill(0);
    }
    let private = [
        (Some(layout.exchange()), physical(&*slot.exchange)?),
        (Some(layout.output()), physical(&*slot.output)?),
        (Some(layout.stack()), physical(&*slot.stack)?),
        (Some(layout.metadata()), physical(&*slot.metadata)?),
        (layout.pixels(), optional_physical(slot.pixels)?),
        (layout.scratch(), optional_physical(slot.scratch)?),
    ];
    for (range, base) in private {
        if let Some(range) = range {
            map_private(&mut slot, range, base)?;
        }
    }
    Ok((physical(&*slot.pml4)?, image.image().entry))
}
fn optional_physical(bytes: &[u8]) -> Result<u64, DomainRefusal> {
    if bytes.is_empty() {
        Ok(0)
    } else {
        physical(bytes)
    }
}

fn map_private(slot: &mut View<'_>, range: MemoryRange, base: u64) -> Result<(), DomainRefusal> {
    map(slot, range.start(), range.bytes(), base, 7 | NX)
}
fn map(
    slot: &mut View<'_>,
    start: u64,
    bytes: u64,
    base: u64,
    flags: u64,
) -> Result<(), DomainRefusal> {
    if !start.is_multiple_of(PAGE as u64)
        || !base.is_multiple_of(PAGE as u64)
        || !bytes.is_multiple_of(PAGE as u64)
    {
        return Err(DomainRefusal::InvalidMemory);
    }
    for offset in (0..bytes).step_by(PAGE) {
        let virtual_address = start
            .checked_add(offset)
            .ok_or(DomainRefusal::InvalidMemory)?;
        let directory =
            usize::try_from(virtual_address >> 21).map_err(|_| DomainRefusal::InvalidMemory)?;
        let index = (virtual_address as usize >> 12) & 511;
        if directory >= slot.pt.len() || slot.pt[directory].0[index] != 0 {
            return Err(DomainRefusal::InvalidMemory);
        }
        let table = physical(&slot.pt[directory])?;
        slot.pd.0[directory] = table | 7;
        slot.pt[directory].0[index] = base
            .checked_add(offset)
            .ok_or(DomainRefusal::InvalidMemory)?
            | flags;
    }
    Ok(())
}
fn physical<T: ?Sized>(value: &T) -> Result<u64, DomainRefusal> {
    boot::executable_physical_address(value as *const T as *const () as u64)
        .filter(|address| address & 0xfff == 0)
        .ok_or(DomainRefusal::InvalidMemory)
}

#[cfg(test)]
#[path = "service_memory/tests.rs"]
mod tests;
