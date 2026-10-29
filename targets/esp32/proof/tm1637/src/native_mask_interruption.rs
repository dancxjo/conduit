//! Explicit caller cancellation settlement and reuse of the exact empty slot.
//! This path never sets completed_retirement or passes normal completion gates.
use super::*;
impl Handle {
    pub fn retire_interrupted(self) -> Result<(Self, InterruptedMask), (Self, &'static str)> {
        match self.take_interrupted() {
            Ok(retired) => Ok((self, retired)),
            Err(reason) => Err((self, reason)),
        }
    }
    fn take_interrupted(&self) -> Result<InterruptedMask, &'static str> {
        if !WORKSPACE.idle() {
            return Err("mask-workspace-held");
        }
        let status = self.status();
        if status.task_active
            || status.phase != "quiesced"
            || !status.activated
            || status.completed == 0
            || status.completed >= super::super::worker::MAXIMUM_CHILDREN
            || status.pending_evidence
            || status.failure.is_some()
            || status.cleanup_refusal.is_some()
            || !STATE.lock(|state| state.borrow().cancelled)
        {
            return Err("mask-task-not-interrupted");
        }
        let mut session = self
            .slot
            .session
            .try_borrow_mut()
            .map_err(|_| "mask-task-session-busy")?;
        let mut leases = self
            .slot
            .leases
            .try_borrow_mut()
            .map_err(|_| "mask-task-issuer-busy")?;
        let mut evidence = self
            .slot
            .evidence
            .try_borrow_mut()
            .map_err(|_| "mask-task-evidence-busy")?;
        let mut guard = self
            .slot
            .revoke
            .try_borrow_mut()
            .map_err(|_| "mask-task-guard-busy")?;
        if guard.is_none() {
            return Err("mask-task-guard");
        }
        session
            .as_ref()
            .ok_or("mask-task-session")?
            .interruption_ready(
                leases.as_ref().ok_or("mask-task-issuer")?,
                evidence.as_ref().ok_or("mask-task-evidence")?,
            )?;
        let result = session
            .take()
            .expect("preflighted session")
            .retire_interrupted(
                leases.take().expect("preflighted issuer"),
                evidence.take().expect("preflighted evidence"),
            );
        match result {
            Ok(retired) => {
                guard.take();
                self.slot.completed_retirement.set(false);
                update(|state| {
                    state.status.phase = "interrupted-retired";
                    state.status.context_bytes = 0;
                });
                Ok(retired)
            }
            Err((returned_session, returned_leases, returned_evidence, reason)) => {
                *session = Some(returned_session);
                *leases = Some(returned_leases);
                *evidence = Some(returned_evidence);
                Err(reason)
            }
        }
    }
    /// The interrupted slot is empty only after its separate consuming owner
    /// transfer. Fresh preparation/activation/Body start still apply unchanged.
    pub fn prepare_after_interruption(
        self,
        session: &mut Option<Box<NativeMaskSession>>,
        leases: &mut Option<Box<MaskLeaseIssuer>>,
        evidence: &mut Option<Box<Consumer>>,
        spawner: Spawner,
    ) -> Result<Self, (Self, &'static str)> {
        let status = self.status();
        if !WORKSPACE.idle()
            || status.phase != "interrupted-retired"
            || status.task_active
            || self.slot.completed_retirement.get()
            || self.slot.session.borrow().is_some()
            || self.slot.leases.borrow().is_some()
            || self.slot.evidence.borrow().is_some()
            || self.slot.revoke.borrow().is_some()
        {
            return Err((self, "mask-task-slot-not-interrupted"));
        }
        match register(self.slot, session, leases, evidence, spawner) {
            Ok(next) => Ok(next),
            Err(reason) => {
                self.slot.completed_retirement.set(false);
                update(|state| state.status.phase = "interrupted-retired");
                Err((self, reason))
            }
        }
    }
}
