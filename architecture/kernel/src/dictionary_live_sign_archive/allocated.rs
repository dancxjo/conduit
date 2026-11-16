use super::*;
use crate::{
    invocation_archive::ArchiveRefusal,
    live_sign_archive::{
        self, ArchiveLimits, LiveArchiveBackend, LiveSignLog, RejectedPreparation, TransferRefusal,
    },
    HostedSignLog,
};
use alloc::boxed::Box;
use core::{
    alloc::Layout,
    mem::{size_of, MaybeUninit},
};
/// Explicit prototype selection. No existing inline/boxed profile changes.
pub type DictionaryLiveSignLog<
    const E: usize,
    const T: usize,
    const G: usize,
    const N: usize,
    const S: usize,
> = LiveSignLog<E, G, S, Box<LivePrefixArchive<E, T, G, N, S>>>;
impl<const E: usize, const T: usize, const G: usize, const N: usize, const S: usize>
    live_sign_archive::backing_sealed::Sealed for Box<LivePrefixArchive<E, T, G, N, S>>
{
}
fn refusal(reason: DictionaryRefusal) -> ArchiveRefusal {
    match reason {
        DictionaryRefusal::Overflow => ArchiveRefusal::InvocationOverflow,
        DictionaryRefusal::StreamCapacity => ArchiveRefusal::StreamCapacity,
        _ => ArchiveRefusal::Capacity,
    }
}
impl<const E: usize, const T: usize, const G: usize, const N: usize, const S: usize>
    LiveArchiveBackend<E, G, S> for Box<LivePrefixArchive<E, T, G, N, S>>
{
    const ALLOCATION_BYTES: usize = size_of::<LivePrefixArchive<E, T, G, N, S>>();
    type Prefix<'a>
        = super::Prefix<'a, E, T, G, N, S>
    where
        Self: 'a;
    fn can_append(
        &self,
        evidence: &impl InvocationEvidence,
        span: u32,
    ) -> Result<(), ArchiveRefusal> {
        self.as_ref().can_append(evidence, span).map_err(refusal)
    }
    fn append(
        &mut self,
        evidence: &impl InvocationEvidence,
        span: u32,
    ) -> Result<(), ArchiveRefusal> {
        self.as_mut().append(evidence, span).map_err(refusal)
    }
    fn prefix(&self, index: u32) -> Option<(u32, Self::Prefix<'_>)> {
        let prefix = self.as_ref().prefix(index)?;
        Some((prefix.span(), prefix))
    }
}
impl<const E: usize, const T: usize, const G: usize, const N: usize, const S: usize>
    DictionaryLiveSignLog<E, T, G, N, S>
{
    pub fn dictionary_stats(&self) -> (usize, usize, usize, u32) {
        let archive = self.archive_backend();
        (
            archive.template_event_count(),
            archive.template_count(),
            archive.tape_run_count(),
            archive.len(),
        )
    }
    pub fn try_prepare_dictionary(
        active: HostedSignLog,
        limits: ArchiveLimits,
    ) -> Result<Self, RejectedPreparation> {
        Self::prepare_with_archive(active, limits, || {
            if E > u16::MAX as usize || T > u16::MAX as usize {
                return Err(TransferRefusal::Bounds);
            }
            let layout = Layout::new::<LivePrefixArchive<E, T, G, N, S>>();
            // SAFETY: counters ensure nonzero valid layout; null is handled fallibly.
            let pointer = unsafe { alloc::alloc::alloc(layout) };
            if pointer.is_null() {
                return Err(TransferRefusal::Allocation);
            }
            // SAFETY: exact-layout owned allocation; fieldwise initializer writes all
            // fields without callbacks/fallibility before this SAME Box is published.
            let mut storage = unsafe {
                Box::from_raw(pointer.cast::<MaybeUninit<LivePrefixArchive<E, T, G, N, S>>>())
            };
            unsafe { LivePrefixArchive::initialize_at(storage.as_mut_ptr()) };
            Ok(unsafe { storage.assume_init() })
        })
    }
}
