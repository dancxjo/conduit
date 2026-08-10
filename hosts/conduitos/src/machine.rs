//! Architecture-neutral contracts for finite machine Bases.
//!
//! These are mechanisms beneath Host offers. They carry no authored meaning,
//! scheduling policy, ambient authority, or architecture-specific vocabulary.

use conduit_kernel::{BoundedValueRef, NodeId, RequestId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseKind {
    Clock,
    Timer,
    Serial,
    Interrupt,
    Idle,
    ExecutionLane,
    Memory,
}

impl BaseKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Timer => "timer",
            Self::Serial => "serial",
            Self::Interrupt => "interrupt",
            Self::Idle => "idle",
            Self::ExecutionLane => "execution-lane",
            Self::Memory => "memory",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseError {
    RingFull,
    SlotFull,
    Masked,
    StaleWake,
    DuplicateWake,
    TimerCancelled,
    PayloadTooLarge,
    Unavailable,
}

impl BaseError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RingFull => "base-ring-full",
            Self::SlotFull => "timer-slots-full",
            Self::Masked => "interrupt-masked",
            Self::StaleWake => "stale-wake",
            Self::DuplicateWake => "duplicate-wake",
            Self::TimerCancelled => "timer-cancelled",
            Self::PayloadTooLarge => "serial-payload-too-large",
            Self::Unavailable => "base-unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelInterest {
    pub node: NodeId,
    pub request: RequestId,
    pub input: BoundedValueRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerToken {
    pub slot: u8,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimerState {
    Empty,
    Armed(KernelInterest),
    Fired,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerSlot {
    generation: u32,
    state: TimerState,
}

impl TimerSlot {
    const EMPTY: Self = Self {
        generation: 0,
        state: TimerState::Empty,
    };
}

/// Fixed timer-operation slots. Terminal slot state is retained so a late,
/// stale, duplicate, or cancelled wake remains machine-readable.
pub struct FixedTimerSlots<const SLOTS: usize> {
    slots: [TimerSlot; SLOTS],
    masked: bool,
    failed: bool,
}

impl<const SLOTS: usize> FixedTimerSlots<SLOTS> {
    pub const fn new() -> Self {
        Self {
            slots: [TimerSlot::EMPTY; SLOTS],
            masked: false,
            failed: false,
        }
    }

    pub fn set_masked(&mut self, masked: bool) {
        self.masked = masked;
    }

    pub fn set_failed(&mut self, failed: bool) {
        self.failed = failed;
    }

    pub fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError> {
        if self.failed {
            return Err(BaseError::Unavailable);
        }
        if self.masked {
            return Err(BaseError::Masked);
        }
        let (index, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| !matches!(slot.state, TimerState::Armed(_)))
            .ok_or(BaseError::SlotFull)?;
        slot.generation = slot
            .generation
            .checked_add(1)
            .ok_or(BaseError::Unavailable)?;
        slot.state = TimerState::Armed(interest);
        Ok(TimerToken {
            slot: u8::try_from(index).map_err(|_| BaseError::SlotFull)?,
            generation: slot.generation,
        })
    }

    pub fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        let slot = self.slot_mut(token)?;
        match slot.state {
            TimerState::Armed(interest) => {
                slot.state = TimerState::Cancelled;
                Ok(interest)
            }
            TimerState::Cancelled => Err(BaseError::TimerCancelled),
            TimerState::Fired => Err(BaseError::DuplicateWake),
            TimerState::Empty => Err(BaseError::StaleWake),
        }
    }

    pub fn wake(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        let slot = self.slot_mut(token)?;
        match slot.state {
            TimerState::Armed(interest) => {
                slot.state = TimerState::Fired;
                Ok(interest)
            }
            TimerState::Fired => Err(BaseError::DuplicateWake),
            TimerState::Cancelled => Err(BaseError::TimerCancelled),
            TimerState::Empty => Err(BaseError::StaleWake),
        }
    }

    fn slot_mut(&mut self, token: TimerToken) -> Result<&mut TimerSlot, BaseError> {
        let slot = self
            .slots
            .get_mut(usize::from(token.slot))
            .ok_or(BaseError::StaleWake)?;
        if slot.generation != token.generation {
            return Err(BaseError::StaleWake);
        }
        Ok(slot)
    }
}

impl<const SLOTS: usize> Default for FixedTimerSlots<SLOTS> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FixedFactRing<T: Copy, const SLOTS: usize> {
    entries: [Option<T>; SLOTS],
    head: usize,
    len: usize,
}

impl<T: Copy, const SLOTS: usize> FixedFactRing<T, SLOTS> {
    pub const fn new() -> Self {
        Self {
            entries: [None; SLOTS],
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) -> Result<(), BaseError> {
        if self.len == SLOTS {
            return Err(BaseError::RingFull);
        }
        let index = (self.head + self.len) % SLOTS;
        self.entries[index] = Some(value);
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let value = self.entries[self.head].take();
        self.head = (self.head + 1) % SLOTS;
        self.len -= 1;
        value
    }

    pub const fn capacity(&self) -> usize {
        SLOTS
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T: Copy, const SLOTS: usize> Default for FixedFactRing<T, SLOTS> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait MonotonicClockBase {
    fn now(&mut self) -> u64;
}

pub trait TimerBase {
    fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError>;
    fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError>;
    fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError>;
    fn wake_count(&self) -> u32;
}

pub trait SerialBase {
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError>;
    fn presentation_count(&self) -> u32;
}

pub trait InterruptBase {
    fn enable(&mut self);
    fn disable(&mut self) -> InterruptState;
    fn restore(&mut self, state: InterruptState);
    fn is_enabled(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptState {
    pub enabled: bool,
}

pub trait IdleBase {
    fn wait_for_interrupt(&mut self) -> Result<(), BaseError>;
    fn idle_count(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueRef;

    fn interest(request: u32) -> KernelInterest {
        let value = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 4,
        };
        KernelInterest {
            node: NodeId(0),
            request: RequestId(request),
            input: BoundedValueRef::new(value, 4).unwrap(),
        }
    }

    #[test]
    fn finite_ring_refuses_instead_of_overwriting() {
        let mut ring = FixedFactRing::<u8, 2>::new();
        ring.push(1).unwrap();
        ring.push(2).unwrap();
        assert_eq!(ring.push(3), Err(BaseError::RingFull));
        assert_eq!(ring.pop(), Some(1));
        assert_eq!(ring.pop(), Some(2));
    }

    #[test]
    fn timer_wakes_are_exact_and_terminal_states_remain_distinct() {
        let mut timers = FixedTimerSlots::<1>::new();
        let token = timers.arm(interest(7)).unwrap();
        assert_eq!(timers.wake(token).unwrap().request, RequestId(7));
        assert_eq!(timers.wake(token), Err(BaseError::DuplicateWake));

        let replacement = timers.arm(interest(8)).unwrap();
        assert_eq!(timers.wake(token), Err(BaseError::StaleWake));
        assert_eq!(timers.cancel(replacement).unwrap().request, RequestId(8));
        assert_eq!(timers.wake(replacement), Err(BaseError::TimerCancelled));
    }

    #[test]
    fn masked_failed_and_full_timer_states_refuse_separately() {
        let mut timers = FixedTimerSlots::<1>::new();
        timers.set_masked(true);
        assert_eq!(timers.arm(interest(1)), Err(BaseError::Masked));
        timers.set_masked(false);
        let _token = timers.arm(interest(1)).unwrap();
        assert_eq!(timers.arm(interest(2)), Err(BaseError::SlotFull));
        timers.set_failed(true);
        assert_eq!(timers.arm(interest(3)), Err(BaseError::Unavailable));
    }
}
