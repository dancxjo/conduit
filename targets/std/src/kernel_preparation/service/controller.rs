//! Finite round controller; this forwards exact Cords through the existing kernel.
use crate::kernel_preparation::worker::{InitializedRegionWorker, WorkerError, WorkerReply};
use conduit_kernel::{
    scheduler::RemoteIngressOutcome, CanonicalValue, CordId, RemoteTerminalDisposition,
};

pub(super) struct Route {
    pub cord: CordId,
    pub source: usize,
    pub sink: usize,
    closed: bool,
}
impl Route {
    pub(super) fn new(cord: CordId, source: usize, sink: usize) -> Self {
        Self {
            cord,
            source,
            sink,
            closed: false,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RegionAdvance {
    Pending,
    Progress,
    /// One bounded sweep; this does not assert semantic completion.
    RoundComplete,
}
#[derive(Clone, Copy)]
enum Phase {
    StepSend(usize),
    StepWait(usize),
    OfferSend,
    OfferWait,
    AdmitSend,
    AdmitWait,
    AckSend,
    AckWait,
    TerminalSend,
    TerminalWait,
    AbnormalSend,
    AbnormalWait,
    CloseSend(Option<CanonicalValue>),
    CloseWait,
}
pub(super) struct RegionController {
    routes: Vec<Route>,
    buffer: Box<[u8]>,
    phase: Phase,
    route: usize,
    sequence: u64,
    bytes: usize,
    failure: Option<WorkerError>,
}
impl RegionController {
    pub(super) fn new(routes: Vec<Route>, transfer_bytes: usize) -> Self {
        Self {
            routes,
            buffer: vec![0; transfer_bytes].into_boxed_slice(),
            phase: Phase::StepSend(0),
            route: 0,
            sequence: 0,
            bytes: 0,
            failure: None,
        }
    }
    pub(super) fn failure(&self) -> Option<WorkerError> {
        self.failure
    }
    pub(super) fn backing_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.routes.capacity() * std::mem::size_of::<Route>()
            + self.buffer.len()
    }
    /// One command submission or reply observation per call; Busy never spins.
    pub(super) fn advance(
        &mut self,
        workers: &mut [InitializedRegionWorker],
    ) -> Result<RegionAdvance, WorkerError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = self.advance_once(workers);
        match result {
            Err(WorkerError::Busy) => Ok(RegionAdvance::Pending),
            Err(error) => {
                self.failure = Some(error);
                Err(error)
            }
            other => other,
        }
    }
    fn next_route(&mut self) {
        self.route += 1;
        self.phase = Phase::OfferSend;
    }
    fn advance_once(
        &mut self,
        workers: &mut [InitializedRegionWorker],
    ) -> Result<RegionAdvance, WorkerError> {
        use Phase::*;
        if workers.is_empty() {
            return Err(WorkerError::NotInstalled);
        }
        match self.phase {
            StepSend(lane) => {
                workers[lane].step()?;
                self.phase = if lane + 1 < workers.len() {
                    StepSend(lane + 1)
                } else {
                    StepWait(0)
                };
                return Ok(RegionAdvance::Progress);
            }
            StepWait(lane) => {
                let Some(reply) = workers[lane].take_reply(&mut [])? else {
                    return Ok(RegionAdvance::Pending);
                };
                if !matches!(reply?, WorkerReply::Step(_)) {
                    return Err(WorkerError::UnexpectedReply);
                }
                self.phase = if lane + 1 < workers.len() {
                    StepWait(lane + 1)
                } else {
                    OfferSend
                };
                return Ok(RegionAdvance::Progress);
            }
            _ => {}
        }
        if self.route >= self.routes.len() {
            self.route = 0;
            self.phase = StepSend(0);
            return Ok(RegionAdvance::RoundComplete);
        }
        let route = &mut self.routes[self.route];
        if route.source >= workers.len()
            || route.sink >= workers.len()
            || route.source == route.sink
        {
            return Err(WorkerError::InvalidLane);
        }
        if route.closed {
            self.next_route();
            return Ok(RegionAdvance::Progress);
        }
        match self.phase {
            OfferSend => {
                workers[route.source].offer(route.cord)?;
                self.phase = OfferWait;
            }
            AdmitSend => {
                workers[route.sink].admit(route.cord, self.sequence, &self.buffer[..self.bytes])?;
                self.phase = AdmitWait;
            }
            AckSend => {
                workers[route.source].delivered(route.cord, self.sequence)?;
                self.phase = AckWait;
            }
            TerminalSend => {
                workers[route.source].terminal(route.cord)?;
                self.phase = TerminalWait;
            }
            AbnormalSend => {
                workers[route.source].abnormal(route.cord)?;
                self.phase = AbnormalWait;
            }
            CloseSend(value) => {
                match value {
                    Some(value) => workers[route.sink].close_abnormal(route.cord, value)?,
                    None => workers[route.sink].close(route.cord)?,
                }
                self.phase = CloseWait;
            }
            phase => {
                let lane = match phase {
                    AdmitWait | CloseWait => route.sink,
                    _ => route.source,
                };
                let Some(reply) = workers[lane].take_reply(&mut self.buffer)? else {
                    return Ok(RegionAdvance::Pending);
                };
                match (phase, reply?) {
                    (
                        OfferWait,
                        WorkerReply::Offer {
                            sequence: Some(sequence),
                            bytes,
                        },
                    ) => {
                        self.sequence = sequence;
                        self.bytes = bytes;
                        self.phase = AdmitSend;
                    }
                    (
                        OfferWait,
                        WorkerReply::Offer {
                            sequence: None,
                            bytes: 0,
                        },
                    ) => self.phase = TerminalSend,
                    (
                        AdmitWait,
                        WorkerReply::Ingress(RemoteIngressOutcome::Accepted { sequence }),
                    ) if sequence == self.sequence => self.phase = AckSend,
                    (AdmitWait, WorkerReply::Ingress(RemoteIngressOutcome::Full { sequence }))
                        if sequence == self.sequence =>
                    {
                        self.next_route()
                    }
                    (AckWait, WorkerReply::Delivered) => self.next_route(),
                    (TerminalWait, WorkerReply::Terminal(None)) => self.next_route(),
                    (
                        TerminalWait,
                        WorkerReply::Terminal(Some(RemoteTerminalDisposition::NormalClose)),
                    ) => self.phase = CloseSend(None),
                    (
                        TerminalWait,
                        WorkerReply::Terminal(Some(RemoteTerminalDisposition::Abnormal)),
                    ) => self.phase = AbnormalSend,
                    (AbnormalWait, WorkerReply::Abnormal(Some(value))) => {
                        self.phase = CloseSend(Some(value))
                    }
                    (CloseWait, WorkerReply::Closed) => {
                        route.closed = true;
                        self.next_route();
                    }
                    _ => return Err(WorkerError::UnexpectedReply),
                }
            }
        }
        Ok(RegionAdvance::Progress)
    }
}
