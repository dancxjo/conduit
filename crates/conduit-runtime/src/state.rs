use super::*;

pub(super) struct RuntimePlan {
    pub(super) fragment: PlanFragment,
    pub(super) mandatory_evidence: MandatoryEvidenceLog,
    pub(super) placements: BTreeMap<PlacementId, RuntimePlacement>,
    pub(super) connections: BTreeMap<ConnectionId, RuntimeConnection>,
    pub(super) state: PlanState,
    pub(super) terminal: Option<TerminalDisposition>,
    pub(super) terminal_emitted: bool,
    pub(super) active_play_id: Option<ActivePlayId>,
    pub(super) composite_inputs: BTreeMap<conduit_core::PortId, CompositeInputState>,
    pub(super) composite_outputs: BTreeMap<conduit_core::PortId, CompositeOutputState>,
}

#[derive(Debug)]
pub(super) struct CompositeInputState {
    pub(super) binding: CompositePortBinding,
    pub(super) queue: BoundedQueue<QueuedValue>,
    pub(super) queued_bytes: u32,
    pub(super) next_expected_sequence: u64,
    pub(super) closed: bool,
}

#[derive(Debug)]
pub(super) struct CompositeOutputState {
    pub(super) binding: CompositePortBinding,
    pub(super) queue: BoundedQueue<QueuedValue>,
    pub(super) queued_bytes: u32,
    pub(super) next_send_sequence: u64,
    pub(super) transmission_in_flight: bool,
    pub(super) terminal: Option<TerminalDisposition>,
    pub(super) terminal_emitted: bool,
}

#[derive(Debug)]
pub(super) struct MandatoryEvidenceLog {
    pub(super) recorded_indices: Vec<u16>,
    pub(super) allocated_item_slots: u32,
    pub(super) storage_budget: EvidenceStorageBudget,
    pub(super) used_bytes: u32,
    pub(super) overflowed: bool,
}

impl MandatoryEvidenceLog {
    pub(super) fn new(fragment: &PlanFragment) -> Self {
        let recorded_indices =
            Vec::with_capacity(usize::from(fragment.evidence_storage_budget.item_capacity));
        Self {
            allocated_item_slots: u32::try_from(recorded_indices.capacity()).unwrap_or(u32::MAX),
            recorded_indices,
            storage_budget: fragment.evidence_storage_budget,
            used_bytes: 0,
            overflowed: false,
        }
    }

    pub(super) fn record(&mut self, expected: &[ExpectedEvidence], evidence: ExpectedEvidence) {
        let Some(index) = expected.iter().position(|item| item == &evidence) else {
            self.overflowed = true;
            return;
        };
        let Ok(index) = u16::try_from(index) else {
            self.overflowed = true;
            return;
        };
        if self.recorded_indices.contains(&index) {
            return;
        }
        let Some(charge) = mandatory_evidence_storage_requirement(core::slice::from_ref(&evidence))
        else {
            self.overflowed = true;
            return;
        };
        let Some(used_bytes) = self.used_bytes.checked_add(charge.byte_capacity) else {
            self.overflowed = true;
            return;
        };
        if self.recorded_indices.len() >= usize::from(self.storage_budget.item_capacity)
            || used_bytes > self.storage_budget.byte_capacity
        {
            self.overflowed = true;
            return;
        }
        self.recorded_indices.push(index);
        self.used_bytes = used_bytes;
    }

    pub(super) fn report(&self, plan_id: PlanId, expected: &[ExpectedEvidence]) -> MandatoryEvidenceReport {
        MandatoryEvidenceReport {
            plan_id,
            expected: expected.to_vec(),
            recorded: self
                .recorded_indices
                .iter()
                .map(|index| expected[usize::from(*index)].clone())
                .collect(),
            storage_budget: self.storage_budget,
            allocated_item_slots: self.allocated_item_slots,
            used_bytes: self.used_bytes,
            overflowed: self.overflowed,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum PlanState {
    Prepared,
    Active,
    Failed,
    Cancelled,
    Completed,
}

pub(super) struct RuntimePlacement {
    pub(super) spec: PlannedOperation,
    pub(super) lifecycle: PlacementLifecycleState,
    pub(super) terminal: Option<TerminalDisposition>,
    pub(super) implementation_state: Box<dyn OperationState>,
    pub(super) action: OperationAction,
    pub(super) effect_issued: bool,
    pub(super) pending_input_connection: Option<ConnectionId>,
    pub(super) pending_input_boundary: Option<conduit_core::PortId>,
    pub(super) inputs_closed_notified: bool,
    pub(super) pending_presentation_id: Option<PresentationId>,
    pub(super) next_presentation_sequence: u64,
}

#[derive(Debug)]
pub(super) struct RuntimeConnection {
    pub(super) spec: PlannedConnection,
    pub(super) queue: BoundedQueue<QueuedValue>,
    pub(super) queued_bytes: u32,
    pub(super) source_done: bool,
    pub(super) sink_failed: bool,
    pub(super) blocked: bool,
    pub(super) last_accepted_sequence: Option<u64>,
    pub(super) last_manifested_sequence: Option<u64>,
    pub(super) terminal: Option<ConnectionTerminalDisposition>,
    pub(super) role: ConnectionRole,
    pub(super) transmission_in_flight: bool,
    pub(super) next_expected_sequence: u64,
    pub(super) next_send_sequence: u64,
    pub(super) accepted_remote_sequences: BTreeSet<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QueuedValue {
    pub(super) sequence: u64,
    pub(super) value: ValuePayload,
}

pub(super) struct PresentationCompletion {
    pub(super) active_play_id: ActivePlayId,
    pub(super) presentation_id: PresentationId,
    pub(super) placement_id: PlacementId,
    pub(super) value: ValuePayload,
    pub(super) success: bool,
    pub(super) message: Option<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum ConnectionRole {
    Local,
    Outbound,
    Inbound,
}

