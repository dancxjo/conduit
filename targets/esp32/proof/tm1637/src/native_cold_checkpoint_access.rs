//! Real partition-limited operations while the actual Root is exclusively cold.
//! This supplies neither a timed sampler window nor an ordinary profile.
use super::*;
use crate::native_root::cold_membership::ColdOwnership;
pub(crate) struct Access<'scope, 'root> {
    _scope: &'scope ColdOwnership<'root>,
    _timing: measurement::ReviewedTiming,
}
impl<'scope, 'root> Access<'scope, 'root> {
    pub(crate) fn new(
        scope: &'scope ColdOwnership<'root>,
        timing: measurement::ReviewedTiming,
    ) -> Self {
        Self {
            _scope: scope,
            _timing: timing,
        }
    }
}
impl SectorStorage<BodyRegion> for Access<'_, '_> {
    fn erase_sector(&mut self, storage: &mut BodyRegion, bank: u8, sector: u8) -> Result<(), E> {
        erase_body_sector(storage, bank, sector)
    }
    fn program(&mut self, storage: &mut BodyRegion, at: u32, bytes: &[u8]) -> Result<(), E> {
        program_body(storage, at, bytes)
    }
    fn read(&mut self, storage: &mut BodyRegion, at: u32, out: &mut [u8]) -> Result<(), E> {
        read_body(storage, at, out)
    }
}
