//! Explicit commit, bounded ordinary-kernel drive and terminal resource release.
use super::*;
impl StdHost {
    fn require_delegated_current(&self, ticket: &PreparedDelegatedBody) -> Result<(), String> {
        if ticket.released
            || ticket.receipt.host_id != self.advertisement.host_id
            || ticket.receipt.boot_id != self.advertisement.boot_id
            || ticket.receipt.offer_generation != self.advertisement.offer_generation
        {
            return Err("delegated preparation is not this current Host incarnation".into());
        }
        self.kernel_resources
            .require_input_memory_owner(&ticket.memory)
    }
    pub fn commit_delegated_startup(
        &mut self,
        ticket: &mut PreparedDelegatedBody,
        startup: &CommittedBodyStartup,
    ) -> Result<(), String> {
        self.require_delegated_current(ticket)?;
        if ticket.committed || ticket.finished || ticket.cancelled {
            return Err("delegated preparation is not awaiting commit".into());
        }
        if self.next_kernel_play_sequence != ticket.receipt.member_play.play_sequence {
            return Err("delegated member Play sequence changed before commit".into());
        }
        let witness = startup.for_prepared_scope(&ticket.scope)?;
        let next = self
            .next_kernel_play_sequence
            .checked_add(1)
            .ok_or("member Play sequence exhausted")?;
        ticket
            .kernel
            .as_ref()
            .ok_or("delegated actual kernel missing")?
            .bind_startup(Some(witness))?;
        self.next_kernel_play_sequence = next;
        ticket.committed = true;
        Ok(())
    }
    /// One iteration of the shared ordinary kernel. Pending deadlines are
    /// observed without sleeping, so an actor can accept cancellation between
    /// polls. Each admitted Host Call still owns its finite effect deadline.
    pub fn poll_delegated_body<W: Write, T: TimerAdapter>(
        &mut self,
        ticket: &mut PreparedDelegatedBody,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
    ) -> Result<DelegatedBodyProgress, String> {
        self.poll_delegated_body_with_cancel(ticket, output, timer, control, &|| false)
    }
    /// The callback observes the existing exact authenticated cancellation
    /// signal while a bounded platform effect is pending. It cannot acknowledge
    /// terminal or supply a second execution outcome.
    pub fn poll_delegated_body_with_cancel<W: Write, T: TimerAdapter>(
        &mut self,
        ticket: &mut PreparedDelegatedBody,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
        external_cancel: &dyn Fn() -> bool,
    ) -> Result<DelegatedBodyProgress, String> {
        self.require_delegated_current(ticket)?;
        if ticket.finished {
            return Ok(DelegatedBodyProgress::Terminal);
        }
        if control.stop_requested() || external_cancel() {
            ticket.cancelled = true;
        }
        if !ticket.committed && !ticket.cancelled {
            return Ok(DelegatedBodyProgress::Prepared);
        }
        let outcome = ticket
            .kernel
            .as_mut()
            .ok_or("delegated actual kernel missing")?
            .drive(
                ticket
                    .execution
                    .as_mut()
                    .ok_or("delegated execution state missing")?,
                output,
                timer,
                &mut None,
                control,
                &self.advertisement.host_id,
                &self.advertisement.boot_id,
                None,
                None,
                &mut None,
                &mut None,
                false,
                ticket.cancelled,
                external_cancel,
            );
        let terminal = match outcome {
            Ok(None) => return Ok(DelegatedBodyProgress::Running),
            Ok(Some(value)) => Ok(value),
            Err(error) => Err(error),
        };
        let kernel = ticket
            .kernel
            .take()
            .ok_or("delegated actual kernel missing")?;
        let state = ticket
            .execution
            .take()
            .ok_or("delegated execution state missing")?;
        let result = kernel.finish(
            state,
            terminal,
            timer,
            &self.advertisement.host_id,
            &self.advertisement.boot_id,
            None,
            None,
            None,
        );
        ticket.terminal.terminal = result.terminal;
        ticket.terminal.failure = result.failure;
        ticket.terminal.cleanup_failure = result.cleanup_failure;
        ticket.terminal.kernel_failure = result.kernel_failure.map(Into::into);
        ticket.terminal.kernel_signs = result.events.len();
        ticket.terminal.startup_chimes = result.startup_chimes;
        // Other result backing drops here; original receipt/diagnostics remain
        // charged conservatively until explicit release, including UNKNOWN.
        ticket.finished = true;
        Ok(DelegatedBodyProgress::Terminal)
    }
    pub fn cancel_delegated_body(
        &mut self,
        ticket: &mut PreparedDelegatedBody,
    ) -> Result<(), String> {
        self.require_delegated_current(ticket)?;
        if !ticket.finished {
            ticket.cancelled = true;
        }
        Ok(())
    }
    /// A refusal keeps the caller's ticket and every remaining reservation.
    /// The actor drops its Option only after this method returns success.
    pub fn release_delegated_body(
        &mut self,
        ticket: &mut PreparedDelegatedBody,
    ) -> Result<(), String> {
        self.require_delegated_current(ticket)?;
        if !ticket.finished {
            return Err("delegated actual kernel is not terminal".into());
        }
        if ticket.terminal.cleanup_failure.is_some() {
            return Err(
                "delegated platform cleanup is uncertain; retain preparation custody".into(),
            );
        }
        while let Some(reservation) = ticket.reservations.last() {
            self.kernel_resources.release_borrowed(reservation)?;
            ticket.reservations.pop();
        }
        ticket.released = true;
        Ok(())
    }
}
