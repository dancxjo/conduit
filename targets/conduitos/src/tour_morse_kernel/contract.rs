//! Exact Root-side Host Call bounds of the already selected Tour Morse Plan.
//! Shared with CPU tests; no scheduler, effect grant or resource bound is added.
use conduit_kernel::{HostCallBinding, HostCallId, NodeId, scheduler::SchedulerError};

pub(crate) fn validate(
    roles: [NodeId; 5],
    bindings: &[Option<HostCallBinding>; 5],
) -> Result<(), SchedulerError> {
    let roles = roles.map(|node| usize::from(node.0));
    if roles.iter().any(|node| *node >= bindings.len())
        || roles
            .iter()
            .enumerate()
            .any(|(index, node)| roles[..index].contains(node))
        || bindings[roles[0]].is_some()
    {
        return Err(SchedulerError::InvalidPlan);
    }
    for (node, input, output) in [
        (
            roles[1],
            conduit_text::MAX_TEXT_BYTES,
            conduit_text::MAX_TEXT_BYTES,
        ),
        (
            roles[2],
            conduit_text::MAX_TEXT_BYTES,
            conduit_core::MAX_FACE_COMPLETION_BYTES,
        ),
        (
            roles[3],
            conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32,
            conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        ),
        // The existing indicator contract has no completion payload. It is
        // distinct from the canonical text Face presentation contract above.
        (
            roles[4],
            conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
            0,
        ),
    ] {
        let binding = bindings[node].ok_or(SchedulerError::InvalidPlan)?;
        if binding.call != HostCallId(0)
            || binding.maximum_input_bytes != input
            || binding.maximum_output_bytes != output
        {
            return Err(SchedulerError::InvalidPlan);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
