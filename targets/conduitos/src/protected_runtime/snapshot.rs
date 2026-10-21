//! One checked Face forwarding lease; Root retains publication authority.
use super::*;
use alloc::vec::Vec;
use conduitos_runtime_core::snapshot as canonical;
pub(crate) struct Snapshot {
    broker: Scheduler,
    started: bool,
}
impl Snapshot {
    pub fn new(admission: canonical::Admission) -> Result<Self, SchedulerError> {
        #[cfg(conduitos_runtime_image)]
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.elf"));
        #[cfg(not(conduitos_runtime_image))]
        let bytes: &[u8] = &[];
        let image = ServiceImage::for_profile(bytes, ServiceProfile::OrdinaryRuntime)
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let mut domain = ServiceDomain::install(&image).map_err(|_| SchedulerError::InvalidPlan)?;
        let window = domain
            .transfer_window()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        window.fill(0);
        let length = wire::snapshot_admission::encode(&admission, window)
            .map_err(|_| SchedulerError::InvalidPlan)?;
        // Snapshot has no Host Calls. This empty effect ledger grants nothing.
        let ledger = Ledger::new(&Admission::new(1, 0))?;
        let mut broker = Scheduler {
            instance: domain.instance(),
            domain: Some(domain),
            sequence: 1,
            error: None,
            active: 1,
            value_bytes: 0,
            value_slots: 0,
            text_mode: false,
            sign_limit: canonical::SIGNS as u16,
            pending_limit: 0,
            expected_requests: [None; NODES],
            stats: wire::Stats {
                decisions: 0,
                signs: 0,
                pending: 0,
            },
            bindings: [None; NODES],
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
        if broker.request(Request::InitSnapshot {
            bytes: length as u32,
        })? != Response::Ready
        {
            return broker.fail(SchedulerError::InvalidPlan);
        }
        Ok(Self {
            broker,
            started: false,
        })
    }
    pub fn forward(&mut self, bytes: &[u8]) -> Result<(Vec<u8>, u16), SchedulerError> {
        let result = self.forward_inner(bytes);
        if let Err(error) = result {
            self.broker.abort();
            return Err(error);
        }
        result
    }
    fn forward_inner(&mut self, bytes: &[u8]) -> Result<(Vec<u8>, u16), SchedulerError> {
        if self.started || bytes.is_empty() || bytes.len() > canonical::VALUE_BYTES {
            return Err(SchedulerError::InvalidPlan);
        }
        self.started = true;
        let mut offset = 0;
        for chunk in bytes.chunks(conduitos_runtime_core::memory::TRANSFER_BYTES) {
            let window = self
                .broker
                .domain
                .as_mut()
                .ok_or(SchedulerError::Cancelled)?
                .transfer_window()
                .map_err(|_| SchedulerError::Cancelled)?;
            window.fill(0);
            window[..chunk.len()].copy_from_slice(chunk);
            if self.broker.request(Request::SnapshotWrite {
                offset,
                bytes: chunk.len() as u32,
                total: bytes.len() as u32,
            })? != Response::Ack
            {
                return Err(SchedulerError::InvalidPlan);
            }
            offset += chunk.len() as u32;
        }
        let Response::Snapshot {
            bytes: total,
            signs,
        } = self.broker.request(Request::SnapshotRun)?
        else {
            return Err(SchedulerError::InvalidPlan);
        };
        if total as usize != bytes.len() || signs as usize > canonical::SIGNS {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut observed = Vec::with_capacity(bytes.len());
        offset = 0;
        while offset < total {
            let maximum =
                (total - offset).min(conduitos_runtime_core::memory::TRANSFER_BYTES as u32);
            let Response::SnapshotChunk {
                offset: at,
                bytes: count,
                total: returned,
            } = self
                .broker
                .request(Request::SnapshotRead { offset, maximum })?
            else {
                return Err(SchedulerError::InvalidPlan);
            };
            if at != offset || returned != total || count == 0 || count > maximum {
                return Err(SchedulerError::InvalidPlan);
            }
            let end = offset + count;
            let window = self
                .broker
                .domain
                .as_ref()
                .ok_or(SchedulerError::Cancelled)?
                .output()
                .map_err(|_| SchedulerError::Cancelled)?;
            let value = window
                .get(..count as usize)
                .ok_or(SchedulerError::InvalidPlan)?;
            // A guest completion cannot replace Root-authorized Face contents.
            if value != &bytes[offset as usize..end as usize] {
                return Err(SchedulerError::InvalidPlan);
            }
            observed.extend_from_slice(value);
            offset = end;
        }
        Ok((observed, signs))
    }
    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        self.broker.cancel()
    }
}
