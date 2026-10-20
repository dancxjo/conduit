//! Borrowed control and inspection of the existing activation owner.
use super::*;
impl BoundedActivationHost {
    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        self.active.as_mut()?.next_host_request()
    }

    pub fn host_request_obligation(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<&crate::KernelCompositeHostCallObligation, BoundedActivationError> {
        self.active
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .host_request_obligation(request)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn host_request_view(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<crate::KernelCompositeHostRequestView<'_>, BoundedActivationError> {
        self.active
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .host_request_view(request)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn admit_host_request(
        &self,
        request: &KernelCompositeHostRequest,
        host: &conduit_core::PreparationHostIdentity,
        resources: &[conduit_core::ResourceBinding],
        authorities: &[conduit_core::AuthorityBinding],
    ) -> Result<AdmittedKernelCompositeHostRequest, BoundedActivationError> {
        self.active
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .admit_host_request(request, host, resources, authorities)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn complete_host_call(
        &mut self,
        request: &AdmittedKernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), BoundedActivationError> {
        self.active
            .as_mut()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .complete_host_call(request, outcome)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn host_request_input(
        &self,
        request: &AdmittedKernelCompositeHostRequest,
    ) -> Result<&[u8], BoundedActivationError> {
        self.active
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .host_request_input(request)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn complete_host_call_bytes(
        &mut self,
        request: &AdmittedKernelCompositeHostRequest,
        bytes: &[u8],
    ) -> Result<(), BoundedActivationError> {
        self.active
            .as_mut()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .complete_host_call_bytes(request, bytes)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn signs(&self) -> BTreeMap<conduit_core::HostId, Vec<KernelEvent>> {
        let mut signs = BTreeMap::new();
        if let Some(pool) = &self.reusable {
            for evidence in &pool.evidence {
                for host in evidence.active_plays().keys() {
                    use conduit_kernel::SignQuery;
                    signs
                        .entry(host.clone())
                        .or_insert_with(Vec::new)
                        .extend(evidence.signs().events());
                }
            }
        }
        for receipt in self
            .receipts
            .iter()
            .chain(self.active.iter())
            .chain(self.reusable.iter().filter_map(|pool| pool.drained()))
        {
            for (host, events) in receipt.signs() {
                signs.entry(host).or_insert_with(Vec::new).extend(events);
            }
        }
        signs
    }

    pub fn allocation_capacities(&self) -> (usize, usize) {
        (self.ready.capacity(), self.receipts.capacity())
    }

    /// Resolve a failure index against the exact prepared child identities.
    pub fn child_identity(&self, child: usize) -> Option<&conduit_core::HostId> {
        self.active
            .as_ref()
            .or_else(|| self.reusable.as_ref().and_then(|pool| pool.host()))
            .or_else(|| self.receipts.last())
            .or_else(|| self.ready.last())
            .and_then(|host| host.child_identity(child))
    }

    pub fn last_cancellation_failures(
        &self,
    ) -> &[(usize, conduit_kernel::scheduler::SchedulerError)] {
        self.reusable
            .as_ref()
            .and_then(|pool| pool.host())
            .or_else(|| self.receipts.last())
            .map_or(&[], KernelCompositeHost::cancellation_failures)
    }

    pub fn remaining_items(&self) -> usize {
        self.reusable
            .as_ref()
            .map_or(self.ready.len(), |pool| pool.remaining())
    }
}
