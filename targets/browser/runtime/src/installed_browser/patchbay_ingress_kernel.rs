//! One closing canonical command stream through the ordinary finite kernel.
use super::{PlannedPatchbayIngress, BYTES};
use conduit_kernel::scheduler::{FixedScheduler, HostCallRequest, SchedulerStatus};
use conduit_kernel::{
    FixedHostCallBindings, FixedRoutes, HostCallOutcome, HostedSignLog, HostedValueStore,
};

const PORTS: usize = super::super::BROWSER_PORTS_PER_GEAR;
type Kernel = FixedScheduler<
    super::super::BrowserBack,
    HostedValueStore,
    HostedSignLog,
    1,
    1,
    PORTS,
    1,
    PORTS,
    1,
    1,
    1,
>;

/// Prepared transport only. It contains no application model or owner evidence.
/// One interaction consumes this instance; a successor needs explicit admission.
pub struct PreparedPatchbayIngress {
    planned: PlannedPatchbayIngress,
    kernel: Box<Kernel>,
    submitted: bool,
    pending: Option<HostCallRequest>,
    terminal: bool,
    revoked: bool,
}

impl PreparedPatchbayIngress {
    pub fn prepare(planned: PlannedPatchbayIngress) -> Result<Self, String> {
        let lowered = planned.lowered();
        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| "Patchbay ingress node count")?;
        let cords = lowered
            .cords
            .iter()
            .map(|cord| cord.spec)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "Patchbay ingress cord count")?;
        let mut routes = FixedRoutes::<PORTS, 1>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(debug)?;
        }
        routes.seal().map_err(debug)?;
        let mut calls = FixedHostCallBindings::<1>::new(1);
        for call in &lowered.host_calls {
            calls.install(call.node, call.binding).map_err(debug)?;
        }
        calls.seal().map_err(debug)?;
        // The one remote input value transfers to the pending Host Call; it is
        // not copied into another queue or grown after preparation.
        let values = HostedValueStore::new(1, BYTES, BYTES).map_err(debug)?;
        const SIGNS: u16 = 64;
        const REMOTE_SIGNS: u16 = 4;
        let signs = HostedSignLog::new_with_remote_storage(
            SIGNS,
            u32::from(SIGNS) * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
            REMOTE_SIGNS,
            conduit_kernel::remote_sign_storage_bytes(REMOTE_SIGNS)
                .ok_or("Patchbay remote Sign bound")?,
        )
        .map_err(debug)?;
        let kernel = Kernel::new_with_host_calls(
            nodes,
            cords,
            routes,
            calls,
            [super::super::BrowserBack::face(BYTES, 1)],
            values,
            signs,
        )
        .map_err(debug)?;
        Ok(Self {
            planned,
            kernel: Box::new(kernel),
            submitted: false,
            pending: None,
            terminal: false,
            revoked: false,
        })
    }

    /// Actual prepared value backing, not the live logical byte quota.
    pub fn reserved_value_storage_bytes(&self) -> usize {
        self.kernel.values().reserved_storage_bytes()
    }

    pub fn is_ready(&self) -> bool {
        !self.submitted && !self.revoked && !self.terminal
    }

    /// This entrance has no selected Line: copying Mask bytes is local to one
    /// exact Host/Boot/generation. A foreign source requires separate Line admission.
    pub(super) fn validate_local_show(&self, show: &conduit_face::MaskShow) -> Result<(), String> {
        let [fragment] = self.planned.plan().fragments.as_slice() else {
            return Err("Patchbay ingress requires one local fragment".into());
        };
        let manifestation = &show.show;
        if fragment.host_id != manifestation.host_id
            || fragment.boot_id != manifestation.boot_id
            || fragment.offer_generation != manifestation.offer_generation
            || show.planned_mask.plan.fragments.iter().any(|mask| {
                mask.host_id != fragment.host_id
                    || mask.boot_id != fragment.boot_id
                    || mask.offer_generation != fragment.offer_generation
            })
        {
            return Err("patchbay-local-ingress-host-mismatch".into());
        }
        Ok(())
    }

    pub fn planned(&self) -> &PlannedPatchbayIngress {
        &self.planned
    }
    pub fn is_terminal(&self) -> bool {
        self.terminal
    }
    pub fn value_capacities(&self) -> (usize, usize) {
        self.kernel.values().allocation_capacities()
    }

    /// Revoke this transport on Show replacement or owner Play retirement.
    /// No owner callback runs; this does not cancel an already returned domain
    /// request. Revoke before cancelling so a cancellation failure cannot reopen
    /// the old input or expose its pending bytes to a later owner dispatch.
    pub fn retire(&mut self) -> Result<(), String> {
        self.revoked = true;
        if self.terminal {
            return Ok(());
        }
        self.kernel.cancel().map_err(debug)?;
        self.pending = None;
        self.terminal = true;
        Ok(())
    }

    /// Copy into the preallocated exact input cord, then close this stream.
    /// No owner callback runs here. Even an uncertain later failure cannot make
    /// this consumed transport available for an implicit resubmission.
    pub fn admit(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.submitted || self.revoked {
            return Err("Patchbay ingress is already consumed".into());
        }
        if bytes.is_empty() || bytes.len() > BYTES as usize {
            return Err("Patchbay interaction exceeds its finite input bound".into());
        }
        let input = self.planned.input();
        input.validate_value(bytes).map_err(debug)?;
        match self
            .kernel
            .admit_remote_input(input.endpoint, input.cord, 0, bytes)
            .map_err(debug)?
        {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => {
                self.submitted = true
            }
            conduit_kernel::scheduler::RemoteIngressOutcome::Full { .. } => {
                return Err("Patchbay ingress is full".into())
            }
        }
        self.kernel
            .close_remote_input(input.endpoint, input.cord)
            .map_err(debug)?;
        for _ in 0..8 {
            if let Some(call) = self.kernel.next_host_request() {
                self.pending = Some(call);
                return Ok(());
            }
            match self.kernel.step().map_err(debug)? {
                SchedulerStatus::Progress { .. } => {}
                other => {
                    return Err(format!(
                        "Patchbay ingress did not request its owner: {other:?}"
                    ))
                }
            }
        }
        Err("Patchbay ingress exceeded its finite dispatch steps".into())
    }

    /// Borrow only bytes that actually reached the planned kernel Host Call.
    /// The owner must recheck current Face/Show and preserve its typed request.
    pub fn pending_input(&self) -> Result<&[u8], String> {
        if self.revoked {
            return Err("Patchbay ingress was revoked".into());
        }
        let call = self
            .pending
            .ok_or("Patchbay ingress has no pending owner call")?;
        self.kernel.host_value(call.input.value).map_err(debug)
    }

    /// Complete the actual call after the owner reports its result. No output
    /// value is admitted by this sink, including on refusal or cancellation.
    pub fn complete(&mut self, outcome: HostCallOutcome) -> Result<(), String> {
        if self.revoked {
            return Err("Patchbay ingress was revoked".into());
        }
        if outcome.output.is_some() {
            return Err("Patchbay ingress admits no Host Call output".into());
        }
        let call = self
            .pending
            .ok_or("Patchbay ingress has no pending owner call")?;
        self.kernel
            .complete_host_call(call.node, call.request, outcome)
            .map_err(debug)?;
        self.pending = None;
        for _ in 0..8 {
            let status = match self.kernel.step() {
                Ok(status) => status,
                Err(error @ conduit_kernel::scheduler::SchedulerError::BackFailed(_)) => {
                    self.terminal = true;
                    return Err(debug(error));
                }
                Err(error) => return Err(debug(error)),
            };
            match status {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Drained => {
                    self.terminal = true;
                    return Ok(());
                }
                other => return Err(format!("Patchbay ingress did not terminate: {other:?}")),
            }
        }
        Err("Patchbay ingress exceeded its finite terminal steps".into())
    }
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("Patchbay ingress kernel: {error:?}")
}
