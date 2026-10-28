//! Body checkpoint operations scoped to the sampler's actual flash permit.
//! No raw sector erase is added to BodyRegion's ordinary interface.
use super::*;
use crate::native_body_checkpoint::{
    incremental::{Operation, SectorStorage},
    Refusal as E,
};
use crate::native_flash_window::FlashPermit;

pub(crate) struct Access<'permit, 'wait, 'gate> {
    _permit: &'permit FlashPermit<'wait, 'gate>,
}
impl<'permit, 'wait, 'gate> Access<'permit, 'wait, 'gate> {
    pub(crate) fn new(permit: &'permit FlashPermit<'wait, 'gate>) -> Self {
        Self { _permit: permit }
    }
}
/// Selected only with the actual ROM-reviewed timing witness. A program step
/// writes <=128 bytes through at most two64-byte writes, with a page
/// boundary requiring at most three page programs. The existing1000ms
/// envelope covers status/unlock, nine programs and an erase, so conservatively
/// bounds this smaller program operation without increasing the admitted cap.
pub(crate) fn budget_ms(_reviewed: measurement::ReviewedTiming, operation: Operation) -> u32 {
    match operation {
        Operation::EraseSector => 800,
        Operation::ProgramChunk => 1000,
        Operation::ReadbackChunk => 100,
    }
}
impl SectorStorage<BodyRegion> for Access<'_, '_, '_> {
    fn erase_sector(&mut self, storage: &mut BodyRegion, bank: u8, sector: u8) -> Result<(), E> {
        if bank > 1 || sector > 7 {
            return Err(E::Geometry);
        }
        let relative = u32::from(bank) * 0x8000 + u32::from(sector) * 4096;
        let start = storage.0.bound(relative, 4096).map_err(|_| E::Geometry)?;
        storage.0.base.lock(|base| {
            NorFlash::erase(&mut *base.borrow_mut(), start, start + 4096).map_err(|_| E::Storage)
        })
    }
    fn program(&mut self, storage: &mut BodyRegion, at: u32, bytes: &[u8]) -> Result<(), E> {
        if bytes.is_empty() || bytes.len() > 128 {
            return Err(E::Geometry);
        }
        storage.program(at, bytes).map_err(|_| E::Storage)
    }
    fn read(&mut self, storage: &mut BodyRegion, at: u32, out: &mut [u8]) -> Result<(), E> {
        if out.is_empty() || out.len() > 160 {
            return Err(E::Geometry);
        }
        storage.read(at, out).map_err(|_| E::Storage)
    }
}

impl crate::native_restart::network_pending_job::live::Access for Access<'_, '_, '_> {
    fn erase_network(
        &mut self,
        owner: &mut crate::native_play::journal::Inspection,
        authority: &crate::native_restart::network_recovery::RecoveryPermit,
        sector: u8,
    ) -> Result<(), &'static str> {
        owner.erase_closed_sector(authority, self._permit, sector)
    }
    fn read_network(
        &mut self,
        owner: &mut crate::native_play::journal::Inspection,
        at: u32,
        out: &mut [u8],
    ) -> Result<(), &'static str> {
        if out.is_empty() || out.len() > 512 {
            return Err("network-live-read-geometry");
        }
        owner.read_chunk(at, out)
    }
}
