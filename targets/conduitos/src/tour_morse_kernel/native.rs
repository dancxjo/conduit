//! Root checked Plan → numeric admission; no local scheduler construction.
use super::*;
use conduitos_runtime_core::morse as canonical;
const _: () = assert!(PORTS == conduitos_runtime_core::PORTS);
const _: () = assert!(conduit_text::MAXIMUM_MORSE_INPUT_BYTES == canonical::MAX_INPUT_BYTES);
const _: () = assert!(conduit_text::MAXIMUM_MORSE_PATTERN_BYTES == canonical::MAX_VALUE_BYTES);
pub(super) fn prepare(
    lowered: &LoweredPlanFragment,
    literal: &str,
    roles: [usize; canonical::NODES],
) -> Result<Scheduler, SchedulerError> {
    let mut admission = canonical::Admission::new(roles.map(|node| NodeId(node as u16)));
    admission.nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| SchedulerError::InvalidPlan)?;
    admission.cords = [
        lowered.cords[0].spec,
        lowered.cords[1].spec,
        lowered.cords[2].spec,
        lowered.cords[3].spec,
    ];
    admission
        .literal
        .get_mut(..literal.len())
        .ok_or(SchedulerError::InvalidPlan)?
        .copy_from_slice(literal.as_bytes());
    admission.literal_bytes = literal.len() as u16;
    admission.sign_bytes = lowered.sign_bytes;
    for route in &lowered.routes {
        let slot = route.source_node.0 as usize * PORTS + route.source_port.0 as usize;
        let range = route.range;
        let end = (range.start as usize)
            .checked_add(range.len as usize)
            .ok_or(SchedulerError::InvalidPlan)?;
        let output = admission
            .targets
            .get_mut(range.start as usize..end)
            .ok_or(SchedulerError::InvalidPlan)?;
        if slot >= canonical::ROUTES
            || admission.ranges[slot].is_some()
            || output.iter().any(Option::is_some)
            || output.len() != route.targets.len()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        admission.ranges[slot] = Some(range);
        for (target, source) in output.iter_mut().zip(&route.targets) {
            *target = Some(*source);
        }
    }
    for operation in &lowered.host_calls {
        let output = admission
            .bindings
            .get_mut(operation.node.0 as usize)
            .ok_or(SchedulerError::InvalidPlan)?;
        if output.is_some() {
            return Err(SchedulerError::InvalidPlan);
        }
        *output = Some(operation.binding);
    }
    Scheduler::new_morse(admission)
}
