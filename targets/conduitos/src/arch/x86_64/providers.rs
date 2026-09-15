use crate::machine::{
    BaseError, FixedTimerSlots, IdleBase, InterruptBase, InterruptState, KernelInterest,
    MonotonicClockBase, SerialBase, TimerBase, TimerToken,
};

use super::{TIMER_IRQ_VECTOR, acpi, cpu, gdt, idt, interrupt_controller, irq, pic, serial};

pub fn initialize_machine(
    record: &crate::boot::BootRecord,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) {
    cpu::disable_interrupts();
    serial::initialize();
    crate::arch::early_write(b"CONDUIT_BOOT_STAGE machine-gdt\n");
    gdt::initialize();
    crate::arch::early_write(b"CONDUIT_BOOT_STAGE machine-idt\n");
    idt::initialize();
    crate::arch::early_write(b"CONDUIT_BOOT_STAGE machine-pic\n");
    pic::initialize();
    crate::arch::early_write(b"CONDUIT_BOOT_STAGE machine-acpi\n");
    let topology = acpi::discover(
        record.hhdm_offset,
        record.rsdp_address,
        image_virtual_to_physical,
    )
    .unwrap_or_else(|error| {
        crate::arch::early_write(b"CONDUIT_MACHINE_REFUSAL ");
        crate::arch::early_write(error.as_str().as_bytes());
        crate::arch::early_write(b"\n");
        cpu::deterministic_exit(false)
    });
    crate::arch::early_write(b"CONDUIT_BOOT_STAGE machine-controller\n");
    if let Err(error) =
        interrupt_controller::initialize(record.hhdm_offset, topology, image_virtual_to_physical)
    {
        crate::arch::early_write(b"CONDUIT_MACHINE_REFUSAL ");
        crate::arch::early_write(error.as_str().as_bytes());
        crate::arch::early_write(b"\n");
        cpu::deterministic_exit(false);
    }
    crate::arch::early_write(b"CONDUIT_BOOT_STAGE machine-ready\n");
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
        let token = self.slots.arm(interest).inspect_err(|error| {
            crate::arch::early_write(b"CONDUIT_TIMER_REFUSAL ");
            crate::arch::early_write(error.as_str().as_bytes());
            crate::arch::early_write(b"\n");
        })?;
        self.active = Some(token);
        interrupt_controller::arm_timer();
        Ok(token)
    }

    fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        interrupt_controller::cancel_timer();
        self.active = None;
        self.slots.cancel(token)
    }

    fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError> {
        let Some(vector) = irq::pop().inspect_err(|error| {
            crate::arch::early_write(b"CONDUIT_TIMER_REFUSAL ");
            crate::arch::early_write(error.as_str().as_bytes());
            crate::arch::early_write(b"\n");
        })?
        else {
            return Ok(None);
        };
        if vector != TIMER_IRQ_VECTOR {
            crate::arch::early_write(b"CONDUIT_TIMER_REFUSAL unexpected-vector\n");
            return Err(BaseError::Unavailable);
        }
        let token = self.active.take().ok_or_else(|| {
            crate::arch::early_write(b"CONDUIT_TIMER_REFUSAL stale-wake\n");
            BaseError::StaleWake
        })?;
        interrupt_controller::cancel_timer();
        let interest = self.slots.wake(token).inspect_err(|error| {
            crate::arch::early_write(b"CONDUIT_TIMER_REFUSAL ");
            crate::arch::early_write(error.as_str().as_bytes());
            crate::arch::early_write(b"\n");
        })?;
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
