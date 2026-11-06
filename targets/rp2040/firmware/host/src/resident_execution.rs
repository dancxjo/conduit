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
    // Validate the entire release set before modifying the existing ledger.
    for fragment in terminal.iter().flatten() {
        let m = fragment.approved.metadata();
        for resource in conduit_body::PlainFragmentResources::new(m.resource_bindings) {
            let Ok(resource) = resource else {
                return false;
            };
            if !owner.admissions().iter().any(|a| {
                a.plan_id.as_str() == m.body_plan && a.placement_id.as_str() == resource.placement
            }) {
                return false;
            }
        }
    }
    for fragment in terminal.iter().flatten() {
        let m = fragment.approved.metadata();
        let entries = || conduit_body::PlainFragmentResources::new(m.resource_bindings);
        for (index, resource) in entries().enumerate() {
            let resource = resource.expect("preflight checked immutable resource keys");
            if entries()
                .take(index)
                .any(|prior| prior.is_ok_and(|p| p.placement == resource.placement))
            {
                continue;
            }
            if owner
                .release_borrowed(m.body_plan, resource.placement, cause)
                .is_err()
            {
                return false;
            }
        }
    }
    true
}
