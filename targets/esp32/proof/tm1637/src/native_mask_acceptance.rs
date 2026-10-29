//! Exact accepted terminal evidence precedes releasing retained presentation owners.
use super::*;
impl NativeMaskSession {
    pub fn accept_pending(
        &mut self,
        leases: &mut impl LeaseSource,
        evidence: &mut impl EvidenceSink,
    ) -> Result<(), &'static str> {
        let (active, value) = self.pending.as_ref().ok_or("mask-evidence-unavailable")?;
        if let Some(handoff) = self
            .handoff
            .as_mut()
            .filter(|state| state.receipt.is_some())
        {
            let receipt = handoff.receipt.as_ref().ok_or("mask-replacement-receipt")?;
            if receipt.active() != active {
                return Err("mask-replacement-evidence-scope");
            }
            evidence.accept_replaced(
                self.index,
                active,
                self.session
                    .kernel()
                    .host_value(*value)
                    .map_err(|_| "mask-replaced-value")?,
                handoff.previous_log.as_ref().ok_or("mask-replaced-log")?,
            )?;
            let call = self.call.as_ref().ok_or("mask-call")?;
            leases.replaced(
                call.retained_identity(),
                active,
                receipt.inspection(),
                call.inspection(),
            )?;
            self.session
                .discard_host_value(*value)
                .map_err(|_| "mask-replaced-release")?;
            handoff.receipt = None;
            handoff.previous_log = None;
            handoff.previous = None;
            self.pending = None;
            self.index = self
                .index
                .checked_add(1)
                .ok_or("mask-invocation-exhausted")?;
            // A retry may acknowledge old evidence after fail-closed withdrawal
            // of the successor. That does not revive an Available invocation.
            self.faulted = call.state() != r::RendererState::Available;
            return Ok(());
        }
        let inspection = self.call.as_ref().ok_or("mask-call")?.inspection().clone();
        if inspection.completed_operations != 2
            || inspection.in_flight_operations != 0
            || inspection.lifecycle != c::CapabilityLifecycle::Revoked
        {
            return Err("mask-child-retirement-budget");
        }
        evidence.accept(
            self.index,
            active,
            self.session
                .kernel()
                .host_value(*value)
                .map_err(|_| "mask-closed-value")?,
            self.session.kernel().signs(),
        )?;
        self.ready = Some(self.invocation.retire_invocation(&mut self.call)?);
        leases.retired(
            self.ready.as_ref().ok_or("mask-owner")?,
            active,
            &inspection,
        )?;
        self.session
            .discard_host_value(*value)
            .map_err(|_| "mask-closed-release")?;
        if self.index + 1 < MAXIMUM_CHILDREN
            && leases.current(embassy_time::Instant::now().as_millis())
        {
            let retired = self
                .session
                .begin_next([(); 3], Self::fresh_log()?)
                .map_err(|_| "mask-rearm")?;
            // The exact log was consumed above. Transfer, then release this owned
            // old log; neither cancellation nor unacknowledged output reaches here.
            // End the drained invocation borrow before rearming the same session.
            #[allow(clippy::drop_non_drop)]
            drop(retired);
        }
        // The final accepted child stays normally drained. A future consuming
        // retirement may return its storage; no unused successor is rearmed.
        if let Some(handoff) = self.handoff.as_mut() {
            handoff.presentation = None;
        }
        self.pending = None;
        self.index = self
            .index
            .checked_add(1)
            .ok_or("mask-invocation-exhausted")?;
        self.faulted = false;
        self.cleanup_refusal = None;
        Ok(())
    }
}
