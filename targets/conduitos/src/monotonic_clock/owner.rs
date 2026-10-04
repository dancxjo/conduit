//! Exact possession and one bounded provider poll per admitted service step.
use super::{
    codec::{ClockDisposition, PreparedClockCodec},
    contract::*,
};
use crate::machine_membrane::selected_operation::{
    SelectedOperationContract, SelectedOperationPlan, bind_selected_operation,
};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub const MAXIMUM_POLL_STEPS: u64 = 4096;

/// A native owner supplies one bounded observation/IRQ check per call. It may
/// arm a finite timer internally; it must never wait in a loop or invent time.
/// Revocation quiesces its timer and prevents delivery of an old completion.
pub trait MonotonicDeadlineProvider {
    fn poll_until(&mut self, deadline_ms: u64) -> Result<Option<u64>, ClockDisposition>;
    fn revoke(&mut self);
}

pub struct ClockCallSelection<'a> {
    pub contract: &'a MonotonicClockContract,
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement: &'a PlacementId,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ClockCallRefusal {
    WrongBinding,
    Possession,
    StaleRequest,
    SequenceExhausted,
    Pending,
    Cancelled,
    Capability(BaseCapabilityRefusal),
    Canonical(StructuredInfoRefusal),
}
struct Pending {
    request: RequestId,
    deadline: u64,
    lease: BaseOperationLease,
    remaining: u64,
}
pub struct MonotonicClockHostCall<P> {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    provider: P,
    node: NodeId,
    next_request: u32,
    pending: Option<Pending>,
    codec: PreparedClockCodec,
    last_observed: Option<u64>,
    cancelled: bool,
}
impl<P: MonotonicDeadlineProvider> MonotonicClockHostCall<P> {
    /// # Safety
    /// The trusted native root owns exactly the selected clock generation and
    /// its millisecond calibration. The provider must preserve bounded polls,
    /// monotonic observations and quiescence on revocation. Descriptive clock
    /// discovery does not grant this possession or confine hostile native code.
    pub unsafe fn bind_selected(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        provider: P,
        selection: ClockCallSelection<'_>,
    ) -> Result<Self, ClockCallRefusal> {
        let mut entries = table.inspections();
        let entry = entries.next().ok_or(ClockCallRefusal::Possession)?;
        if entries.next().is_some()
            || entry.scope.maximum_in_flight != 1
            || entry.scope.maximum_work_units < MAXIMUM_POLL_STEPS
            || entry.scope.maximum_parameter_bytes < CLOCK_MAXIMUM_BYTES
            || entry.scope.maximum_result_bytes < CLOCK_MAXIMUM_BYTES
            || claim.parameter_bytes != CLOCK_MAXIMUM_BYTES
            || claim.work_units != MAXIMUM_POLL_STEPS
        {
            return Err(ClockCallRefusal::Possession);
        }
        drop(entries);
        let node = bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: selection.contract.kind(),
                call: CLOCK_CALL,
                input_bytes: CLOCK_MAXIMUM_BYTES,
                output_bytes: CLOCK_MAXIMUM_BYTES,
                resource_bytes: 1,
            },
            SelectedOperationPlan {
                fragment: selection.fragment,
                lowered: selection.lowered,
                active: selection.active,
                placement_id: selection.placement,
            },
        )
        .map_err(|_| ClockCallRefusal::WrongBinding)?;
        Ok(Self {
            table,
            handle,
            claim,
            provider,
            node,
            next_request: 0,
            pending: None,
            codec: PreparedClockCodec::new(selection.contract)
                .map_err(ClockCallRefusal::Canonical)?,
            last_observed: None,
            cancelled: false,
        })
    }
    pub fn start(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), ClockCallRefusal> {
        self.check_binding(node, call)?;
        if self.cancelled {
            return Err(ClockCallRefusal::Cancelled);
        }
        if self.pending.is_some() {
            return Err(ClockCallRefusal::Pending);
        }
        if request != RequestId(self.next_request) {
            return Err(ClockCallRefusal::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(ClockCallRefusal::SequenceExhausted)?;
        let deadline = self
            .codec
            .deadline(input)
            .map_err(ClockCallRefusal::Canonical)?;
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(ClockCallRefusal::Capability)?;
        self.next_request = next;
        self.pending = Some(Pending {
            request,
            deadline,
            lease,
            remaining: MAXIMUM_POLL_STEPS,
        });
        Ok(())
    }
    pub fn poll(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
    ) -> Result<Option<&[u8]>, ClockCallRefusal> {
        self.check_binding(node, call)?;
        if self.cancelled {
            return Err(ClockCallRefusal::Cancelled);
        }
        if self
            .pending
            .as_ref()
            .is_none_or(|pending| pending.request != request)
        {
            return Err(ClockCallRefusal::StaleRequest);
        }
        let mut pending = self
            .pending
            .take()
            .expect("validated pending clock request");
        let result = if pending.remaining == 0 {
            Err(ClockDisposition::Timeout)
        } else {
            pending.remaining -= 1;
            self.provider.poll_until(pending.deadline)
        };
        let encoded = match result {
            Ok(None) => {
                self.pending = Some(pending);
                return Ok(None);
            }
            Ok(Some(now))
                if now >= pending.deadline && self.last_observed.is_none_or(|last| now >= last) =>
            {
                self.last_observed = Some(now);
                self.codec.completed(now)
            }
            Ok(Some(_)) => self.codec.refused(ClockDisposition::Malformed),
            Err(reason) => self.codec.refused(reason),
        };
        let bytes = encoded.as_ref().map_or(0, |value| value.len() as u32);
        self.table
            .complete(&mut self.handle, pending.lease, bytes)
            .map_err(ClockCallRefusal::Capability)?;
        encoded.map(Some).map_err(ClockCallRefusal::Canonical)
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), ClockCallRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(ClockCallRefusal::Capability)?;
        self.pending = None;
        self.cancelled = true;
        self.provider.revoke();
        Ok(())
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), ClockCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            Err(ClockCallRefusal::WrongBinding)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
