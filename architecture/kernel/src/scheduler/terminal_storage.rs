//! Immutable terminal-law storage, separate from mutable terminal progress.
use super::{AssignedTerminalTransduction, SchedulerError};
type Contracts<const N: usize, const P: usize> = [[Option<AssignedTerminalTransduction>; P]; N];

#[derive(Clone, Copy, Debug)]
pub(super) struct TerminalContracts<const N: usize, const P: usize, const M: usize> {
    inline: Contracts<M, P>,
    linked: Option<&'static Contracts<N, P>>,
}
impl<const N: usize, const P: usize, const M: usize> TerminalContracts<N, P, M> {
    pub(super) fn empty_owned() -> Result<Self, SchedulerError> {
        if M != N {
            return Err(SchedulerError::InvalidPlan);
        }
        Ok(Self {
            inline: [[None; P]; M],
            linked: None,
        })
    }
    #[cfg(feature = "alloc")]
    pub(super) fn owned(contracts: &Contracts<N, P>) -> Result<Self, SchedulerError> {
        let mut selected = Self::empty_owned()?;
        selected.inline.copy_from_slice(contracts);
        Ok(selected)
    }
    #[cfg(feature = "alloc")]
    pub(super) fn linked(contracts: &'static Contracts<N, P>) -> Self {
        Self {
            inline: [[None; P]; M],
            linked: Some(contracts),
        }
    }
    /// Linked law custody is immutable for this scheduler lifetime. A same-law
    /// bind is harmless; changing it requires a fresh checked scheduler.
    pub(super) fn replace(&mut self, contracts: &Contracts<N, P>) -> Result<(), SchedulerError> {
        if let Some(linked) = self.linked {
            return if linked == contracts {
                Ok(())
            } else {
                Err(SchedulerError::InvalidPlan)
            };
        }
        if M != N {
            return Err(SchedulerError::InvalidPlan);
        }
        self.inline.copy_from_slice(contracts);
        Ok(())
    }
}
impl<const N: usize, const P: usize, const M: usize> core::ops::Deref
    for TerminalContracts<N, P, M>
{
    type Target = Contracts<N, P>;
    fn deref(&self) -> &Self::Target {
        match self.linked {
            Some(linked) => linked,
            None => self
                .inline
                .as_slice()
                .try_into()
                .expect("owned terminal row count checked"),
        }
    }
}
impl<const N: usize, const P: usize, const M: usize> PartialEq<Contracts<N, P>>
    for TerminalContracts<N, P, M>
{
    fn eq(&self, other: &Contracts<N, P>) -> bool {
        &**self == other
    }
}
