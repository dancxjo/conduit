use crate::shared_pool::MemberIdentity;
use crate::{Failure, ValueRef};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanBranchDisposition {
    Pending,
    Delivered,
    Failed(Failure),
    Cancelled,
}

impl FanBranchDisposition {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanBranch {
    pub recipient: MemberIdentity,
    pub disposition: FanBranchDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanError {
    InvalidCapacity,
    ItemInFlight,
    RecipientCapacityExceeded,
    DuplicateRecipient,
    NoItemInFlight,
    UnknownRecipient,
    BranchAlreadyTerminal,
    BranchesPending,
}

/// State for one semantic input item replicated to one immutable membership
/// snapshot. Hosts/lines may apply pressure to individual addressed
/// branches, but cannot add or remove recipients from the captured item.
pub struct FixedFan<const BRANCHES: usize> {
    branches: [Option<FanBranch>; BRANCHES],
    branch_count: u16,
    value: Option<ValueRef>,
}

impl<const BRANCHES: usize> FixedFan<BRANCHES> {
    pub fn new() -> Result<Self, FanError> {
        if BRANCHES == 0 || BRANCHES > usize::from(u16::MAX) {
            return Err(FanError::InvalidCapacity);
        }
        Ok(Self {
            branches: [None; BRANCHES],
            branch_count: 0,
            value: None,
        })
    }

    pub fn begin(
        &mut self,
        value: ValueRef,
        recipients: &[MemberIdentity],
    ) -> Result<(), FanError> {
        if self.value.is_some() {
            return Err(FanError::ItemInFlight);
        }
        if recipients.len() > BRANCHES || recipients.len() > usize::from(u16::MAX) {
            return Err(FanError::RecipientCapacityExceeded);
        }
        for (index, recipient) in recipients.iter().enumerate() {
            if recipients[..index].iter().any(|prior| prior == recipient) {
                return Err(FanError::DuplicateRecipient);
            }
        }
        self.branches.fill(None);
        for (slot, recipient) in self.branches.iter_mut().zip(recipients) {
            *slot = Some(FanBranch {
                recipient: *recipient,
                disposition: FanBranchDisposition::Pending,
            });
        }
        self.branch_count = recipients.len() as u16;
        self.value = Some(value);
        Ok(())
    }

    pub fn value(&self) -> Option<ValueRef> {
        self.value
    }

    pub fn branches(&self) -> impl Iterator<Item = FanBranch> + '_ {
        self.branches[..usize::from(self.branch_count)]
            .iter()
            .copied()
            .flatten()
    }

    pub fn next_pending(&self) -> Option<(MemberIdentity, ValueRef)> {
        let value = self.value?;
        self.branches().find_map(|branch| {
            (branch.disposition == FanBranchDisposition::Pending)
                .then_some((branch.recipient, value))
        })
    }

    /// Receiver pressure is not terminal and deliberately leaves the exact
    /// branch pending for the same value and recipient.
    pub fn observe_full(&self, recipient: MemberIdentity) -> Result<(), FanError> {
        let branch = self.branch(recipient)?;
        if branch.disposition != FanBranchDisposition::Pending {
            return Err(FanError::BranchAlreadyTerminal);
        }
        Ok(())
    }

    pub fn deliver(&mut self, recipient: MemberIdentity) -> Result<(), FanError> {
        self.terminal(recipient, FanBranchDisposition::Delivered)
    }

    pub fn fail(&mut self, recipient: MemberIdentity, failure: Failure) -> Result<(), FanError> {
        self.terminal(recipient, FanBranchDisposition::Failed(failure))
    }

    pub fn cancel(&mut self, recipient: MemberIdentity) -> Result<(), FanError> {
        self.terminal(recipient, FanBranchDisposition::Cancelled)
    }

    pub fn take_terminal_value(&mut self) -> Result<ValueRef, FanError> {
        let value = self.value.ok_or(FanError::NoItemInFlight)?;
        if self
            .branches()
            .any(|branch| !branch.disposition.is_terminal())
        {
            return Err(FanError::BranchesPending);
        }
        self.value = None;
        self.branch_count = 0;
        self.branches.fill(None);
        Ok(value)
    }

    fn branch(&self, recipient: MemberIdentity) -> Result<FanBranch, FanError> {
        if self.value.is_none() {
            return Err(FanError::NoItemInFlight);
        }
        self.branches()
            .find(|branch| branch.recipient == recipient)
            .ok_or(FanError::UnknownRecipient)
    }

    fn terminal(
        &mut self,
        recipient: MemberIdentity,
        disposition: FanBranchDisposition,
    ) -> Result<(), FanError> {
        if self.value.is_none() {
            return Err(FanError::NoItemInFlight);
        }
        let Some(branch) = self.branches[..usize::from(self.branch_count)]
            .iter_mut()
            .flatten()
            .find(|branch| branch.recipient == recipient)
        else {
            return Err(FanError::UnknownRecipient);
        };
        if branch.disposition != FanBranchDisposition::Pending {
            return Err(FanError::BranchAlreadyTerminal);
        }
        branch.disposition = disposition;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeEvent {
    pub sequence: u64,
    pub source: MemberIdentity,
    pub value: ValueRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeError {
    InvalidCapacity,
    QueueFull,
    DuplicateOrOutOfOrderSequence,
}

/// Explicit bounded many-to-one event stream. Every retained value keeps the
/// exact source member identity and caller-issued monotonic sequence.
pub struct FixedMerge<const EVENTS: usize> {
    events: [Option<MergeEvent>; EVENTS],
    head: u16,
    len: u16,
    last_sequence: Option<u64>,
}

impl<const EVENTS: usize> FixedMerge<EVENTS> {
    pub fn new() -> Result<Self, MergeError> {
        if EVENTS == 0 || EVENTS > usize::from(u16::MAX) {
            return Err(MergeError::InvalidCapacity);
        }
        Ok(Self {
            events: [None; EVENTS],
            head: 0,
            len: 0,
            last_sequence: None,
        })
    }

    pub fn len(&self) -> u16 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn offer(&mut self, event: MergeEvent) -> Result<(), MergeError> {
        if usize::from(self.len) >= EVENTS {
            return Err(MergeError::QueueFull);
        }
        if self
            .last_sequence
            .is_some_and(|last| event.sequence <= last)
        {
            return Err(MergeError::DuplicateOrOutOfOrderSequence);
        }
        let index = (usize::from(self.head) + usize::from(self.len)) % EVENTS;
        self.events[index] = Some(event);
        self.len += 1;
        self.last_sequence = Some(event.sequence);
        Ok(())
    }

    pub fn front(&self) -> Option<MergeEvent> {
        self.events[usize::from(self.head)]
    }

    pub fn pop(&mut self) -> Option<MergeEvent> {
        let event = self.events[usize::from(self.head)].take()?;
        self.head = ((usize::from(self.head) + 1) % EVENTS) as u16;
        self.len -= 1;
        Some(event)
    }
}
