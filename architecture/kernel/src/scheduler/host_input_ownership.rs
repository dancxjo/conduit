//! Exact ownership transfer from consumed references to admitted Host input.
use super::*;

impl<
        D,
        S,
        E,
        const NODES: usize,
        const CORDS: usize,
        const PORTS: usize,
        const QUEUE_SLOTS: usize,
        const ROUTE_SLOTS: usize,
        const ROUTE_TARGETS: usize,
        const HOST_BINDING_SLOTS: usize,
        const PENDING_REQUESTS: usize,
    >
    FixedScheduler<
        D,
        S,
        E,
        NODES,
        CORDS,
        PORTS,
        QUEUE_SLOTS,
        ROUTE_SLOTS,
        ROUTE_TARGETS,
        HOST_BINDING_SLOTS,
        PENDING_REQUESTS,
    >
where
    D: StepOperation<PORTS>,
    S: ValueStorage,
    E: SignSink,
{
    pub(super) fn preflight_host_input(
        &self,
        node: usize,
        staged: &StagedStep<PORTS>,
        retained_values: &[Option<ValueRef>; PORTS],
        available_host_value: Option<ValueRef>,
        consumed_host_value: Option<ValueRef>,
    ) -> Result<(), SchedulerError> {
        let host_request = staged.host_request;
        let consumed_host_completion = staged.consumed_host_completion;
        let discards = &staged.discards;
        let outputs = &staged.outputs;
        let consumed = &staged.consumed;
        if let Some((_, _, input)) = host_request {
            let value = input.value;
            if available_host_value == Some(value) && !consumed_host_completion {
                return Err(SchedulerError::InvalidHostOperationAccess);
            }
            if discards.iter().flatten().any(|discard| *discard == value)
                || retained_values
                    .iter()
                    .flatten()
                    .any(|retained| *retained == value)
            {
                return Err(SchedulerError::InvalidHostOperationAccess);
            }
            if !outputs.iter().flatten().any(|output| *output == value) {
                let consumed_references = consumed
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(port, is_consumed)| {
                        *is_consumed
                            && self.node_specs[node].input_cords[*port]
                                .and_then(|cord| self.peek(usize::from(cord.0)).ok().flatten())
                                == Some(value)
                    })
                    .count()
                    + usize::from(consumed_host_value == Some(value));
                let input_matches = self.node_specs[node]
                    .input_cords
                    .iter()
                    .flatten()
                    .filter(|cord| self.peek(usize::from(cord.0)).ok().flatten() == Some(value))
                    .count();
                // A consumed reference transfers to the pending request. Other
                // aliases stay owned by their queues; they need not be consumed.
                if input_matches > 0 && consumed_references == 0 {
                    return Err(SchedulerError::InvalidHostOperationAccess);
                }
                let current = usize::from(self.values.reference_count(value)?);
                if current < consumed_references {
                    return Err(SchedulerError::Storage(StorageError::StaleReference));
                }
            }
        }
        Ok(())
    }
}
