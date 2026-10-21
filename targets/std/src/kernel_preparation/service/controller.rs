//! Finite round controller; this forwards exact Cords through the existing kernel.
use crate::kernel_preparation::worker::{InitializedRegionWorker, WorkerError, WorkerReply};
use conduit_kernel::{
    scheduler::{RemoteIngressOutcome, SchedulerStatus},
    CanonicalValue, CordId, RemoteTerminalDisposition,
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
/// A completed sweep of kernel statuses, not a semantic completion Sign.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RegionRoundState {
    statuses: [Option<SchedulerStatus>; 4],
    lanes: usize,
    pub open_transfers: usize,
    pub pressured_transfers: usize,
}
impl RegionRoundState {
    /// Progress node IDs are dense worker-local ordinals, as in the underlying
    /// fixed kernel. Retirement retains their original Plan node mapping.
    pub fn lane_status(&self, lane: usize) -> Option<SchedulerStatus> {
        if lane >= self.lanes {
            return None;
        }
        self.statuses[lane]
    }
    pub fn lane_count(&self) -> usize {
        self.lanes
    }
    /// Structural drain only; the Plan's semantic completion policy is separate.
    pub fn is_drained(&self) -> bool {
        self.open_transfers == 0
            && self.statuses[..self.lanes]
                .iter()
                .all(|status| *status == Some(SchedulerStatus::Drained))
    }
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
    statuses: [Option<SchedulerStatus>; 4],
    open_transfers: usize,
    pressured_transfers: usize,
    last_round: Option<RegionRoundState>,
}
impl RegionController {
    pub(super) fn new(routes: Vec<Route>, transfer_bytes: usize) -> Self {
        let open_transfers = routes.len();
        Self {
            routes,
            buffer: vec![0; transfer_bytes].into_boxed_slice(),
            phase: Phase::StepSend(0),
            route: 0,
            sequence: 0,
            bytes: 0,
            failure: None,
            statuses: [None; 4],
            open_transfers,
            pressured_transfers: 0,
            last_round: None,
        }
    }
    pub(super) fn round_state(&self) -> Option<RegionRoundState> {
        self.last_round
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
        if workers.len() > 4 {
            return Err(WorkerError::InvalidLane);
        }
        if workers.is_empty() {
            return Err(WorkerError::NotInstalled);
        }
        match self.phase {
            StepSend(lane) => {
                workers[lane].step()?;
                if lane == 0 {
                    self.pressured_transfers = 0;
                    self.statuses = [None; 4];
                }
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
                let WorkerReply::Step(status) = reply? else {
                    return Err(WorkerError::UnexpectedReply);
                };
                self.statuses[lane] = Some(status);
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
            self.last_round = Some(RegionRoundState {
                statuses: self.statuses,
                lanes: workers.len(),
                open_transfers: self.open_transfers,
                pressured_transfers: self.pressured_transfers,
            });
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
                        self.pressured_transfers += 1;
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
                        self.open_transfers -= 1;
                        self.next_route();
                    }
                    _ => return Err(WorkerError::UnexpectedReply),
                }
            }
        }
        Ok(RegionAdvance::Progress)
    }
}
