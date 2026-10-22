//! Root-side bounds of the already selected ordinary text Host Call contract.
//! Shared with CPU tests; this neither constructs nor replaces a scheduler.
use conduit_kernel::{HostCallBinding, HostCallId, NodeId, scheduler::SchedulerError};

pub(crate) fn validate(
    roles: [NodeId; 3],
    bindings: &[Option<HostCallBinding>; 3],
) -> Result<(), SchedulerError> {
    let roles = roles.map(|node| usize::from(node.0));
    if roles.iter().any(|node| *node >= bindings.len())
        || roles[0] == roles[1]
        || roles[0] == roles[2]
        || roles[1] == roles[2]
    {
        return Err(SchedulerError::InvalidPlan);
    }
    let upper = bindings[roles[1]].ok_or(SchedulerError::InvalidPlan)?;
    let face = bindings[roles[2]].ok_or(SchedulerError::InvalidPlan)?;
    if bindings[roles[0]].is_some()
        || upper.call != HostCallId(0)
        || face.call != HostCallId(0)
        || upper.maximum_input_bytes != conduit_text::MAX_TEXT_BYTES
        || upper.maximum_output_bytes != conduit_text::MAX_TEXT_BYTES
        || face.maximum_input_bytes != conduit_text::MAX_TEXT_BYTES
        || face.maximum_output_bytes != conduit_core::MAX_FACE_COMPLETION_BYTES
    {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
}
