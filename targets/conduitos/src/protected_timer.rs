//! Root owns exact timer/serial authority; the private domain owns ordinary progress.
use crate::{
    arch::TextDomain,
    composition::{MachineRunError, MachineRunReceipt},
    domain_timer_gate::{TimerGate, TimerGateRefusal},
    machine::{
        IdleBase, InterruptBase, KernelInterest, MonotonicClockBase, SerialBase, TimerBase,
        TimerToken,
    },
    offer::HostOffer,
    protected_region::{DomainFault, DomainReturn, ProtectedRegion, RegionBinding},
    protection_domain::{KernelCapabilityHandle, KernelRevocationCause},
    tour_timer_kernel::runtime::PreparedTimerGraph,
};
use conduit_core::{ActivePlayIdentity, Plan};
use conduit_kernel::scheduler::HostCallRequest;

pub(crate) const ROOT_METADATA_CEILING: u32 =
    core::mem::size_of::<ProtectedTimer>() as u32 + 3 * (4 * 64 + 64);

pub(crate) struct ProtectedTimer {
    current: RegionBinding,
    region: ProtectedRegion<TextDomain>,
    gate: TimerGate,
    pending: Option<(TimerToken, HostCallRequest)>,
}

impl ProtectedTimer {
    #[cfg(feature = "ordinary-domain-proof")]
    pub(crate) fn cost(&self) -> crate::protected_region::DomainCost {
        self.region.cost()
    }

    pub fn prepare(
        plan: &Plan,
        active: &ActivePlayIdentity,
        offer: &HostOffer<'_>,
        graph: PreparedTimerGraph,
    ) -> Result<Self, MachineRunError> {
        let started = TextDomain::ticks();
        let fragment = plan
            .fragments
            .first()
            .ok_or(MachineRunError::KernelConstruction)?;
        let [region] = fragment.execution_regions.as_slice() else {
            return Err(MachineRunError::KernelConstruction);
        };
        if plan.fragments.len() != 1
            || region.execution_profile_id.as_str()
                != crate::ordinary_plan::PROTECTED_REGION_PROFILE
            || !region.isolation_required
            || !region.preemption_required
        {
            return Err(MachineRunError::KernelConstruction);
        }
        for placement in &fragment.placements {
            if !offer.capabilities.iter().any(|capability| {
                placement.kind_id.as_str() == capability.kind
                    && placement.kind_contract_revision.as_str() == capability.contract_revision
                    && placement.implementation_id.as_str() == capability.implementation
                    && placement.artifact_id.as_str()
                        == alloc::format!("conduitos-build/{}", capability.artifact_build)
            }) {
                return Err(MachineRunError::KernelConstruction);
            }
        }
        let current = RegionBinding::admit(
            plan,
            active,
            &region.region_id,
            crate::text_protection::allocate_domain()?,
        )
        .map_err(MachineRunError::ProtectionDomain)?;
        let table = crate::text_protection::capability_table(offer.generation)?;
        let gate =
            TimerGate::admit_with_table(plan, current.clone(), offer, table).map_err(gate_error)?;
        let backend = TextDomain::install().map_err(MachineRunError::ProtectionDomain)?;
        let mut owner = Self {
            region: ProtectedRegion::installed(current.clone(), backend),
            current,
            gate,
            pending: None,
        };
        let heap_bytes = [&owner.current, owner.region.binding(), owner.gate.binding()]
            .into_iter()
            .map(|binding| {
                binding.active.host_id.0.capacity()
                    + binding.active.boot_id.0.capacity()
                    + binding.active.plan_id.0.capacity()
                    + binding.active.active_play_id.0.capacity()
                    + binding.region.0.capacity()
            })
            .sum::<usize>();
        let root_bytes = u32::try_from(core::mem::size_of::<Self>() + heap_bytes)
            .map_err(|_| MachineRunError::KernelConstruction)?;
        if TextDomain::RESERVED_BYTES
            .checked_add(root_bytes)
            .is_none_or(|bytes| bytes > region.requirements.runtime_memory_bytes)
        {
            return Err(MachineRunError::KernelConstruction);
        }
        owner
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .preparation_cost(started, root_bytes);
        let (timer, count) = owner.gate.handles();
        owner
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .initialize_timer(graph, timer.raw_for_domain(), count.raw_for_domain())
            .map_err(MachineRunError::ProtectionDomain)?;
        owner.expect_yield()?;
        owner
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .timer_initialized()
            .map_err(MachineRunError::ProtectionDomain)?;
        Ok(owner)
    }

    fn enter(&mut self) -> Result<DomainReturn, MachineRunError> {
        let returned = self
            .region
            .resume(&self.current, 1, self.gate.capabilities())
            .map_err(MachineRunError::ProtectionDomain)?;
        match returned {
            DomainReturn::Fault(fault) => Err(MachineRunError::ProtectionFault(fault)),
            returned => Ok(returned),
        }
    }

    fn expect_yield(&mut self) -> Result<(), MachineRunError> {
        if self.enter()? == DomainReturn::Yielded {
            return Ok(());
        }
        self.region
            .fault(DomainFault::InvalidGate, self.gate.capabilities());
        Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate))
    }

    fn completion(&mut self, kind: u32, request: HostCallRequest) -> Result<(), MachineRunError> {
        self.region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .timer_completion(kind, request.node, request.request)
            .map_err(MachineRunError::ProtectionDomain)?;
        self.expect_yield()
    }

    pub fn run<
        C: MonotonicClockBase,
        T: TimerBase,
        S: SerialBase,
        I: InterruptBase,
        D: IdleBase,
    >(
        &mut self,
        clock: &mut C,
        timer: &mut T,
        serial: &mut S,
        interrupts: &mut I,
        idle: &mut D,
    ) -> Result<MachineRunReceipt, MachineRunError> {
        let started = clock.now();
        let state = interrupts.disable();
        let result = if interrupts.is_enabled() {
            Err(MachineRunError::InterruptBaseFailure)
        } else {
            self.run_masked(clock, timer, serial, idle, started)
        };
        // Retire the physical arm before revoking its lease or private memory.
        let cleanup = self
            .pending
            .take()
            .map(|(token, _)| timer.cancel(token))
            .transpose();
        self.gate.revoke(KernelRevocationCause::PlayCancelled);
        self.region.revoke(
            KernelRevocationCause::PlayCancelled,
            self.gate.capabilities(),
        );
        interrupts.restore(state);
        cleanup.map_err(|_| MachineRunError::TimerBaseFailure)?;
        result
    }

    fn run_masked<C: MonotonicClockBase, T: TimerBase, S: SerialBase, D: IdleBase>(
        &mut self,
        clock: &mut C,
        timer: &mut T,
        serial: &mut S,
        idle: &mut D,
        started: u64,
    ) -> Result<MachineRunReceipt, MachineRunError> {
        let mut presentations = 0;
        let mut wakes = 0;
        for _ in 0..512 {
            if let Some(wake) = timer
                .take_wake()
                .map_err(|_| MachineRunError::TimerBaseFailure)?
            {
                let (_, request) = self.pending.ok_or(MachineRunError::TimerBaseFailure)?;
                self.gate
                    .complete_wait(
                        wake,
                        timer.provider_generation(),
                        clock.provider_generation(),
                    )
                    .map_err(gate_error)?;
                self.pending = None;
                self.completion(1, request)?;
                wakes += 1;
            }
            self.region
                .backend_mut()
                .map_err(MachineRunError::ProtectionDomain)?
                .timer_command(11)
                .map_err(MachineRunError::ProtectionDomain)?;
            let returned = self.enter()?;
            let observation = self
                .region
                .backend_mut()
                .map_err(MachineRunError::ProtectionDomain)?
                .timer_observation()
                .map_err(MachineRunError::ProtectionDomain)?;
            if presentations == 2
                && wakes == 1
                && self.pending.is_some()
                && observation.pending == 1
                && observation.status == 1
                && returned == DomainReturn::Yielded
            {
                let (token, _) = self.pending.ok_or(MachineRunError::TimerBaseFailure)?;
                timer
                    .cancel(token)
                    .map_err(|_| MachineRunError::TimerBaseFailure)?;
                self.pending = None;
                self.region
                    .backend_mut()
                    .map_err(MachineRunError::ProtectionDomain)?
                    .timer_command(13)
                    .map_err(MachineRunError::ProtectionDomain)?;
                self.expect_yield()?;
                let stopped = self
                    .region
                    .backend_mut()
                    .map_err(MachineRunError::ProtectionDomain)?
                    .timer_observation()
                    .map_err(MachineRunError::ProtectionDomain)?;
                if stopped.status != 2 {
                    return Err(MachineRunError::KernelFailure);
                }
                return Ok(MachineRunReceipt {
                    logical_operations: 3,
                    decisions: stopped.decisions,
                    kernel_signs: stopped.signs,
                    timer_irq_wakes: wakes,
                    idle_entries: idle.idle_count(),
                    serial_presentations: serial.presentation_count(),
                    clock_monotonic: clock.now() >= started,
                    pending_host_calls: 1,
                    overlap_witness: false,
                    timer_pending_during_text_progress: true,
                    physical_parallelism: false,
                });
            }
            match returned {
                DomainReturn::Gate => {
                    let request = self
                        .region
                        .backend_mut()
                        .map_err(MachineRunError::ProtectionDomain)?
                        .timer_request()
                        .map_err(MachineRunError::ProtectionDomain)?;
                    let handle = KernelCapabilityHandle::from_untrusted(request.handle);
                    match (request.kind, request.operation) {
                        (1, 10) => {
                            if self.pending.is_some() {
                                return Err(MachineRunError::TimerBaseFailure);
                            }
                            let payload = &request.payload[..request.length];
                            self.gate
                                .begin_wait(
                                    request.request,
                                    handle,
                                    payload,
                                    request.work_units,
                                    timer.provider_generation(),
                                    clock.provider_generation(),
                                )
                                .map_err(gate_error)?;
                            let milliseconds = u64::from_le_bytes(
                                payload
                                    .try_into()
                                    .map_err(|_| MachineRunError::TimerBaseFailure)?,
                            );
                            let interest = KernelInterest {
                                node: request.request.node,
                                request: request.request.request,
                                input: request.request.input,
                            };
                            let token = timer
                                .arm_after_milliseconds(interest, milliseconds)
                                .map_err(|_| MachineRunError::TimerBaseFailure)?;
                            self.pending = Some((token, request.request));
                        }
                        (2, 11) => {
                            let digits = &request.payload[..request.length];
                            self.gate
                                .begin_count(
                                    request.request,
                                    handle,
                                    digits,
                                    request.work_units,
                                    serial.provider_generation(),
                                )
                                .map_err(gate_error)?;
                            serial
                                .present(digits)
                                .map_err(|_| MachineRunError::SerialBaseFailure)?;
                            self.gate
                                .complete_count(request.request, serial.provider_generation())
                                .map_err(gate_error)?;
                            self.completion(2, request.request)?;
                            presentations += 1;
                        }
                        _ => return Err(MachineRunError::UnexpectedHostCall),
                    }
                }
                DomainReturn::Yielded if observation.status == 1 && self.pending.is_some() => idle
                    .wait_for_interrupt()
                    .map_err(|_| MachineRunError::InterruptBaseFailure)?,
                _ => return Err(MachineRunError::KernelFailure),
            }
        }
        Err(MachineRunError::StepLimitExceeded)
    }
}

fn gate_error(error: TimerGateRefusal) -> MachineRunError {
    match error {
        TimerGateRefusal::Binding(error) => MachineRunError::ProtectionDomain(error),
        TimerGateRefusal::Capability(error) => MachineRunError::ProtectionCapability(error),
        TimerGateRefusal::Request | TimerGateRefusal::Revoked => {
            MachineRunError::UnexpectedHostCall
        }
    }
}

impl Drop for ProtectedTimer {
    fn drop(&mut self) {
        self.gate.revoke(KernelRevocationCause::PlayCancelled);
        self.region.revoke(
            KernelRevocationCause::PlayCancelled,
            self.gate.capabilities(),
        );
        use core::fmt::Write;
        let cost = self.region.cost();
        let mut sign = crate::sign_format::FixedText::new();
        if writeln!(sign, "CONDUIT_DOMAIN_TIMER_COST {{\"schema\":\"conduit.conduitos/domain-cost@1\",\"architecture\":\"{}\",\"plan_id\":\"{}\",\"play_id\":\"{}\",\"domain_id\":{},\"state\":\"{:?}\",\"entries\":{},\"privilege_transitions\":{},\"gate_transitions\":{},\"base_gate_transitions\":{},\"copied_bytes\":{},\"shared_peak_bytes\":{},\"shared_page_bytes\":4096,\"ring_slots\":0,\"interrupt_entries\":{},\"source_timer_interrupts\":{},\"setup_copied_bytes\":{},\"root_metadata_bytes\":{},\"reserved_bytes\":{},\"teardown_zeroed_bytes\":{},\"tlb_flushes\":{},\"address_space_switches\":{},\"scheduler_returns\":{},\"preemptions\":{},\"setup_ticks\":{},\"teardown_ticks\":{},\"tick_unit\":\"{}\",\"dma_isolation\":false,\"driver_isolation\":false}}",
            crate::arch::ARCHITECTURE, self.current.active.plan_id.as_str(), self.current.active.active_play_id.as_str(), self.current.domain.0, self.region.state(),
            cost.entries, cost.privilege_transitions, cost.gate_transitions, cost.base_gate_transitions,
            cost.copied_bytes, cost.shared_peak_bytes, cost.interrupt_entries, cost.source_timer_interrupts, cost.setup_copied_bytes, cost.root_metadata_bytes, cost.reserved_bytes, cost.teardown_zeroed_bytes,
            cost.tlb_flushes, cost.address_space_switches, cost.scheduler_returns, cost.preemptions,
            cost.setup_ticks, cost.teardown_ticks, TextDomain::TICK_UNIT).is_ok() { crate::arch::early_write(sign.as_bytes()); }
    }
}
