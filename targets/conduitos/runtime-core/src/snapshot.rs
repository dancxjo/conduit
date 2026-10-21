//! Canonical one-value Face Fore forwarding, without semantic Face authority.
use crate::{
    admission::Admission as Numeric,
    wire::{Request, Response},
};
use conduit_kernel::{
    scheduler::{
        FixedScheduler, RemoteIngressOutcome, RemoteTerminalDisposition, SchedulerError,
        SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    *,
};
pub const VALUE_BYTES: usize = 48 * 1024;
pub const SIGNS: usize = 64;
pub const REMOTE_SIGNS: u16 = 16;
pub const WORK_STEPS: usize = 32;
pub type Scheduler = FixedScheduler<
    SnapshotForward,
    FixedValueStore<2, VALUE_BYTES>,
    FixedSignLog<SIGNS>,
    1,
    2,
    { crate::PORTS },
    2,
    { crate::PORTS },
    2,
    1,
    1,
>;
pub struct Admission {
    pub numeric: Numeric,
    pub input_endpoint: RemoteEndpointId,
    pub input_cord: CordId,
    pub output_endpoint: RemoteEndpointId,
    pub output_cord: CordId,
}
impl Admission {
    pub fn build(self) -> Result<Runtime, SchedulerError> {
        let n = self.numeric;
        if n.active_nodes != 1
            || n.active_cords != 2
            || n.bindings.iter().any(Option::is_some)
            || n.ranges[crate::PORTS..].iter().any(Option::is_some)
            || n.targets[2..].iter().any(Option::is_some)
            || self.input_cord.0 >= 2
            || self.output_cord.0 >= 2
            || self.input_cord == self.output_cord
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut routes = FixedRoutes::<{ crate::PORTS }, 2>::new(crate::PORTS as u16);
        for (port, range) in n.ranges[..crate::PORTS].iter().enumerate() {
            let Some(range) = range else {
                continue;
            };
            let start = range.start as usize;
            let end = start
                .checked_add(range.len as usize)
                .ok_or(SchedulerError::InvalidPlan)?;
            let source = n
                .targets
                .get(start..end)
                .ok_or(SchedulerError::InvalidPlan)?;
            if source.len() > 2 {
                return Err(SchedulerError::InvalidPlan);
            }
            let mut targets = [RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(0), PortId(0)),
            }; 2];
            for (target, source) in targets.iter_mut().zip(source) {
                *target = source.ok_or(SchedulerError::InvalidPlan)?;
            }
            routes.install(
                NodeId(0),
                PortId(port as u16),
                *range,
                &targets[..source.len()],
            )?;
        }
        routes.seal()?;
        let values = FixedValueStore::<2, VALUE_BYTES>::new((2 * VALUE_BYTES) as u32)?;
        let signs = FixedSignLog::<SIGNS>::new_with_remote_storage(
            (SIGNS * core::mem::size_of::<KernelEvent>()) as u32,
            REMOTE_SIGNS,
            remote_sign_storage_bytes(REMOTE_SIGNS).ok_or(SchedulerError::InvalidPlan)?,
        )?;
        let scheduler = Scheduler::new(
            [n.nodes[0]],
            [n.cords[0], n.cords[1]],
            routes,
            [SnapshotForward],
            values,
            signs,
        )?;
        Ok(Runtime {
            scheduler,
            input_endpoint: self.input_endpoint,
            input_cord: self.input_cord,
            output_endpoint: self.output_endpoint,
            output_cord: self.output_cord,
            bytes: [0; VALUE_BYTES],
            total: 0,
            written: 0,
            read: 0,
            completed: false,
            started: false,
            cancelled: false,
        })
    }
}
pub struct Runtime {
    scheduler: Scheduler,
    input_endpoint: RemoteEndpointId,
    input_cord: CordId,
    output_endpoint: RemoteEndpointId,
    output_cord: CordId,
    bytes: [u8; VALUE_BYTES],
    total: usize,
    written: usize,
    read: usize,
    completed: bool,
    started: bool,
    cancelled: bool,
}
impl Runtime {
    pub fn gap(&self) -> Option<SignRetentionGap> {
        SignQuery::retention_gap(self.scheduler.signs())
    }
    pub fn execute(
        &mut self,
        request: Request,
        transfer: &mut [u8],
    ) -> Result<Response, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        match request {
            Request::SnapshotWrite {
                offset,
                bytes,
                total,
            } => {
                let (offset, count, total) = (offset as usize, bytes as usize, total as usize);
                if self.started
                    || count == 0
                    || count > crate::memory::TRANSFER_BYTES
                    || total == 0
                    || total > VALUE_BYTES
                    || offset != self.written
                    || (self.written != 0 && total != self.total)
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                let end = offset
                    .checked_add(count)
                    .ok_or(SchedulerError::InvalidPlan)?;
                if end > total {
                    return Err(SchedulerError::InvalidPlan);
                }
                self.bytes[offset..end]
                    .copy_from_slice(transfer.get(..count).ok_or(SchedulerError::InvalidPlan)?);
                self.total = total;
                self.written = end;
                transfer.fill(0);
                Ok(Response::Ack)
            }
            Request::SnapshotRun => {
                if self.started || self.total == 0 || self.written != self.total {
                    return Err(SchedulerError::InvalidPlan);
                }
                self.started = true;
                self.forward()?;
                self.completed = true;
                Ok(Response::Snapshot {
                    bytes: self.total as u32,
                    signs: self.scheduler.signs().len(),
                })
            }
            Request::SnapshotRead { offset, maximum } => {
                let (offset, maximum) = (offset as usize, maximum as usize);
                if !self.completed
                    || offset != self.read
                    || offset >= self.total
                    || maximum == 0
                    || maximum > crate::memory::TRANSFER_BYTES
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                let count = maximum.min(self.total - offset);
                transfer.fill(0);
                transfer
                    .get_mut(..count)
                    .ok_or(SchedulerError::InvalidPlan)?
                    .copy_from_slice(&self.bytes[offset..offset + count]);
                self.read += count;
                Ok(Response::SnapshotChunk {
                    offset: offset as u32,
                    bytes: count as u32,
                    total: self.total as u32,
                })
            }
            Request::Cancel => {
                self.scheduler.cancel()?;
                self.cancelled = true;
                self.bytes.fill(0);
                Ok(Response::Cancelled)
            }
            _ => Err(SchedulerError::InvalidPlan),
        }
    }
    fn forward(&mut self) -> Result<(), SchedulerError> {
        if self.scheduler.admit_remote_input(
            self.input_endpoint,
            self.input_cord,
            0,
            &self.bytes[..self.total],
        )? != (RemoteIngressOutcome::Accepted { sequence: 0 })
        {
            return Err(SchedulerError::InvalidPlan);
        }
        self.scheduler
            .close_remote_input(self.input_endpoint, self.input_cord)?;
        let mut delivered = false;
        for _ in 0..WORK_STEPS {
            if self.scheduler.next_host_request().is_some() {
                return Err(SchedulerError::InvalidHostCallAccess);
            }
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.output_endpoint, self.output_cord)?
            {
                let value = self.scheduler.host_value(offer.value)?;
                if delivered || value != &self.bytes[..self.total] {
                    return Err(SchedulerError::InvalidPlan);
                }
                // Retain the actual delivered bytes, not a separate publication truth.
                self.bytes[..self.total].copy_from_slice(value);
                self.scheduler.remote_egress_accept(
                    self.output_endpoint,
                    self.output_cord,
                    offer.sequence,
                )?;
                self.scheduler.remote_egress_delivered(
                    self.output_endpoint,
                    self.output_cord,
                    offer.sequence,
                )?;
                delivered = true;
            }
            match self.scheduler.step()? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Drained
                    if delivered
                        && self.scheduler.remote_egress_terminal_disposition(
                            self.output_endpoint,
                            self.output_cord,
                        )? == Some(RemoteTerminalDisposition::NormalClose) =>
                {
                    return Ok(());
                }
                _ => return Err(SchedulerError::InvalidPlan),
            }
        }
        Err(SchedulerError::DecisionLimitExceeded)
    }
}
#[derive(Clone, Copy)]
pub struct SnapshotForward;
impl StepBack<{ crate::PORTS }> for SnapshotForward {
    fn step(
        &mut self,
        io: &mut StepIo<{ crate::PORTS }>,
        _: &StepInputBytes<'_, { crate::PORTS }>,
    ) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume(PortId(0)).is_err() || io.send(PortId(0), value).is_err() {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 1,
                });
            }
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 2,
                });
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
