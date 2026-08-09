use crate::machine::{
    BaseError, FixedTimerSlots, IdleBase, InterruptBase, InterruptState, KernelInterest,
    MonotonicClockBase, SerialBase, TimerBase, TimerToken,
};

use super::{TIMER_IRQ_VECTOR, cpu, gdt, idt, irq, pic, pit, serial};

pub fn initialize_machine() {
    cpu::disable_interrupts();
    serial::initialize();
    gdt::initialize();
    idt::initialize();
    pic::initialize();
}

pub struct Clock {
    last: u64,
}

impl Clock {
    pub const fn new() -> Self {
        Self { last: 0 }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl MonotonicClockBase for Clock {
    fn now(&mut self) -> u64 {
        let observed = cpu::read_tsc();
        self.last = observed.max(self.last);
        self.last
    }
}

pub struct Timer {
    slots: FixedTimerSlots<1>,
    active: Option<TimerToken>,
    wakes: u32,
}

impl Timer {
    pub const fn new() -> Self {
        Self {
            slots: FixedTimerSlots::new(),
            active: None,
            wakes: 0,
        }
    }

    pub const fn wakes(&self) -> u32 {
        self.wakes
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerBase for Timer {
    fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError> {
        let token = self.slots.arm(interest)?;
        self.active = Some(token);
        pic::unmask_timer();
        pit::arm_one_shot();
        Ok(token)
    }

    fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        pic::mask_timer();
        self.active = None;
        self.slots.cancel(token)
    }

    fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError> {
        let Some(vector) = irq::pop()? else {
            return Ok(None);
        };
        if vector != TIMER_IRQ_VECTOR {
            return Err(BaseError::Unavailable);
        }
        let token = self.active.take().ok_or(BaseError::StaleWake)?;
        pic::mask_timer();
        let interest = self.slots.wake(token)?;
        self.wakes = self.wakes.checked_add(1).ok_or(BaseError::Unavailable)?;
        Ok(Some(interest))
    }

    fn wake_count(&self) -> u32 {
        self.wakes
    }
}

pub struct Serial {
    presentations: u32,
}

impl Serial {
    pub const fn new() -> Self {
        Self { presentations: 0 }
    }

    pub const fn presentations(&self) -> u32 {
        self.presentations
    }
}

impl Default for Serial {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialBase for Serial {
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
        serial::present(bytes)?;
        self.presentations = self
            .presentations
            .checked_add(1)
            .ok_or(BaseError::Unavailable)?;
        Ok(())
    }

    fn presentation_count(&self) -> u32 {
        self.presentations
    }
}

pub struct Interrupts {
    enabled: bool,
}

impl Interrupts {
    pub const fn new() -> Self {
        Self { enabled: false }
    }
}

impl Default for Interrupts {
    fn default() -> Self {
        Self::new()
    }
}

impl InterruptBase for Interrupts {
    fn enable(&mut self) {
        cpu::enable_interrupts();
        self.enabled = true;
    }

    fn disable(&mut self) -> InterruptState {
        let state = InterruptState {
            enabled: cpu::interrupts_enabled(),
        };
        cpu::disable_interrupts();
        self.enabled = false;
        state
    }

    fn restore(&mut self, state: InterruptState) {
        if state.enabled {
            self.enable();
        } else {
            cpu::disable_interrupts();
            self.enabled = false;
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

pub struct Idle {
    entries: u32,
}

impl Idle {
    pub const fn new() -> Self {
        Self { entries: 0 }
    }

    pub const fn entries(&self) -> u32 {
        self.entries
    }
}

impl Default for Idle {
    fn default() -> Self {
        Self::new()
    }
}

impl IdleBase for Idle {
    fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
        self.entries = self.entries.checked_add(1).ok_or(BaseError::Unavailable)?;
        cpu::interruptible_idle();
        Ok(())
    }

    fn idle_count(&self) -> u32 {
        self.entries
    }
}
