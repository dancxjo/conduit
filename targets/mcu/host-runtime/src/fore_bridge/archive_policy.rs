//! Optional evidence custody, never a replenishment of operation authority.
use crate::execution_sign_budget::DuplexSignBudget;
use conduit_kernel::{
    dictionary_live_sign_archive::DictionaryLiveSignLog,
    scheduler::{FixedScheduler, StepBack},
    ValueStorage,
};

mod sealed {
    pub trait Sealed {}
}
/// Default observer policy: existing finite lifetime behavior is unchanged.
pub struct NoArchive;
impl sealed::Sealed for NoArchive {}
/// Counts belong to this observer for its entire original invocation. Only
/// successful lossless transfer advances archived; no peer ACK implies storage.
pub struct DictionaryEvidenceWindow {
    width: u16,
    accepted: u64,
    delivered: u64,
    archived: u64,
}
impl sealed::Sealed for DictionaryEvidenceWindow {}
impl DictionaryEvidenceWindow {
    pub(super) fn new(budget: DuplexSignBudget) -> Self {
        Self {
            width: budget.ingress_items(),
            accepted: 0,
            delivered: 0,
            archived: 0,
        }
    }
    pub(super) fn totals(&self) -> (u64, u64, u64) {
        (self.accepted, self.delivered, self.archived)
    }
}
/// Sealed to the inert default and exact same-owner dictionary implementation.
pub trait EvidencePolicy<K>: sealed::Sealed {
    const RENEWING: bool;
    fn can_accept(&self) -> bool;
    fn accepted(&mut self);
    fn can_deliver(&self) -> bool;
    fn delivered(&mut self);
    fn renew(&mut self, kernel: &mut K, remaining: &mut Option<u16>);
}
impl<K> EvidencePolicy<K> for NoArchive {
    const RENEWING: bool = false;
    fn can_accept(&self) -> bool {
        true
    }
    fn accepted(&mut self) {}
    fn can_deliver(&self) -> bool {
        true
    }
    fn delivered(&mut self) {}
    fn renew(&mut self, _: &mut K, _: &mut Option<u16>) {}
}
impl<
        D: StepBack<P>,
        V: ValueStorage,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const K: usize,
        const M: usize,
        const F: usize,
        const U: usize,
        const Z: usize,
        const E: usize,
        const DT: usize,
        const G: usize,
        const DN: usize,
        const S: usize,
    >
    EvidencePolicy<
        FixedScheduler<
            D,
            V,
            DictionaryLiveSignLog<E, DT, G, DN, S>,
            N,
            C,
            P,
            Q,
            R,
            T,
            H,
            K,
            M,
            F,
            U,
            Z,
        >,
    > for DictionaryEvidenceWindow
{
    const RENEWING: bool = true;
    fn can_accept(&self) -> bool {
        self.accepted.checked_add(1).is_some()
    }
    fn accepted(&mut self) {
        self.accepted += 1;
    }
    fn can_deliver(&self) -> bool {
        self.delivered < self.accepted && self.delivered.checked_add(1).is_some()
    }
    fn delivered(&mut self) {
        self.delivered += 1;
    }
    fn renew(
        &mut self,
        kernel: &mut FixedScheduler<
            D,
            V,
            DictionaryLiveSignLog<E, DT, G, DN, S>,
            N,
            C,
            P,
            Q,
            R,
            T,
            H,
            K,
            M,
            F,
            U,
            Z,
        >,
        remaining: &mut Option<u16>,
    ) {
        if *remaining != Some(0)
            || self.accepted != self.delivered
            || self.delivered.checked_sub(self.archived) != Some(u64::from(self.width))
        {
            return;
        }
        // Refusal retains all active mandatory evidence and keeps ingress under
        // pressure. It is not an observer error and never retires local runners.
        if let Ok(transfer) = kernel.prepare_sign_prefix_transfer() {
            transfer.commit();
            self.archived = self.delivered;
            *remaining = Some(self.width);
        }
    }
}
