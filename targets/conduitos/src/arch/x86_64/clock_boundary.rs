//! Exact-image, one-entry proof clock custody. Never a production guest offer.
use super::{ServiceDomain, domain_transition::EntryContext};
use crate::{protected_region::DomainRefusal, service_memory::ServiceImage};
use sha2::{Digest, Sha256};
#[allow(dead_code)]
#[path = "../../../domain/frame.rs"]
mod frame;
mod policy;
const IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/domain.elf"));
unsafe extern "C" {
    static mut conduitos_boundary_clock: [u64; 5];
}

pub(super) struct Binding {
    digest: [u8; 32],
    state: policy::State,
}
pub(super) struct Permit {
    instance: u64,
    row: u8,
    digest: [u8; 32],
    entry: u64,
    cr3: u64,
}
pub(super) struct Active {
    permit: Permit,
    root_cr4: u64,
}
#[derive(Clone, Copy)]
pub(crate) struct Observation {
    pub instance: u64,
    pub row: u8,
    pub digest: [u8; 32],
    pub stamps: [u64; 4],
    pub completed: bool,
    pub root_cr4_preserved: bool,
    pub user_interrupts: (u32, u32),
}
impl Binding {
    fn prepared() -> Result<Self, DomainRefusal> {
        let cpu = unsafe { core::arch::x86_64::__cpuid(1) };
        if cpu.edx & (1 << 4) == 0 || cpu.edx & (1 << 26) == 0 {
            return Err(DomainRefusal::Unsupported);
        }
        Ok(Self {
            digest: Sha256::digest(IMAGE).into(),
            state: policy::State::default(),
        })
    }
    pub(super) fn arm(&mut self, row: usize, bytes: &[u8]) -> Result<(), DomainRefusal> {
        let read = |offset| {
            bytes
                .get(offset..offset + 4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_le_bytes)
        };
        let length = [0, 8, frame::TEXT_CAPACITY]
            .get(row / 6)
            .copied()
            .ok_or(DomainRefusal::InvalidLifecycle)?;
        if read(core::mem::offset_of!(frame::TextFrame, probe)) != Some(22)
            || read(core::mem::offset_of!(frame::TextFrame, command)) != Some(0)
            || read(core::mem::offset_of!(frame::TextFrame, capacity))
                != Some(frame::TEXT_CAPACITY as u32)
            || read(core::mem::offset_of!(frame::TextFrame, input_length)) != Some(length as u32)
            || !bytes
                .get(
                    core::mem::offset_of!(frame::TextFrame, input)
                        ..core::mem::offset_of!(frame::TextFrame, input) + length,
                )
                .is_some_and(|b| b.iter().all(|v| *v == b'a'))
        {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        self.state
            .arm(row)
            .map_err(|_| DomainRefusal::InvalidLifecycle)
    }
    pub(super) fn take(&mut self, instance: u64, context: &EntryContext) -> Option<Permit> {
        Some(Permit {
            row: self.state.take()?,
            instance,
            digest: self.digest,
            entry: context.entry,
            cr3: context.cr3,
        })
    }
}
impl ServiceDomain {
    pub(crate) fn proof_root_cr4() -> u64 {
        cr4()
    }
    /// No caller-supplied image or digest can confer clock authority. Generic
    /// install remains ineligible even for these same compiled proof bytes.
    pub(crate) fn install_clock_fixture() -> Result<Self, DomainRefusal> {
        let image = ServiceImage::compositor(IMAGE).map_err(|_| DomainRefusal::InvalidMemory)?;
        let binding = Binding::prepared()?;
        let mut domain = Self::install(&image)?;
        domain.clock_binding = Some(binding);
        Ok(domain)
    }
    pub(crate) fn arm_clock_fixture(&mut self, row: usize) -> Result<(), DomainRefusal> {
        if self.quarantined {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        let binding = self
            .clock_binding
            .as_mut()
            .ok_or(DomainRefusal::InvalidLifecycle)?;
        binding.arm(row, self.space.exchange()?)
    }
}
fn cr4() -> u64 {
    let value;
    unsafe {
        core::arch::asm!("mov {}, cr4",out(reg)value,options(nomem,nostack,preserves_flags));
    }
    value
}
pub(super) fn begin(
    permit: Option<Permit>,
    context: &EntryContext,
) -> Result<Option<Active>, DomainRefusal> {
    let pointer = core::ptr::addr_of_mut!(conduitos_boundary_clock).cast::<u64>();
    for index in 0..5 {
        unsafe {
            pointer.add(index).write_volatile(0);
        }
    }
    let Some(permit) = permit else {
        return Ok(None);
    };
    if permit.entry != context.entry || permit.cr3 != context.cr3 || permit.instance == 0 {
        return Err(DomainRefusal::InvalidLifecycle);
    }
    let active = Active {
        permit,
        root_cr4: cr4(),
    };
    unsafe {
        pointer.write_volatile(1);
    }
    Ok(Some(active))
}
pub(super) fn finish(active: Option<Active>) -> Option<Observation> {
    let active = active?;
    let pointer = core::ptr::addr_of_mut!(conduitos_boundary_clock).cast::<u64>();
    let raw: [u64; 5] = core::array::from_fn(|index| unsafe { pointer.add(index).read_volatile() });
    unsafe {
        pointer.write_volatile(0);
    }
    Some(Observation {
        instance: active.permit.instance,
        row: active.permit.row,
        digest: active.permit.digest,
        stamps: [raw[1], raw[2], raw[3], raw[4]],
        completed: raw[0] == 2,
        root_cr4_preserved: cr4() == active.root_cr4,
        user_interrupts: super::domain_budget::user_interrupts(),
    })
}
