//! Sealed borrowed matching workspace; the default retains no index.
pub use super::matcher::Index as MatchIndex;
mod sealed {
    pub trait Sealed {}
    impl Sealed for () {}
    impl Sealed for &mut super::MatchIndex {}
}
/// Sealed finite matching storage: the default is zero-sized; indexed callers
/// explicitly lend their admitted workspace for the encoder's whole lifetime.
pub trait MatchWorkspace: sealed::Sealed {
    fn index(&self) -> Option<&MatchIndex>;
    fn observe(&mut self, position: usize, byte: u8);
    fn clear(&mut self);
}
impl MatchWorkspace for () {
    fn index(&self) -> Option<&MatchIndex> {
        None
    }
    fn observe(&mut self, _: usize, _: u8) {}
    fn clear(&mut self) {}
}
impl MatchWorkspace for &mut MatchIndex {
    fn index(&self) -> Option<&MatchIndex> {
        Some(self)
    }
    fn observe(&mut self, position: usize, byte: u8) {
        MatchIndex::observe(self, position, byte);
    }
    fn clear(&mut self) {
        MatchIndex::clear(self);
    }
}
