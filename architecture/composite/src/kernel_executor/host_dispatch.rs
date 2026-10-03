//! Correlate and consume exactly admitted Host Call dispatches.
use super::*;

impl KernelCompositeHost {
    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        if !self.started || self.cancelled {
            return None;
        }
        let slot = self
            .outstanding_host_calls
            .iter()
            .position(Option::is_none)?;
        let next_dispatch_token = self.next_dispatch_token.checked_add(1)?;
        let surfaced = self
            .children
            .iter_mut()
            .enumerate()
            .find_map(|(index, (_, kernel))| {
                kernel.next_host_request().map(|request| (index, request))
            })?;
        let (child_index, request) = surfaced;
        let child = self.children.keys().nth(child_index)?;
        let obligation_identity = self
            .host_call_obligations
            .iter()
            .find(|((host, node, call), _)| {
                host == child && *node == request.node && *call == request.call
            })
            .map(|(_, (identity, _))| *identity)
            .expect("lowered Host Call has a sealed obligation");
        let dispatch_token = self.next_dispatch_token;
        self.next_dispatch_token = next_dispatch_token;
        self.outstanding_host_calls[slot] = Some(OutstandingHostCall {
            token: dispatch_token,
            child_index,
            request,
            obligation_identity,
        });
        Some(KernelCompositeHostRequest { dispatch_token })
    }

    pub fn host_request_view(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<KernelCompositeHostRequestView<'_>, KernelCompositeError> {
        let outstanding = self.outstanding_host_call(request.dispatch_token)?;
        let child = self.child_id(outstanding.child_index)?;
        Ok(KernelCompositeHostRequestView {
            child,
            request: &outstanding.request,
            obligation_identity: &outstanding.obligation_identity,
        })
    }

    pub fn admitted_host_request_view(
        &self,
        request: &AdmittedKernelCompositeHostRequest,
    ) -> Result<KernelCompositeHostRequestView<'_>, KernelCompositeError> {
        let outstanding = self.outstanding_host_call(request.dispatch_token)?;
        let child = self.child_id(outstanding.child_index)?;
        Ok(KernelCompositeHostRequestView {
            child,
            request: &outstanding.request,
            obligation_identity: &outstanding.obligation_identity,
        })
    }

    pub fn host_request_obligation(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<&KernelCompositeHostCallObligation, KernelCompositeError> {
        let request = self.outstanding_host_call(request.dispatch_token)?;
        let child = self.child_id(request.child_index)?;
        self.host_call_obligations
            .iter()
            .find(|((host, node, call), _)| {
                host == child && *node == request.request.node && *call == request.request.call
            })
            .map(|(_, (_, obligation))| obligation)
            .ok_or_else(|| {
                KernelCompositeError::InvalidBoundary("Host Call has no selected obligation".into())
            })
    }

    pub fn admit_host_request(
        &self,
        request: &KernelCompositeHostRequest,
        host: &PreparationHostIdentity,
        resources: &[ResourceBinding],
        authorities: &[AuthorityBinding],
    ) -> Result<AdmittedKernelCompositeHostRequest, KernelCompositeError> {
        let exact = self.host_request_obligation(request)?;
        if !dispatch_matches(exact, host, resources, authorities) {
            return Err(KernelCompositeError::HostCallDispatchMismatch);
        }
        Ok(AdmittedKernelCompositeHostRequest {
            dispatch_token: request.dispatch_token,
        })
    }

    pub fn complete_host_call(
        &mut self,
        admitted: &AdmittedKernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let slot = self.outstanding_host_call_index(admitted.dispatch_token)?;
        let (child_index, request) = {
            let outstanding = self.outstanding_host_calls[slot]
                .as_ref()
                .expect("resolved Host Call slot is occupied");
            (outstanding.child_index, outstanding.request)
        };
        self.children
            .values_mut()
            .nth(child_index)
            .ok_or(KernelCompositeError::StaleHostCallChild)?
            .complete_host_call(request.node, request.request, outcome)
            .map_err(KernelCompositeError::InvalidBoundary)?;
        self.outstanding_host_calls[slot] = None;
        Ok(())
    }

    /// Resolve the exact admitted input for a surfaced Host Call.
    pub fn host_request_input(
        &self,
        admitted: &AdmittedKernelCompositeHostRequest,
    ) -> Result<&[u8], KernelCompositeError> {
        let request = self.outstanding_host_call(admitted.dispatch_token)?;
        let child = self.child_id(request.child_index)?;
        self.children
            .get(child)
            .ok_or(KernelCompositeError::StaleHostCallChild)?
            .host_value(request.request.input.value)
            .map_err(|reason| execution(child, reason))
    }

    /// Store a bounded adapter result in the owning child and complete its call.
    pub fn complete_host_call_bytes(
        &mut self,
        admitted: &AdmittedKernelCompositeHostRequest,
        bytes: &[u8],
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let slot = self.outstanding_host_call_index(admitted.dispatch_token)?;
        let (child_index, request, maximum_output_bytes) = {
            let outstanding = self.outstanding_host_calls[slot]
                .as_ref()
                .expect("resolved Host Call slot is occupied");
            let child = self.child_id(outstanding.child_index)?;
            let obligation = self
                .host_call_obligations
                .iter()
                .find(|((host, node, call), _)| {
                    host == child
                        && *node == outstanding.request.node
                        && *call == outstanding.request.call
                })
                .map(|(_, (_, obligation))| obligation)
                .ok_or_else(|| {
                    KernelCompositeError::InvalidBoundary(
                        "Host Call has no selected obligation".into(),
                    )
                })?;
            (
                outstanding.child_index,
                outstanding.request,
                obligation.requirement.maximum_output_bytes,
            )
        };
        if bytes.len() > maximum_output_bytes as usize {
            return Err(KernelCompositeError::HostCallOutputExceeded);
        }
        let child = self
            .children
            .values_mut()
            .nth(child_index)
            .ok_or(KernelCompositeError::StaleHostCallChild)?;
        let value = child
            .store_host_value(bytes)
            .map_err(KernelCompositeError::InvalidBoundary)?;
        let output = conduit_kernel::BoundedValueRef::new(value, bytes.len() as u32)
            .map_err(|error| KernelCompositeError::InvalidBoundary(format!("{error:?}")))?;
        child
            .complete_host_call(
                request.node,
                request.request,
                conduit_kernel::HostCallOutcome {
                    disposition: conduit_kernel::HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )
            .map_err(KernelCompositeError::InvalidBoundary)?;
        self.outstanding_host_calls[slot] = None;
        Ok(())
    }

    pub(super) fn outstanding_host_call_index(
        &self,
        token: u64,
    ) -> Result<usize, KernelCompositeError> {
        outstanding_host_call_index(&self.outstanding_host_calls, token)
    }

    pub(super) fn outstanding_host_call(
        &self,
        token: u64,
    ) -> Result<&OutstandingHostCall, KernelCompositeError> {
        let slot = self.outstanding_host_call_index(token)?;
        Ok(self.outstanding_host_calls[slot]
            .as_ref()
            .expect("resolved Host Call slot is occupied"))
    }

    pub(super) fn child_id(&self, index: usize) -> Result<&HostId, KernelCompositeError> {
        self.children
            .keys()
            .nth(index)
            .ok_or(KernelCompositeError::StaleHostCallChild)
    }
}

pub(super) fn dispatch_matches(
    exact: &KernelCompositeHostCallObligation,
    host: &PreparationHostIdentity,
    resources: &[ResourceBinding],
    authorities: &[AuthorityBinding],
) -> bool {
    &exact.host == host && exact.resources == resources && exact.authorities == authorities
}

pub(super) fn invalid_host_call_token() -> KernelCompositeError {
    KernelCompositeError::InvalidHostCallToken
}

pub(super) fn outstanding_host_call_index(
    outstanding: &[Option<OutstandingHostCall>],
    token: u64,
) -> Result<usize, KernelCompositeError> {
    outstanding
        .iter()
        .position(|slot| slot.as_ref().is_some_and(|item| item.token == token))
        .ok_or_else(invalid_host_call_token)
}
