use super::*;
use crate::preparation_backing::{sum, Backing};
impl NativeFamilyWitness {
    pub(crate) fn preparation_backing_at(&self, depth: usize) -> Option<usize> {
        if depth > MAXIMUM_DEPTH {
            return None;
        }
        sum([
            Some(core::mem::size_of::<BundleSources>() + 2 * core::mem::size_of::<usize>()),
            self.bundle.sources.manifest.backing(),
            self.bundle.sources.members.backing(),
            self.anchor.exported_path.backing(),
            super::super::preparation_backing::imports(&self.imports, depth),
            super::super::preparation_backing::families(&self.families, depth),
        ])
    }
}
impl Backing for NativeFamilyMember {
    fn backing(&self) -> Option<usize> {
        sum([self.path.backing(), self.source.backing()])
    }
}
