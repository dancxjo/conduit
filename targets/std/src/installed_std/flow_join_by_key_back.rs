//! Finite FIFO one-to-one realization of `flow/join/by-key`.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    KeyedJoinSemanticLaw, PlannedGear, PreparedTuplePairEncoder, PreparedTupleTripleEncoder,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::FLOW_JOIN_BY_KEY_IMPLEMENTATION,
    budget,
    prepare,
};

struct PendingSlot {
    bytes: Vec<u8>,
    len: usize,
}

struct PendingSet {
    slots: Vec<PendingSlot>,
    count: usize,
}

impl PendingSet {
    fn new(items: usize, maximum_bytes: usize) -> Self {
        Self {
            slots: (0..items)
                .map(|_| PendingSlot {
                    bytes: vec![0; maximum_bytes],
                    len: 0,
                })
                .collect(),
            count: 0,
        }
    }

    fn is_full(&self) -> bool {
        self.count == self.slots.len()
    }

    fn append(&mut self, bytes: &[u8]) {
        let slot = &mut self.slots[self.count];
        slot.bytes[..bytes.len()].copy_from_slice(bytes);
        slot.len = bytes.len();
        self.count += 1;
    }

    fn find_key(&self, codec: &PreparedTuplePairEncoder, key: &[u8]) -> Result<Option<usize>, ()> {
        for (index, slot) in self.slots[..self.count].iter().enumerate() {
            let (candidate, _) = codec.decode(&slot.bytes[..slot.len]).map_err(|_| ())?;
            if candidate == key {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    fn value<'a>(
        &'a self,
        index: usize,
        codec: &PreparedTuplePairEncoder,
    ) -> Result<(&'a [u8], &'a [u8]), ()> {
        let slot = &self.slots[index];
        codec.decode(&slot.bytes[..slot.len]).map_err(|_| ())
    }

    fn remove(&mut self, index: usize) {
        for next in index + 1..self.count {
            let len = self.slots[next].len;
            let (before, after) = self.slots.split_at_mut(next);
            before[next - 1].bytes[..len].copy_from_slice(&after[0].bytes[..len]);
            before[next - 1].len = len;
        }
        self.count -= 1;
        self.slots[self.count].len = 0;
    }

    fn clear(&mut self) {
        self.count = 0;
        for slot in &mut self.slots {
            slot.len = 0;
        }
    }
}

enum Candidate {
    Pending { side: usize, len: usize },
    Matched { opposite_side: usize, index: usize },
    Discard,
}

pub(super) struct FlowJoinByKeyBack {
    pending: [PendingSet; 2],
    candidate: Vec<u8>,
    candidate_action: Option<Candidate>,
    candidate_closed: Option<usize>,
    closed: [bool; 2],
    codecs: [PreparedTuplePairEncoder; 2],
    output: Box<PreparedTupleTripleEncoder>,
    output_staged: bool,
    terminal: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for FlowJoinByKeyBack {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut contracts = [None; PORTS];
        for (input, contract) in contracts.iter_mut().enumerate().take(2) {
            *contract = Some(AssignedTerminalTransduction {
                input: PortId(input as u16),
                output: PortId(0),
                normal_close: AssignedNormalCloseTransduction::PropagateWhenAllClose,
                abnormal: AssignedAbnormalTransduction::PropagateAfterDrain,
                cancellation: AssignedCancellationTransduction::NotCancellable,
            });
        }
        contracts
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }
        for port in [PortId(0), PortId(1)] {
            if let Some(terminal) = io.input_abnormal(port) {
                io.consume_abnormal(port)
                    .expect("present keyed-join abnormal terminal");
                self.terminal = true;
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal,
                };
            }
        }
        for side in 0..2 {
            let port = PortId(side as u16);
            if self.closed[side] || !io.input_closed(port) {
                continue;
            }
            io.consume_closed(port)
                .expect("present keyed-join normal close");
            if self.closed[1 - side] {
                self.terminal = true;
                return StepOutcome::Complete;
            }
            self.candidate_closed = Some(side);
            return StepOutcome::Progress;
        }
        for side in 0..2 {
            if self.closed[side] {
                continue;
            }
            let port = PortId(side as u16);
            let Some(reference) = io.input(port) else {
                continue;
            };
            let Some(bytes) = inputs.input(port) else {
                return fail(951 + side as u16);
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > self.candidate.len() {
                return fail(953 + side as u16);
            }
            let Ok((key, incoming_value)) = self.codecs[side].decode(bytes) else {
                return fail(955 + side as u16);
            };
            let opposite = 1 - side;
            let match_index = match self.pending[opposite].find_key(&self.codecs[opposite], key) {
                Ok(index) => index,
                Err(()) => return fail(957 + side as u16),
            };
            if let Some(index) = match_index {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                let Ok((matched_key, matched_value)) =
                    self.pending[opposite].value(index, &self.codecs[opposite])
                else {
                    return fail(959 + side as u16);
                };
                if matched_key != key {
                    return fail(961);
                }
                let (left, right) = if side == 0 {
                    (incoming_value, matched_value)
                } else {
                    (matched_value, incoming_value)
                };
                let Ok(encoded) = self.output.encode(key, left, right) else {
                    return fail(962);
                };
                self.output_staged = true;
                self.candidate_action = Some(Candidate::Matched {
                    opposite_side: opposite,
                    index,
                });
                io.consume(port).expect("present keyed-join matching input");
                io.send_prepared(PortId(0), encoded.len() as u32)
                    .expect("ready admitted keyed-join output");
                return StepOutcome::Progress;
            }
            // Once the opposite input has closed and has no matching pending
            // item, this arrival can never match and is explicitly discarded.
            if self.closed[opposite] {
                self.candidate_action = Some(Candidate::Discard);
                io.consume(port)
                    .expect("present impossible keyed-join input");
                return StepOutcome::Progress;
            }
            if self.pending[side].is_full() {
                continue;
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_action = Some(Candidate::Pending {
                side,
                len: bytes.len(),
            });
            io.consume(port).expect("present keyed-join pending input");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.output_staged).then(|| self.output.encoded())
    }

    fn step_committed(&mut self) {
        if let Some(action) = self.candidate_action.take() {
            match action {
                Candidate::Pending { side, len } => {
                    self.pending[side].append(&self.candidate[..len]);
                }
                Candidate::Matched {
                    opposite_side,
                    index,
                } => self.pending[opposite_side].remove(index),
                Candidate::Discard => {}
            }
        }
        if let Some(side) = self.candidate_closed.take() {
            self.closed[side] = true;
            self.pending[1 - side].clear();
        }
        self.output_staged = false;
    }

    fn cancel(&mut self) {
        self.pending[0].clear();
        self.pending[1].clear();
        self.candidate_action = None;
        self.terminal = true;
    }
}

fn join_law(placement: &PlannedGear) -> Result<&KeyedJoinSemanticLaw, String> {
    placement
        .semantic_contract
        .keyed_join()
        .ok_or_else(|| "flow/join/by-key placement has no exact keyed-join law".into())
}

fn validate(placement: &PlannedGear) -> Result<&KeyedJoinSemanticLaw, String> {
    let law = join_law(placement)?;
    let expected = conduit_semantic_catalog::flow_join_by_key_semantic_contract(
        &law.key,
        &law.left_value,
        &law.right_value,
    )
    .map_err(str::to_string)?;
    if [&law.key, &law.left_value, &law.right_value]
        .iter()
        .any(|contract| {
            contract.maximum_bytes > conduit_std_offers::FLOW_JOIN_BY_KEY_MAXIMUM_COMPONENT_BYTES
        })
        || placement.kind_id.as_str() != conduit_semantic_catalog::FLOW_JOIN_BY_KEY_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::FLOW_JOIN_BY_KEY_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::FLOW_JOIN_BY_KEY_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::FLOW_JOIN_BY_KEY_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::FLOW_JOIN_BY_KEY_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err(
            "planned flow/join/by-key identity differs from its exact specialization".into(),
        );
    }
    Ok(law)
}

fn prepared_parts(
    law: &KeyedJoinSemanticLaw,
) -> Result<
    (
        PreparedTuplePairEncoder,
        PreparedTuplePairEncoder,
        PreparedTupleTripleEncoder,
    ),
    String,
> {
    Ok((
        PreparedTuplePairEncoder::new(
            law.key.value_kind.clone(),
            law.key.maximum_bytes,
            law.left_value.value_kind.clone(),
            law.left_value.maximum_bytes,
        )
        .map_err(|error| format!("cannot prepare keyed left tuple: {error:?}"))?,
        PreparedTuplePairEncoder::new(
            law.key.value_kind.clone(),
            law.key.maximum_bytes,
            law.right_value.value_kind.clone(),
            law.right_value.maximum_bytes,
        )
        .map_err(|error| format!("cannot prepare keyed right tuple: {error:?}"))?,
        PreparedTupleTripleEncoder::new(
            law.key.value_kind.clone(),
            law.key.maximum_bytes,
            law.left_value.value_kind.clone(),
            law.left_value.maximum_bytes,
            law.right_value.value_kind.clone(),
            law.right_value.maximum_bytes,
        )
        .map_err(|error| format!("cannot prepare joined tuple: {error:?}"))?,
    ))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let law = validate(placement)?;
    let (left, right, output) = prepared_parts(law)?;
    let pending = u32::from(law.maximum_pending_per_side)
        .checked_mul(
            left.maximum_bytes()
                .checked_add(right.maximum_bytes())
                .ok_or("keyed-join pending item budget overflows")?,
        )
        .ok_or("keyed-join pending byte budget overflows")?;
    Ok(BackBudget {
        value_items: law.maximum_pending_per_side * 2 + 2,
        value_bytes: pending
            .checked_add(left.maximum_bytes().max(right.maximum_bytes()))
            .and_then(|bytes| bytes.checked_add(output.maximum_bytes()))
            .ok_or("keyed-join prepared byte budget overflows")?,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: output.maximum_bytes(),
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let law = validate(placement)?;
    let (left, right, output) = prepared_parts(law)?;
    let pending = usize::from(law.maximum_pending_per_side);
    let candidate = left.maximum_bytes().max(right.maximum_bytes()) as usize;
    Ok(InstalledBack::FlowJoinByKey(Box::new(FlowJoinByKeyBack {
        pending: [
            PendingSet::new(pending, left.maximum_bytes() as usize),
            PendingSet::new(pending, right.maximum_bytes() as usize),
        ],
        candidate: vec![0; candidate],
        candidate_action: None,
        candidate_closed: None,
        closed: [false; 2],
        codecs: [left, right],
        output: Box::new(output),
        output_staged: false,
        terminal: false,
    })))
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(test)]
mod tests;
