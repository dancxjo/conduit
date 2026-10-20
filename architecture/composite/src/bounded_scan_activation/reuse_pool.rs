//! One mutable child lease plus finite immutable per-invocation reservations.
use super::*;
use crate::{PreparedValueInvocation, ValueInvocationEvidence};
use conduit_kernel::HostedSignLog;

enum Slot {
    Ready(KernelCompositeHost),
    Drained(KernelCompositeHost),
    Leased,
}
pub(super) struct ReusableScanPool {
    slot: Slot,
    maximum: u16,
    bound: bool,
    pending: Vec<(PreparedValueInvocation, HostedSignLog)>,
    pub(super) evidence: Vec<ValueInvocationEvidence>,
}
impl ReusableScanPool {
    pub(super) fn new(host: KernelCompositeHost, maximum: u16) -> Result<Self, BoundedScanError> {
        if maximum == 0 {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        Ok(Self {
            slot: Slot::Ready(host),
            maximum,
            bound: false,
            pending: Vec::with_capacity(usize::from(maximum - 1)),
            evidence: Vec::with_capacity(usize::from(maximum - 1)),
        })
    }

    pub(super) fn bind(
        &mut self,
        parent: &ActivePlayId,
        activation: &str,
    ) -> Result<(), BoundedScanError> {
        let Slot::Ready(host) = &mut self.slot else {
            return Err(BoundedScanError::InvalidLifecycle);
        };
        if self.bound || !self.pending.is_empty() {
            return Err(BoundedScanError::InvalidLifecycle);
        }
        host.bind_parent_play(parent, activation, 0)
            .map_err(BoundedScanError::Refused)?;
        for index in (1..self.maximum).rev() {
            self.pending.push((
                host.prepare_value_invocation(parent, activation, index)
                    .map_err(BoundedScanError::Refused)?,
                host.empty_value_evidence()
                    .map_err(BoundedScanError::Refused)?,
            ));
        }
        self.bound = true;
        Ok(())
    }
    pub(super) fn remaining(&self) -> usize {
        self.pending.len() + usize::from(matches!(self.slot, Slot::Ready(_)))
    }
    pub(super) fn take(&mut self) -> Result<KernelCompositeHost, BoundedScanError> {
        if !self.bound || self.remaining() == 0 {
            return Err(BoundedScanError::InvalidLifecycle);
        }
        match core::mem::replace(&mut self.slot, Slot::Leased) {
            Slot::Ready(host) => Ok(host),
            other => {
                self.slot = other;
                Err(BoundedScanError::InvalidLifecycle)
            }
        }
    }
    pub(super) fn retire(&mut self, host: KernelCompositeHost) {
        debug_assert!(matches!(self.slot, Slot::Leased));
        self.slot = Slot::Drained(host);
    }
    /// Called only after the containing controller's output acceptance.
    pub(super) fn advance(&mut self) -> Result<(), BoundedScanError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut host = match core::mem::replace(&mut self.slot, Slot::Leased) {
            Slot::Drained(host) => host,
            other => {
                self.slot = other;
                return Err(BoundedScanError::InvalidLifecycle);
            }
        };
        let next = self.pending.pop();
        let Some((binding, signs)) = next else {
            self.slot = Slot::Drained(host);
            return Err(BoundedScanError::InvalidLifecycle);
        };
        match host.rearm_value(binding, signs) {
            Ok(evidence) => {
                self.evidence.push(evidence);
                self.slot = Slot::Ready(host);
                Ok(())
            }
            Err(error) => {
                self.slot = Slot::Drained(host);
                Err(BoundedScanError::Refused(error))
            }
        }
    }
    pub(super) fn host(&self) -> Option<&KernelCompositeHost> {
        match &self.slot {
            Slot::Ready(h) | Slot::Drained(h) => Some(h),
            Slot::Leased => None,
        }
    }
    pub(super) fn drained(&self) -> Option<&KernelCompositeHost> {
        match &self.slot {
            Slot::Drained(h) => Some(h),
            _ => None,
        }
    }
}
impl BoundedScanActivationHost {
    pub(super) fn remaining_children(&self) -> usize {
        self.reusable
            .as_ref()
            .map_or(self.ready.len(), ReusableScanPool::remaining)
    }
    pub(super) fn retire_child(&mut self, host: KernelCompositeHost) {
        if let Some(pool) = &mut self.reusable {
            pool.retire(host);
        } else {
            self.receipts.push(host);
        }
    }
    pub(super) fn observed_child(&self) -> Option<&KernelCompositeHost> {
        self.active
            .as_ref()
            .or_else(|| self.reusable.as_ref().and_then(ReusableScanPool::host))
            .or_else(|| self.receipts.last())
            .or_else(|| self.ready.last())
    }
}
