//! Ordinary text uses the same broker with its independent service lease.
use super::*;
use conduitos_runtime_core::text as canonical;
impl Scheduler {
    pub fn new_text(admission: canonical::Admission) -> Result<Self, SchedulerError> {
        let roles = [
            admission.literal_node.0 as usize,
            admission.upper_node.0 as usize,
            admission.face_node.0 as usize,
        ];
        if roles.iter().any(|n| *n >= canonical::NODES)
            || roles[0] == roles[1]
            || roles[0] == roles[2]
            || roles[1] == roles[2]
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let upper = admission.bindings[roles[1]].ok_or(SchedulerError::InvalidPlan)?;
        let face = admission.bindings[roles[2]].ok_or(SchedulerError::InvalidPlan)?;
        if admission.bindings[roles[0]].is_some()
            || upper.call != HostCallId(0)
            || face.call != HostCallId(0)
            || upper.maximum_input_bytes != 256
            || upper.maximum_output_bytes != 256
            || face.maximum_input_bytes != 256
            || face.maximum_output_bytes != 0
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let ledger = Ledger::new_text(&admission)?;
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
        let length = wire::text_admission::encode(&admission, window)
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let mut bindings = [None; NODES];
        bindings[..canonical::NODES].copy_from_slice(&admission.bindings);
        let mut expected_requests = [None; NODES];
        expected_requests[roles[1]] = Some(RequestId(1));
        expected_requests[roles[2]] = Some(RequestId(2));
        let mut result = Self {
            instance: domain.instance(),
            domain: Some(domain),
            sequence: 1,
            error: None,
            active: canonical::NODES,
            value_bytes: canonical::MAX_VALUE_BYTES,
            value_slots: 6,
            text_mode: true,
            sign_limit: canonical::SIGN_ITEMS as u16,
            pending_limit: 3,
            expected_requests,
            stats: wire::Stats {
                decisions: 0,
                signs: 0,
                pending: 0,
            },
            bindings,
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
        if result.request(Request::InitText {
            bytes: length as u32,
        })? != Response::Ready
        {
            return result.fail(SchedulerError::InvalidPlan);
        }
        Ok(result)
    }
}
impl<const BYTES: usize> Broker<BYTES> {
    pub(super) fn refresh_stats(
        &mut self,
        request: Request,
        response: Response,
    ) -> Result<(), SchedulerError> {
        let Response::Stats(stats) = self.exchange(Request::Stats)? else {
            return Err(SchedulerError::InvalidPlan);
        };
        // Queued guest calls may not have been dispatched yet. Root's own
        // pending calls provide a lower bound; account for the just-acknowledged
        // transaction before the public method updates its local ledger.
        let mut minimum_pending = self.pending.iter().filter(|p| p.is_some()).count();
        match (request, response) {
            (Request::Complete { node, request, .. }, Response::Ack)
                if self
                    .pending
                    .get(node.0 as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|p| p.request == request) =>
            {
                minimum_pending = minimum_pending.saturating_sub(1);
            }
            (Request::Next, Response::Next(Some(_))) => minimum_pending += 1,
            (Request::Cancel, Response::Cancelled) => minimum_pending = 0,
            _ => {}
        }
        if stats.decisions < self.stats.decisions
            || stats.signs < self.stats.signs
            || stats.signs > self.sign_limit
            || stats.pending > self.pending_limit
            || (matches!(
                response,
                Response::Status(SchedulerStatus::Drained | SchedulerStatus::Cancelled)
            ) && stats.pending != 0)
            || (stats.pending as usize) < minimum_pending
            || self.gap.is_some()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        self.stats = stats;
        Ok(())
    }
    pub fn decisions(&self) -> u32 {
        self.stats.decisions
    }
    pub fn sign_count(&self) -> u16 {
        self.stats.signs
    }
    pub fn pending_host_call_count(&self) -> usize {
        self.stats.pending as usize
    }
}
