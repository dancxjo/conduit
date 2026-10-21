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
struct Loan {
    value: Option<ValueRef>,
    len: usize,
    bytes: [u8; VALUE_BYTES],
}
impl Loan {
    fn new(value: Option<ValueRef>, bytes: &[u8]) -> Result<Self, SchedulerError> {
        if bytes.len() > VALUE_BYTES {
            return Err(SchedulerError::HostCallOutputExceeded);
        }
        let mut result = Self {
            value,
            len: bytes.len(),
            bytes: [0; VALUE_BYTES],
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
pub(super) struct Ledger {
    kinds: [BackKind; NODES],
    incoming: [Option<u16>; NODES],
    outgoing: [u16; NODES],
    start: [u16; CORDS],
    capacity: [u16; CORDS],
    head: [u16; CORDS],
    len: [u16; CORDS],
    loans: [Option<Loan>; CORDS * 2],
    initial: [bool; NODES],
}
impl Ledger {
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
        // The unchanged Edit Back emits one admitted empty value before its
        // first Host Call. Its only effect grant is empty text on reviewed routes.
        for node in 0..usize::from(admission.active_nodes) {
            if admission.backs[node] == BackKind::Edit {
                result.publish(node, None, &[])?;
            }
        }
        Ok(result)
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
    pub fn clear(&mut self) {
        self.loans.fill(None);
        self.initial.fill(false);
        self.len.fill(0);
    }
}
