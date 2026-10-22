//! Root-issued input loans at the Host Call boundary, not a semantic scheduler.
//! Guest value references are opaque: only exact Root-completed bytes may
//! authorize a later Root effect along its reviewed numeric route.
use conduit_kernel::{
    scheduler::{HostCallRequest, SchedulerError},
    *,
};
use conduitos_runtime_core::{
    admission::{Admission, BackKind},
    *,
};
#[derive(Clone, Copy)]
struct Loan<const BYTES: usize> {
    value: Option<ValueRef>,
    len: usize,
    bytes: [u8; BYTES],
}
impl<const BYTES: usize> Loan<BYTES> {
    fn new(value: Option<ValueRef>, bytes: &[u8]) -> Result<Self, SchedulerError> {
        if bytes.len() > BYTES {
            return Err(SchedulerError::HostCallOutputExceeded);
        }
        let mut result = Self {
            value,
            len: bytes.len(),
            bytes: [0; BYTES],
        };
        result.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(result)
    }
    fn matches(&self, value: ValueRef, bytes: &[u8]) -> bool {
        self.value.is_none_or(|expected| expected == value)
            && value.byte_len as usize == self.len
            && bytes == &self.bytes[..self.len]
    }
}
pub(super) type Ledger = CustodyLedger<VALUE_BYTES>;
pub(super) struct CustodyLedger<const BYTES: usize> {
    kinds: [BackKind; NODES],
    incoming: [Option<u16>; NODES],
    outgoing: [u16; NODES],
    start: [u16; CORDS],
    capacity: [u16; CORDS],
    head: [u16; CORDS],
    len: [u16; CORDS],
    loans: [Option<Loan<BYTES>>; CORDS * 2],
    initial: [bool; NODES],
    ingress: [Option<RemoteEndpointId>; CORDS],
    ingress_limit: [u32; CORDS],
    ingress_sequence: [u64; CORDS],
}
impl<const BYTES: usize> CustodyLedger<BYTES> {
    pub fn new(admission: &Admission) -> Result<Self, SchedulerError> {
        if admission.active_nodes == 0
            || usize::from(admission.active_nodes) > NODES
            || usize::from(admission.active_cords) > CORDS
        {
            return Err(SchedulerError::InvalidActiveCapacity);
        }
        let mut result = Self {
            kinds: admission.backs,
            incoming: [None; NODES],
            outgoing: [0; NODES],
            start: [0; CORDS],
            capacity: [0; CORDS],
            head: [0; CORDS],
            len: [0; CORDS],
            loans: [None; CORDS * 2],
            initial: [false; NODES],
            ingress: [None; CORDS],
            ingress_limit: [0; CORDS],
            ingress_sequence: [0; CORDS],
        };
        let mut occupied = [false; CORDS];
        let mut loan_start = 0;
        for (index, node) in admission
            .nodes
            .iter()
            .enumerate()
            .take(usize::from(admission.active_nodes))
        {
            result.incoming[index] = node.input_cords[0].map(|c| c.0);
            result.initial[index] = admission.backs[index] == BackKind::Application;
        }
        for (index, cord) in admission
            .cords
            .iter()
            .enumerate()
            .take(usize::from(admission.active_cords))
        {
            if cord.cord.0 as usize != index
                || cord.item_capacity == 0
                || cord.pressure_policy
                    != conduit_kernel::scheduler::AssignedPressurePolicy::PreserveOrder
                || cord.track != conduit_kernel::scheduler::AssignedConnectionTrack::Payload
            {
                return Err(SchedulerError::InvalidPlan);
            }
            if let CordEndpoint::Remote(endpoint) = cord.source {
                // Legacy text/event brokers do not gain a remote input route.
                if BYTES != FACE_VALUE_BYTES || !matches!(cord.sink, CordEndpoint::Local { .. }) {
                    return Err(SchedulerError::InvalidPlan);
                }
                result.ingress[index] = Some(endpoint);
                result.ingress_limit[index] = cord
                    .maximum_value_bytes
                    .min(cord.byte_capacity)
                    .min(BYTES as u32);
            }
            let end = usize::from(cord.slot_start)
                .checked_add(usize::from(cord.item_capacity))
                .ok_or(SchedulerError::InvalidPlan)?;
            for slot in occupied
                .get_mut(usize::from(cord.slot_start)..end)
                .ok_or(SchedulerError::InvalidPlan)?
            {
                if *slot {
                    return Err(SchedulerError::InvalidPlan);
                }
                *slot = true;
            }
            // A value may already have left its Cord for an undispatched
            // Host Call. Keep that outstanding effect loan beside the full
            // admitted queue; this does not choose runnable work or route it.
            result.start[index] = loan_start;
            result.capacity[index] = cord.item_capacity + 1;
            loan_start = loan_start
                .checked_add(result.capacity[index])
                .ok_or(SchedulerError::InvalidPlan)?;
            if usize::from(loan_start) > CORDS * 2 {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for (slot, range) in admission.ranges.iter().enumerate() {
            let Some(range) = range else {
                continue;
            };
            if slot % PORTS != 0 {
                return Err(SchedulerError::InvalidPlan);
            }
            let end = usize::from(range.start)
                .checked_add(usize::from(range.len))
                .ok_or(SchedulerError::InvalidPlan)?;
            for target in admission
                .targets
                .get(usize::from(range.start)..end)
                .ok_or(SchedulerError::InvalidPlan)?
            {
                let target = target.ok_or(SchedulerError::InvalidPlan)?;
                let cord = usize::from(target.cord.0);
                if cord >= usize::from(admission.active_cords)
                    || target.sink != admission.cords[cord].sink
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                let CordEndpoint::Local { node, port } = target.sink else {
                    return Err(SchedulerError::InvalidPlan);
                };
                if port.0 != 0
                    || usize::from(node.0) >= usize::from(admission.active_nodes)
                    || result.incoming[usize::from(node.0)] != Some(target.cord.0)
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                result.outgoing[slot / PORTS] |= 1 << cord;
            }
        }
        // Initialize only Root-known bytes on each Back's reviewed output
        // routes. Opaque guest references cannot expand or replenish this loan.
        // Document's eight bytes are revision zero plus empty UTF-8 text; an
        // empty legacy Edit value is not a canonical document or edit.
        for node in 0..usize::from(admission.active_nodes) {
            match admission.backs[node] {
                BackKind::Edit => result.publish(node, None, &[])?,
                BackKind::Document => result.publish(node, None, &[0; 8])?,
                _ => {}
            }
        }
        Ok(result)
    }
    pub fn new_text(
        admission: &conduitos_runtime_core::text::Admission,
    ) -> Result<Self, SchedulerError> {
        let mut numeric = Admission::new(3, 2);
        numeric.nodes[..3].copy_from_slice(&admission.nodes);
        numeric.cords[..2].copy_from_slice(&admission.cords);
        numeric.ranges[..conduitos_runtime_core::text::ROUTES].copy_from_slice(&admission.ranges);
        numeric.targets[..2].copy_from_slice(&admission.targets);
        numeric.bindings[..3].copy_from_slice(&admission.bindings);
        let mut ledger = Self::new(&numeric)?;
        let literal = admission
            .literal
            .get(..admission.literal_bytes as usize)
            .ok_or(SchedulerError::InvalidPlan)?;
        // One startup loan, exact Root-checked literal bytes, on reviewed routes.
        // No guest-issued value reference can manufacture a second startup loan.
        ledger.publish(admission.literal_node.0 as usize, None, literal)?;
        Ok(ledger)
    }
    pub fn new_morse(
        admission: &conduitos_runtime_core::morse::Admission,
    ) -> Result<Self, SchedulerError> {
        let mut numeric = Admission::new(5, 4);
        numeric.nodes[..5].copy_from_slice(&admission.nodes);
        numeric.cords[..4].copy_from_slice(&admission.cords);
        numeric.ranges[..conduitos_runtime_core::morse::ROUTES].copy_from_slice(&admission.ranges);
        numeric.targets[..4].copy_from_slice(&admission.targets);
        numeric.bindings[..5].copy_from_slice(&admission.bindings);
        let mut ledger = Self::new(&numeric)?;
        let literal = admission
            .literal
            .get(..admission.literal_bytes as usize)
            .ok_or(SchedulerError::InvalidPlan)?;
        ledger.publish(admission.roles[0].0 as usize, None, literal)?;
        Ok(ledger)
    }
    pub fn accept(&mut self, request: HostCallRequest, bytes: &[u8]) -> Result<(), SchedulerError> {
        let node = usize::from(request.node.0);
        if self.kinds[node] == BackKind::Keyboard {
            return if bytes.is_empty()
                && request.input.value.byte_len == 0
                && request.input.admitted_bytes == 0
            {
                Ok(())
            } else {
                Err(SchedulerError::InvalidHostCallAccess)
            };
        }
        if self.initial[node] {
            if !bytes.is_empty() || request.input.value.byte_len != 0 {
                return Err(SchedulerError::ValueOwnershipViolation);
            }
            self.initial[node] = false;
            return Ok(());
        }
        let cord = usize::from(self.incoming[node].ok_or(SchedulerError::ValueOwnershipViolation)?);
        if cord >= CORDS || self.len[cord] == 0 {
            return Err(SchedulerError::ValueOwnershipViolation);
        }
        let slot = usize::from(self.start[cord] + self.head[cord]);
        let loan = self
            .loans
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or(SchedulerError::ValueOwnershipViolation)?;
        if !loan.matches(request.input.value, bytes) {
            return Err(SchedulerError::ValueOwnershipViolation);
        }
        self.loans[slot] = None;
        self.head[cord] = (self.head[cord] + 1) % self.capacity[cord];
        self.len[cord] -= 1;
        Ok(())
    }
    pub fn publish(
        &mut self,
        node: usize,
        value: Option<ValueRef>,
        bytes: &[u8],
    ) -> Result<(), SchedulerError> {
        let loan = Loan::new(value, bytes)?;
        // Preflight every consumer before recording any new grant.
        for cord in 0..CORDS {
            if self.outgoing[node] & (1 << cord) != 0
                && (self.capacity[cord] == 0 || self.len[cord] >= self.capacity[cord])
            {
                return Err(SchedulerError::OutputBlocked);
            }
        }
        for cord in 0..CORDS {
            if self.outgoing[node] & (1 << cord) != 0 {
                let at = (self.head[cord] + self.len[cord]) % self.capacity[cord];
                let slot = usize::from(self.start[cord] + at);
                if self.loans.get(slot).is_none_or(Option::is_some) {
                    return Err(SchedulerError::ValueOwnershipViolation);
                }
                self.loans[slot] = Some(loan);
                self.len[cord] += 1;
            }
        }
        Ok(())
    }
    /// Root-issued bytes are a custody receipt, not a guest-selected value grant.
    pub fn preflight_ingress(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<(), SchedulerError> {
        let index = usize::from(cord.0);
        if self.ingress.get(index).copied().flatten() != Some(endpoint) {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        if self.ingress_sequence[index] != sequence || sequence.checked_add(1).is_none() {
            return Err(SchedulerError::RemoteSequenceRejected);
        }
        if bytes.len() > self.ingress_limit[index] as usize {
            return Err(SchedulerError::QueueByteCapacityExceeded);
        }
        if self.len[index] >= self.capacity[index] {
            return Err(SchedulerError::OutputBlocked);
        }
        Ok(())
    }
    /// Called only after exact guest acceptance. A Full response creates no loan
    /// and consumes no sequence; a later uncertain failure fences the broker.
    pub fn accepted_ingress(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<(), SchedulerError> {
        self.preflight_ingress(endpoint, cord, sequence, bytes)?;
        let index = usize::from(cord.0);
        let at = (self.head[index] + self.len[index]) % self.capacity[index];
        let slot = usize::from(self.start[index] + at);
        if self.loans.get(slot).is_none_or(Option::is_some) {
            return Err(SchedulerError::ValueOwnershipViolation);
        }
        self.loans[slot] = Some(Loan::new(None, bytes)?);
        self.len[index] += 1;
        self.ingress_sequence[index] = sequence
            .checked_add(1)
            .ok_or(SchedulerError::RemoteSequenceRejected)?;
        Ok(())
    }
    pub fn clear(&mut self) {
        self.loans.fill(None);
        self.initial.fill(false);
        self.ingress.fill(None);
        self.ingress_limit.fill(0);
        self.ingress_sequence.fill(0);
        self.len.fill(0);
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
