//! One reusable child owner and finite per-Value evidence/identity reservations.
use crate::{
    KernelCompositeError, KernelCompositeHost, PreparedValueInvocation, ValueInvocationEvidence,
};
use alloc::vec::Vec;
use conduit_core::ActivePlayId;
use conduit_kernel::HostedSignLog;

pub(crate) enum ValueReuseError {
    InvalidBound,
    InvalidLifecycle,
    Child(KernelCompositeError),
}
enum Slot {
    Ready(KernelCompositeHost),
    Drained(KernelCompositeHost),
    Leased,
}
pub(crate) struct ReusableValuePool {
    slot: Slot,
    maximum: u16,
    bound: bool,
    pending: Vec<(PreparedValueInvocation, HostedSignLog)>,
    pub(crate) evidence: Vec<ValueInvocationEvidence>,
}
impl ReusableValuePool {
    pub(crate) fn new(host: KernelCompositeHost, maximum: u16) -> Result<Self, ValueReuseError> {
        if maximum == 0 {
            return Err(ValueReuseError::InvalidBound);
        }
        Ok(Self {
            slot: Slot::Ready(host),
            maximum,
            bound: false,
            pending: Vec::with_capacity(usize::from(maximum - 1)),
            evidence: Vec::with_capacity(usize::from(maximum - 1)),
        })
    }

    pub(crate) fn bind(
        &mut self,
        parent: &ActivePlayId,
        activation: &str,
    ) -> Result<(), ValueReuseError> {
        let Slot::Ready(host) = &mut self.slot else {
            return Err(ValueReuseError::InvalidLifecycle);
        };
        if self.bound || !self.pending.is_empty() {
            return Err(ValueReuseError::InvalidLifecycle);
        }
        host.bind_parent_play(parent, activation, 0)
            .map_err(ValueReuseError::Child)?;
        for index in (1..self.maximum).rev() {
            self.pending.push((
                host.prepare_value_invocation(parent, activation, index)
                    .map_err(ValueReuseError::Child)?,
                host.empty_value_evidence()
                    .map_err(ValueReuseError::Child)?,
            ));
        }
        self.bound = true;
        Ok(())
    }
    pub(crate) fn has_admitted_capacity(&self, expected: usize) -> bool {
        usize::from(self.maximum) == expected
            && matches!(self.slot, Slot::Ready(_))
            && self.pending.capacity() >= expected - 1
            && self.evidence.capacity() >= expected - 1
    }
    pub(crate) fn remaining(&self) -> usize {
        self.pending.len() + usize::from(matches!(self.slot, Slot::Ready(_)))
    }
    pub(crate) fn take(&mut self) -> Result<KernelCompositeHost, ValueReuseError> {
        if !self.bound || self.remaining() == 0 {
            return Err(ValueReuseError::InvalidLifecycle);
        }
        match core::mem::replace(&mut self.slot, Slot::Leased) {
            Slot::Ready(host) => Ok(host),
            other => {
                self.slot = other;
                Err(ValueReuseError::InvalidLifecycle)
            }
        }
    }
    pub(crate) fn retire(&mut self, host: KernelCompositeHost) {
        debug_assert!(matches!(self.slot, Slot::Leased));
        self.slot = Slot::Drained(host);
    }
    /// Called only after the containing controller's output acceptance.
    pub(crate) fn advance(&mut self) -> Result<(), ValueReuseError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut host = match core::mem::replace(&mut self.slot, Slot::Leased) {
            Slot::Drained(host) => host,
            other => {
                self.slot = other;
                return Err(ValueReuseError::InvalidLifecycle);
            }
        };
        let next = self.pending.pop();
        let Some((binding, signs)) = next else {
            self.slot = Slot::Drained(host);
            return Err(ValueReuseError::InvalidLifecycle);
        };
        match host.rearm_value(binding, signs) {
            Ok(evidence) => {
                self.evidence.push(evidence);
                self.slot = Slot::Ready(host);
                Ok(())
            }
            Err(error) => {
                self.slot = Slot::Drained(host);
                Err(ValueReuseError::Child(error))
            }
        }
    }
    pub(crate) fn host(&self) -> Option<&KernelCompositeHost> {
        match &self.slot {
            Slot::Ready(h) | Slot::Drained(h) => Some(h),
            Slot::Leased => None,
        }
    }
    pub(crate) fn drained(&self) -> Option<&KernelCompositeHost> {
        match &self.slot {
            Slot::Drained(h) => Some(h),
            _ => None,
        }
    }
}
