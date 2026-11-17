use super::*;
use crate::preparation_backing::{Backing, sum};
impl Backing for NativeLawWitness {
    fn backing(&self) -> Option<usize> {
        self.preparation_backing_at(0)
    }
}
impl NativeLawWitness {
    pub(super) fn preparation_backing_at(&self, depth: usize) -> Option<usize> {
        if depth > MAXIMUM_DEPTH {
            return None;
        }
        // Arc/Rc control block and whole immutable module are charged per alias.
        sum([
            Some(core::mem::size_of::<Module>() + 2 * core::mem::size_of::<usize>()),
            self.module.source.backing(),
            self.checked.backing(),
            imports(&self.module.imports, depth),
            families(&self.module.families, depth),
        ])
    }
}
pub(super) fn imports(values: &Vec<(String, NativeLawWitness)>, depth: usize) -> Option<usize> {
    sum(core::iter::once(
        values
            .capacity()
            .checked_mul(core::mem::size_of::<(String, NativeLawWitness)>()),
    )
    .chain(
        values
            .iter()
            .map(|(name, value)| sum([name.backing(), value.preparation_backing_at(depth + 1)])),
    ))
}
pub(super) fn families(values: &Vec<(String, NativeFamilyWitness)>, depth: usize) -> Option<usize> {
    sum(core::iter::once(
        values
            .capacity()
            .checked_mul(core::mem::size_of::<(String, NativeFamilyWitness)>()),
    )
    .chain(
        values
            .iter()
            .map(|(name, value)| sum([name.backing(), value.preparation_backing_at(depth + 1)])),
    ))
}
