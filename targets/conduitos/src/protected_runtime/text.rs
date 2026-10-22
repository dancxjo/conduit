//! Ordinary text uses the same broker with its independent service lease.
use super::*;
use conduitos_runtime_core::text as canonical;
impl Scheduler {
    pub fn new_text(admission: canonical::Admission) -> Result<Self, SchedulerError> {
        diagnose(
            "text-contract",
            crate::text_planned_kernel::contract::validate(
                [
                    admission.literal_node,
                    admission.upper_node,
                    admission.face_node,
                ],
                &admission.bindings,
            ),
        )?;
        let roles = [
            usize::from(admission.literal_node.0),
            usize::from(admission.upper_node.0),
            usize::from(admission.face_node.0),
        ];
        let ledger = diagnose("text-ledger", Ledger::new_text(&admission))?;
        #[cfg(conduitos_runtime_image)]
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.elf"));
        #[cfg(not(conduitos_runtime_image))]
        let bytes: &[u8] = &[];
        let image = diagnose(
            "text-image",
            ServiceImage::for_profile(bytes, ServiceProfile::OrdinaryRuntime),
        )
        .map_err(|_| SchedulerError::InvalidPlan)?;
        let mut domain = diagnose("text-install", ServiceDomain::install(&image))
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let window = diagnose("text-transfer", domain.transfer_window())
            .map_err(|_| SchedulerError::InvalidPlan)?;
        window.fill(0);
        let length = diagnose(
            "text-encode",
            wire::text_admission::encode(&admission, window),
        )
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
        if diagnose(
            "text-init-request",
            result.request(Request::InitText {
                bytes: length as u32,
            }),
        )? != Response::Ready
        {
            return result.fail(SchedulerError::InvalidPlan);
        }
        Ok(result)
    }
    pub(super) fn refresh_stats(
        &mut self,
        request: Request,
        response: Response,
    ) -> Result<(), SchedulerError> {
        let Response::Stats(stats) = diagnose("text-stats", self.exchange(Request::Stats))? else {
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
            return diagnose("text-stats-counters", Err(SchedulerError::InvalidPlan));
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
