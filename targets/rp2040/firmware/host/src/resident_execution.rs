//! Root supervision over the one production kernel and original reservation set.
use super::*;
use crate::resident::generated::ENTRY_COUNT;
use conduit_kernel::scheduler::{SchedulerError, SchedulerStatus};

pub struct TerminalFragment<'a> {
    pub approved: DelegatedBodyFragment<'a>,
    pub retired: RetiredKernel,
}

/// Root calls only after current explicit execution authorization and complete
/// admission. No transport, ingress, time renewal or capability is created here.
pub fn step_workset(
    fragments: &mut [Option<BoundFragment<'_, '_>>; ENTRY_COUNT],
) -> Result<bool, SchedulerError> {
    let mut drained = true;
    for fragment in fragments.iter_mut().flatten() {
        drained &= matches!(
            fragment.kernel.step()?,
            SchedulerStatus::Drained | SchedulerStatus::Cancelled
        );
    }
    Ok(drained)
}

/// Keep all reservations until every kernel's evidence and Back return are
/// retained. Partial refusal preserves remaining kernels and all reservations.
/// Terminal storage is caller-funded and must be empty before the first call.
pub fn retire_workset<'a, 'o>(
    fragments: &mut [Option<BoundFragment<'a, 'o>>; ENTRY_COUNT],
    terminal: &mut [Option<TerminalFragment<'a>>; ENTRY_COUNT],
    owner: &mut c::ResourceAdmissionOwner,
    cause: c::ResourceReleaseCause,
) -> bool {
    for (fragment, retained) in fragments.iter_mut().zip(terminal.iter_mut()) {
        if fragment.is_some() && retained.is_some() {
            return false;
        }
        let Some(BoundFragment { approved, kernel }) = fragment.take() else {
            continue;
        };
        match kernel.cancel_and_retire() {
            Ok(retired) => *retained = Some(TerminalFragment { approved, retired }),
            Err((kernel, _)) => {
                *fragment = Some(BoundFragment { approved, kernel });
                return false;
            }
        }
    }
    if terminal
        .iter()
        .flatten()
        .any(|f| !f.retired.providers_returned)
    {
        return false;
    }
    let fragments = terminal.each_ref().map(|f| f.as_ref().map(|f| &f.approved));
    crate::retired_resources::release(&fragments, owner, cause)
}
