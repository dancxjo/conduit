//! Root capability leases for the finite standing timer's actual effect boundaries.
//! This owner never steps Every/Count, formats values, or manufactures timer wakes.
use crate::{
    domain_serial_scope::SerialScope,
    domain_timer_scope::TimerScope,
    machine::KernelInterest,
    offer::HostOffer,
    protected_region::{DomainRefusal, RegionBinding},
    protection_domain::{
        KernelCapabilityHandle, KernelCapabilityRefusal, KernelCapabilityScope,
        KernelCapabilityTable, KernelOperationClaim, KernelOperationLease, KernelRevocationCause,
        KernelRevocationReceipt,
    },
};
use conduit_core::Plan;
use conduit_kernel::{HostCallId, NodeId, scheduler::HostCallRequest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerGateRefusal {
    Binding(DomainRefusal),
    Capability(KernelCapabilityRefusal),
    Request,
    Revoked,
}

struct Pending {
    request: HostCallRequest,
    lease: KernelOperationLease,
}

pub struct TimerGate {
    binding: RegionBinding,
    table: KernelCapabilityTable,
    timer: TimerScope,
    count: SerialScope,
    timer_handle: KernelCapabilityHandle,
    count_handle: KernelCapabilityHandle,
    timer_node: NodeId,
    count_node: NodeId,
    period: u64,
    next_timer: u32,
    next_count: u32,
    pending_timer: Option<Pending>,
    pending_count: Option<Pending>,
    revoked: bool,
}

impl TimerGate {
    pub fn admit(
        plan: &Plan,
        binding: RegionBinding,
        fixed: &HostOffer<'_>,
        secret: u64,
    ) -> Result<Self, TimerGateRefusal> {
        let timer = TimerScope::admit(plan, &binding, fixed).map_err(TimerGateRefusal::Binding)?;
        let count =
            SerialScope::admit_count(plan, &binding, fixed).map_err(TimerGateRefusal::Binding)?;
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id == binding.active.host_id
                    && fragment.boot_id == binding.active.boot_id
            })
            .ok_or(TimerGateRefusal::Request)?;
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
            .map_err(|_| TimerGateRefusal::Request)?;
        let graph = crate::tour_timer_kernel::TourTimerKernel::prepare_graph(fragment, &lowered)
            .map_err(|_| TimerGateRefusal::Request)?;
        let mut table = KernelCapabilityTable::new(secret).map_err(TimerGateRefusal::Capability)?;
        let timer_handle = table
            .issue(binding.domain, timer.scope)
            .map_err(TimerGateRefusal::Capability)?;
        let count_handle = table
            .issue(binding.domain, count.scope)
            .map_err(TimerGateRefusal::Capability)?;
        Ok(Self {
            binding,
            table,
            timer,
            count,
            timer_handle,
            count_handle,
            timer_node: graph.timer,
            count_node: graph.presentation,
            period: graph.period,
            next_timer: 1,
            next_count: 1,
            pending_timer: None,
            pending_count: None,
            revoked: false,
        })
    }

    pub fn handles(&self) -> (KernelCapabilityHandle, KernelCapabilityHandle) {
        (self.timer_handle, self.count_handle)
    }

    /// Reserve the operation before the trusted provider arms hardware. The
    /// caller must quiesce and revoke if arming fails; a lease is not a wake.
    pub fn begin_wait(
        &mut self,
        request: HostCallRequest,
        handle: KernelCapabilityHandle,
        payload: &[u8],
        work_units: u32,
        timer_generation: Option<u64>,
        clock_generation: Option<u64>,
    ) -> Result<(), TimerGateRefusal> {
        self.check_active()?;
        check_request(&request, self.timer_node, self.next_timer)?;
        if self.pending_timer.is_some() || payload != self.period.to_le_bytes() || work_units != 1 {
            return Err(TimerGateRefusal::Request);
        }
        let scope = self
            .timer
            .current(&self.binding, timer_generation, clock_generation)
            .map_err(TimerGateRefusal::Binding)?;
        let lease = self
            .table
            .authorize_current(
                self.binding.domain,
                handle,
                &scope,
                claim(scope, payload.len() as u32, work_units),
            )
            .map_err(TimerGateRefusal::Capability)?;
        self.pending_timer = Some(Pending { request, lease });
        self.next_timer += 1;
        Ok(())
    }

    /// Only a current, exact provider wake retires the outstanding timer lease.
    /// The containing owner delivers semantic completion to the domain afterward.
    pub fn complete_wait(
        &mut self,
        wake: KernelInterest,
        timer_generation: Option<u64>,
        clock_generation: Option<u64>,
    ) -> Result<(), TimerGateRefusal> {
        self.check_active()?;
        let pending = self
            .pending_timer
            .as_ref()
            .ok_or(TimerGateRefusal::Request)?;
        if interest(pending.request) != wake {
            return Err(TimerGateRefusal::Request);
        }
        self.timer
            .current(&self.binding, timer_generation, clock_generation)
            .map_err(TimerGateRefusal::Binding)?;
        self.table
            .complete(pending.lease)
            .map_err(TimerGateRefusal::Capability)?;
        self.pending_timer = None;
        Ok(())
    }

    /// Authorize bounded digits already rendered by the domain. Root does not
    /// decode the semantic count or substitute expected fixture values.
    pub fn begin_count(
        &mut self,
        request: HostCallRequest,
        handle: KernelCapabilityHandle,
        digits: &[u8],
        work_units: u32,
        serial_generation: Option<u64>,
    ) -> Result<(), TimerGateRefusal> {
        self.check_active()?;
        check_request(&request, self.count_node, self.next_count)?;
        if self.pending_count.is_some()
            || digits.is_empty()
            || digits.len() > 20
            || !digits.iter().all(u8::is_ascii_digit)
            || (digits.len() > 1 && digits[0] == b'0')
            || work_units != 1
        {
            return Err(TimerGateRefusal::Request);
        }
        let scope = self
            .count
            .current(&self.binding, serial_generation)
            .map_err(TimerGateRefusal::Binding)?;
        let lease = self
            .table
            .authorize_current(
                self.binding.domain,
                handle,
                &scope,
                claim(scope, digits.len() as u32, work_units),
            )
            .map_err(TimerGateRefusal::Capability)?;
        self.pending_count = Some(Pending { request, lease });
        self.next_count += 1;
        Ok(())
    }

    pub fn complete_count(
        &mut self,
        request: HostCallRequest,
        serial_generation: Option<u64>,
    ) -> Result<(), TimerGateRefusal> {
        self.check_active()?;
        let pending = self
            .pending_count
            .as_ref()
            .ok_or(TimerGateRefusal::Request)?;
        if pending.request != request {
            return Err(TimerGateRefusal::Request);
        }
        self.count
            .current(&self.binding, serial_generation)
            .map_err(TimerGateRefusal::Binding)?;
        self.table
            .complete(pending.lease)
            .map_err(TimerGateRefusal::Capability)?;
        self.pending_count = None;
        Ok(())
    }

    pub fn pending_wait(&self) -> Option<KernelInterest> {
        self.pending_timer
            .as_ref()
            .map(|pending| interest(pending.request))
    }

    /// The containing physical owner must quiesce an outstanding hardware arm
    /// before discarding its token. Revocation refuses every later gate/wake.
    pub fn revoke(&mut self, cause: KernelRevocationCause) -> KernelRevocationReceipt {
        self.revoked = true;
        self.pending_timer = None;
        self.pending_count = None;
        self.table.revoke_domain(self.binding.domain, cause)
    }

    fn check_active(&self) -> Result<(), TimerGateRefusal> {
        if self.revoked {
            Err(TimerGateRefusal::Revoked)
        } else {
            Ok(())
        }
    }
}

fn check_request(
    request: &HostCallRequest,
    node: NodeId,
    sequence: u32,
) -> Result<(), TimerGateRefusal> {
    if request.node != node
        || request.call != HostCallId(0)
        || request.request.0 != sequence
        || !(1..=2).contains(&sequence)
        || request.input.value.slot >= 6
        || request.input.value.generation == 0
        || request.input.value.byte_len != 8
        || request.input.admitted_bytes != 8
    {
        Err(TimerGateRefusal::Request)
    } else {
        Ok(())
    }
}

fn interest(request: HostCallRequest) -> KernelInterest {
    KernelInterest {
        node: request.node,
        request: request.request,
        input: request.input,
    }
}

fn claim(
    scope: KernelCapabilityScope,
    parameter_bytes: u32,
    work_units: u32,
) -> KernelOperationClaim {
    KernelOperationClaim {
        boot: scope.boot,
        plan: scope.plan,
        play: scope.play,
        base_generation: scope.base_generation,
        resource_generation: scope.resource_generation,
        operation: scope.operation,
        parameter_bytes,
        work_units,
    }
}

#[cfg(test)]
mod tests;
