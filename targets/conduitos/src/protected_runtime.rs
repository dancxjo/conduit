//! Root Host Call broker for the independently owned canonical scheduler.
//! Root keeps only reviewed effect grants, pending calls and transient boundary
//! bytes. Scheduler, ready/cord/value/sign state belong to the Runtime guest.
mod ledger;
mod morse;
mod text;
use crate::{
    arch::ServiceDomain,
    protected_region::{DomainBackend, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
};
use conduit_kernel::{
    scheduler::{HostCallRequest, SchedulerError, SchedulerStatus},
    *,
};
use conduitos_runtime_core::{
    admission::Admission,
    wire::{self, Envelope, Request, Response},
    *,
};
use ledger::Ledger;

pub(crate) struct Scheduler {
    domain: Option<ServiceDomain>,
    instance: u64,
    sequence: u64,
    error: Option<SchedulerError>,
    active: usize,
    value_bytes: usize,
    value_slots: u16,
    text_mode: bool,
    sign_limit: u16,
    pending_limit: u16,
    expected_requests: [Option<RequestId>; NODES],
    stats: wire::Stats,
    bindings: [Option<HostCallBinding>; NODES],
    pending: [Option<HostCallRequest>; NODES],
    last_request: [Option<RequestId>; NODES],
    issued: [bool; NODES],
    ledger: Ledger,
    input: Option<ValueRef>,
    input_bytes: [u8; VALUE_BYTES],
    staged: Option<ValueRef>,
    staged_bytes: [u8; VALUE_BYTES],
    gap: Option<SignRetentionGap>,
}
// Supplemental proof diagnostics retain the original error and never grant an
// operation or change admission. Each event is one finite enum Debug value.
fn diagnose<T, E: core::fmt::Debug>(stage: &'static str, result: Result<T, E>) -> Result<T, E> {
    #[cfg(all(feature = "ordinary-domain-proof", target_os = "none"))]
    if let Err(error) = &result {
        crate::arch::early_write(
            alloc::format!("CONDUIT_RUNTIME_DIAGNOSTIC {stage} {error:?}\n").as_bytes(),
        );
    }
    let _ = stage;
    result
}
impl Scheduler {
    pub fn new(admission: Admission) -> Result<Self, SchedulerError> {
        let ledger = Ledger::new(&admission)?;
        #[cfg(conduitos_runtime_image)]
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.elf"));
        #[cfg(not(conduitos_runtime_image))]
        let bytes: &[u8] = &[];
        let image = ServiceImage::for_profile(bytes, ServiceProfile::Runtime)
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let mut domain = ServiceDomain::install(&image).map_err(|_| SchedulerError::InvalidPlan)?;
        let window = domain
            .transfer_window()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        window.fill(0);
        let length =
            wire::admission::encode(&admission, window).map_err(|_| SchedulerError::InvalidPlan)?;
        let mut result = Self {
            instance: domain.instance(),
            domain: Some(domain),
            sequence: 1,
            error: None,
            active: usize::from(admission.active_nodes),
            value_bytes: VALUE_BYTES,
            value_slots: 10,
            text_mode: false,
            sign_limit: SIGN_ITEMS as u16,
            pending_limit: NODES as u16,
            expected_requests: [None; NODES],
            stats: wire::Stats {
                decisions: 0,
                signs: 0,
                pending: 0,
            },
            bindings: admission.bindings,
            pending: [None; NODES],
            last_request: [None; NODES],
            issued: [false; NODES],
            ledger,
            input: None,
            input_bytes: [0; VALUE_BYTES],
            staged: None,
            staged_bytes: [0; VALUE_BYTES],
            gap: None,
        };
        if result.request(Request::Init {
            bytes: length as u32,
        })? != Response::Ready
        {
            return result.fail(SchedulerError::InvalidPlan);
        }
        Ok(result)
    }
    fn quarantine(&mut self) {
        if let Some(mut domain) = self.domain.take() {
            domain.quarantine();
            drop(domain);
        }
        self.pending.fill(None);
        self.ledger.clear();
        self.input = None;
        self.staged = None;
        self.input_bytes.fill(0);
        self.staged_bytes.fill(0);
    }
    fn fail<T>(&mut self, error: SchedulerError) -> Result<T, SchedulerError> {
        self.error = Some(error);
        self.quarantine();
        Err(error)
    }
    fn request(&mut self, request: Request) -> Result<Response, SchedulerError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let result = self.exchange(request);
        match result {
            Ok(Response::Failed(error)) | Err(error) => self.fail(error),
            Ok(response) => {
                if self.text_mode && !matches!(request, Request::Stats) {
                    if let Err(error) = self.refresh_stats(request, response) {
                        return self.fail(error);
                    }
                }
                Ok(response)
            }
        }
    }
    fn exchange(&mut self, request: Request) -> Result<Response, SchedulerError> {
        let envelope = Envelope {
            instance: self.instance,
            request: self.sequence,
        };
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(SchedulerError::DecisionLimitExceeded)?;
        let domain = self.domain.as_mut().ok_or(SchedulerError::Cancelled)?;
        let exchange = domain.exchange().map_err(|_| SchedulerError::Cancelled)?;
        exchange.fill(0);
        wire::encode_request(envelope, request, exchange)
            .map_err(|_| SchedulerError::InvalidHostCallAccess)?;
        let returned =
            diagnose("guest-entry", domain.enter(1)).map_err(|_| SchedulerError::Cancelled)?;
        if returned != DomainReturn::Yielded {
            #[cfg(all(feature = "ordinary-domain-proof", target_os = "none"))]
            crate::arch::early_write(
                alloc::format!("CONDUIT_RUNTIME_DIAGNOSTIC guest-return {returned:?}\n").as_bytes(),
            );
            return Err(SchedulerError::Cancelled);
        }
        let exchange = domain.exchange().map_err(|_| SchedulerError::Cancelled)?;
        let len = wire::encoded_len(exchange).map_err(|_| SchedulerError::InvalidHostCallAccess)?;
        let (returned, response, gap) = wire::decode_response(
            exchange
                .get(..len)
                .ok_or(SchedulerError::InvalidHostCallAccess)?,
        )
        .map_err(|_| SchedulerError::InvalidHostCallAccess)?;
        if returned != envelope || !response.matches(request) {
            return Err(SchedulerError::InvalidHostCallAccess);
        }
        self.gap = gap;
        Ok(response)
    }
    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        let response = self.request(Request::Step)?;
        let Response::Status(status) = response else {
            return self.fail(SchedulerError::InvalidPlan);
        };
        if matches!(status,SchedulerStatus::Progress{node} if usize::from(node.0)>=self.active) {
            return self.fail(SchedulerError::InvalidPlan);
        }
        self.issued.fill(false);
        self.input = None;
        Ok(status)
    }
    /// The original API returns Option; a refused dispatch latches terminal
    /// failure, and the immediately following step reports that exact error.
    pub fn next_host_request(&mut self) -> Option<HostCallRequest> {
        match self.next() {
            Ok(request) => request,
            Err(error) => {
                let _: Result<(), _> = self.fail(error);
                None
            }
        }
    }
    fn next(&mut self) -> Result<Option<HostCallRequest>, SchedulerError> {
        let response = self.request(Request::Next)?;
        let Response::Next(request) = response else {
            return Err(SchedulerError::InvalidHostCallAccess);
        };
        let Some(request) = request else {
            self.input = None;
            return Ok(None);
        };
        let node = usize::from(request.node.0);
        if node >= self.active
            || self.issued[node]
            || self.pending[node].is_some()
            || self.last_request[node].is_some_and(|last| request.request.0 <= last.0)
        {
            return Err(SchedulerError::HostCallRequestDuplicate);
        }
        let binding = self.bindings[node].ok_or(SchedulerError::InvalidHostCallAccess)?;
        let length = request.input.value.byte_len as usize;
        if request.call != binding.call
            || request.input.admitted_bytes > binding.maximum_input_bytes
            || request.input.value.byte_len > request.input.admitted_bytes
            || length > self.value_bytes
            || request.input.value.slot >= self.value_slots
            || self.expected_requests[node].is_some_and(|expected| request.request != expected)
        {
            return Err(SchedulerError::InvalidHostCallAccess);
        }
        let output = self
            .domain
            .as_ref()
            .ok_or(SchedulerError::Cancelled)?
            .output()
            .map_err(|_| SchedulerError::Cancelled)?;
        let bytes = output
            .get(..length)
            .ok_or(SchedulerError::InvalidHostCallAccess)?;
        self.ledger.accept(request, bytes)?;
        self.input_bytes.fill(0);
        self.input_bytes[..length].copy_from_slice(bytes);
        self.input = Some(request.input.value);
        self.issued[node] = true;
        self.pending[node] = Some(request);
        self.last_request[node] = Some(request.request);
        Ok(Some(request))
    }
    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.input != Some(value) {
            return Err(SchedulerError::ValueOwnershipViolation);
        }
        self.input_bytes
            .get(..value.byte_len as usize)
            .ok_or(SchedulerError::ValueOwnershipViolation)
    }
    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.staged.is_some() || bytes.len() > self.value_bytes {
            return self.fail(SchedulerError::HostCallOutputExceeded);
        }
        let result = self.stage(bytes);
        match result {
            Ok(value) => Ok(value),
            Err(error) => self.fail(error),
        }
    }
    fn stage(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        let window = self
            .domain
            .as_mut()
            .ok_or(SchedulerError::Cancelled)?
            .transfer_window()
            .map_err(|_| SchedulerError::Cancelled)?;
        window.fill(0);
        window
            .get_mut(..bytes.len())
            .ok_or(SchedulerError::HostCallOutputExceeded)?
            .copy_from_slice(bytes);
        let response = self.request(Request::Store {
            bytes: bytes.len() as u32,
        })?;
        let Response::Stored(value) = response else {
            return Err(SchedulerError::HostCallOutputExceeded);
        };
        if value.byte_len as usize != bytes.len() || value.slot >= self.value_slots {
            return Err(SchedulerError::HostCallOutputExceeded);
        }
        self.staged = Some(value);
        self.staged_bytes.fill(0);
        self.staged_bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(value)
    }
    pub fn complete_host_call(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        let result = self.complete(node, request, outcome);
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.fail(error),
        }
    }
    fn complete(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let index = usize::from(node.0);
        let pending = self
            .pending
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::HostCallCompletionRejected)?;
        if pending.node != node || pending.request != request {
            return Err(SchedulerError::HostCallCompletionRejected);
        }
        let binding = self.bindings[index].ok_or(SchedulerError::InvalidHostCallAccess)?;
        match outcome.disposition {
            HostCallDisposition::Completed => {
                if outcome.failure.is_some() {
                    return Err(SchedulerError::HostCallCompletionRejected);
                }
                if let Some(output) = outcome.output {
                    if self.staged != Some(output.value)
                        || output.admitted_bytes > binding.maximum_output_bytes
                        || output.value.byte_len > output.admitted_bytes
                    {
                        return Err(SchedulerError::HostCallOutputExceeded);
                    }
                    self.ledger.publish(
                        index,
                        Some(output.value),
                        &self.staged_bytes[..output.value.byte_len as usize],
                    )?;
                } else if self.staged.is_some() {
                    return Err(SchedulerError::ValueOwnershipViolation);
                }
            }
            HostCallDisposition::Failed
            | HostCallDisposition::Denied
            | HostCallDisposition::Cancelled => {
                if outcome.output.is_some() || outcome.failure.is_none() || self.staged.is_some() {
                    return Err(SchedulerError::HostCallCompletionRejected);
                }
            }
        }
        if self.request(Request::Complete {
            node,
            request,
            outcome,
        })? != Response::Ack
        {
            return Err(SchedulerError::HostCallCompletionRejected);
        }
        self.pending[index] = None;
        self.staged = None;
        self.staged_bytes.fill(0);
        Ok(())
    }
    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        let result = self.request(Request::Cancel);
        self.quarantine();
        self.error = Some(SchedulerError::Cancelled);
        match result {
            Ok(Response::Cancelled) => Ok(()),
            Ok(_) => Err(SchedulerError::InvalidPlan),
            Err(error) => Err(error),
        }
    }
    /// Retire a failed Root transaction without entering guest code again.
    pub fn abort(&mut self) {
        self.quarantine();
        self.error = Some(SchedulerError::Cancelled);
    }
    pub fn sign_retention_gap(&self) -> Option<SignRetentionGap> {
        self.gap
    }
}
impl Drop for Scheduler {
    fn drop(&mut self) {
        self.quarantine();
    }
}
