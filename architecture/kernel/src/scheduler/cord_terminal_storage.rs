//! Full terminal values retained only where the checked topology can write them.
use super::*;

fn is_ingress(cord: &CordSpec) -> bool {
    matches!(
        (cord.source, cord.sink),
        (CordEndpoint::Remote(_), CordEndpoint::Local { .. })
    )
}

/// Default storage is Cord-indexed. Compact storage has one slot for every
/// logical ingress, including local Fore, and no routed abnormal terminals.
pub(super) fn validate_capacity<const STOCK: usize, const CORDS: usize>(
    active: &[CordSpec],
) -> Result<(), SchedulerError> {
    if STOCK == CORDS {
        return Ok(());
    }
    if active
        .iter()
        .any(|cord| cord.track == AssignedConnectionTrack::AbnormalTerminal)
        || active.iter().filter(|cord| is_ingress(cord)).count() != STOCK
    {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
}

impl<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
        const TERMINAL_ROWS: usize,
        const PHASE_BYTES: usize,
        const UNRESOLVED_NODES: usize,
        const CORD_TERMINALS: usize,
    >
    FixedScheduler<
        D,
        S,
        E,
        N,
        C,
        P,
        Q,
        R,
        T,
        H,
        A,
        TERMINAL_ROWS,
        PHASE_BYTES,
        UNRESOLVED_NODES,
        CORD_TERMINALS,
    >
where
    D: StepBack<P>,
    S: ValueStorage,
    E: SignSink,
{
    fn cord_terminal_slot(&self, cord: usize) -> Result<Option<usize>, SchedulerError> {
        if cord >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        if CORD_TERMINALS == C {
            return Ok(Some(cord));
        }
        if !is_ingress(&self.cord_specs[cord]) {
            return Ok(None);
        }
        let slot = self.cord_specs[..cord]
            .iter()
            .filter(|spec| is_ingress(spec))
            .count();
        if slot >= CORD_TERMINALS {
            return Err(SchedulerError::InvalidPlan);
        }
        Ok(Some(slot))
    }

    pub(super) fn cord_terminal(
        &self,
        cord: usize,
    ) -> Result<Option<CanonicalValue>, SchedulerError> {
        Ok(match self.cord_terminal_slot(cord)? {
            Some(slot) => self.cord_terminals[slot],
            None => None,
        })
    }

    pub(super) fn set_cord_terminal(
        &mut self,
        cord: usize,
        terminal: Option<CanonicalValue>,
    ) -> Result<(), SchedulerError> {
        match self.cord_terminal_slot(cord)? {
            Some(slot) => self.cord_terminals[slot] = terminal,
            None if terminal.is_none() => {}
            None => return Err(SchedulerError::InvalidPlan),
        }
        Ok(())
    }
}
