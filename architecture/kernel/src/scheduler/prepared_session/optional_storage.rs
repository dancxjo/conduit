use super::OptionalOwnedOutputs;
use alloc::boxed::Box;

/// Affine finite optional-output tracking allocated before an exact Plan binds.
/// This is storage custody, not an endpoint declaration or execution authority.
pub struct OptionalOutputStorage<const C: usize> {
    pub(super) inner: Box<OptionalOwnedOutputs<C>>,
}
impl<const C: usize> Default for OptionalOutputStorage<C> {
    fn default() -> Self {
        Self::prepare()
    }
}
impl<const C: usize> OptionalOutputStorage<C> {
    pub fn prepare() -> Self {
        Self {
            inner: Box::new(OptionalOwnedOutputs {
                starts: [0; C],
                optional: [false; C],
            }),
        }
    }
    pub const fn required_bytes() -> usize {
        core::mem::size_of::<OptionalOwnedOutputs<C>>()
    }
    #[cfg(test)]
    pub(super) fn backing_identity(&self) -> usize {
        (&*self.inner as *const OptionalOwnedOutputs<C>) as usize
    }
}
