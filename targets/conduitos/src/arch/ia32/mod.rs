//! IA-32 PC mechanisms below the generic machine/Base seam.

use super::ia32_timer_lifecycle::{IrqMailbox, TimerState};
use crate::machine::{
    BaseError, IdleBase, InterruptBase, InterruptState, KernelInterest, MonotonicClockBase,
    SerialBase, TimerBase, TimerToken,
};

#[cfg(target_os = "linux")]
mod domain_budget;
#[cfg(target_os = "linux")]
mod domain_memory;
#[cfg(target_os = "linux")]
mod domain_transition;
#[cfg(target_os = "linux")]
#[path = "../ordinary_domain.rs"]
mod ordinary_domain;
#[cfg(target_os = "linux")]
pub use ordinary_domain::TextDomain;
mod entropy;
pub use entropy::RdrandEntropy;

#[cfg(target_os = "linux")]
fn domain_ticks() -> u64 {
    read_counter()
}
pub fn early_write(bytes: &[u8]) {
    present(bytes);
}
mod timer_hardware;

pub const PIT_IRQ: u8 = 32;
static TIMER_IRQ: IrqMailbox = IrqMailbox::new();
static mut IDT: [u64; 256] = [0; 256];

#[cfg(target_os = "linux")]
unsafe fn install_domain_gate(vector: u8, handler: u32, attributes: u8) {
    let gate = u64::from(handler & 0xffff)
        | (0x08_u64 << 16)
        | (u64::from(attributes) << 40)
        | (u64::from(handler >> 16) << 48);
    unsafe {
        IDT[vector as usize] = gate;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptFact {
    Timer,
    UnarmedTimer,
    WrongSource(u8),
    Overflow,
}

core::arch::global_asm!(
    r#"
.section .text.conduitos_ia32_irq,"ax",@progbits
.global conduitos_ia32_irq_entry
conduitos_ia32_irq_entry:
    pushad
    call conduitos_ia32_irq_handler
    popad
    iretd
"#
);

pub fn initialize_machine() {
    disable_interrupts();
    super::ia32_domain_gdt::initialize();
    #[cfg(target_os = "linux")]
    domain_budget::initialize();
    timer_hardware::report_inherited_state();
    timer_hardware::stop();
    TIMER_IRQ.retire();
    let handler = conduitos_ia32_irq_entry as *const () as usize as u32;
    let selector: u16;
    unsafe {
        core::arch::asm!("mov {0:x}, cs", out(reg) selector, options(nomem, nostack, preserves_flags))
    };
    let gate = u64::from(handler & 0xffff)
        | (u64::from(selector) << 16)
        | (0x8e_u64 << 40)
        | (u64::from(handler >> 16) << 48);
    unsafe {
        IDT[PIT_IRQ as usize] = gate;
        let descriptor = DescriptorTable {
            limit: (core::mem::size_of::<[u64; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u32,
        };
        core::arch::asm!("lidt [{0}]", in(reg) &descriptor, options(readonly, nostack, preserves_flags));
        remap_pic();
    }
}

// Raw architecture appliance entrance; production uses its admitted token.
pub fn timer_arm() {
    with_interrupts_masked(|| {
        timer_hardware::quiesce()
            .and_then(|()| TIMER_IRQ.start(1))
            .unwrap_or_else(|error| {
                timer_refusal("appliance-arm", error);
                emergency_halt()
            });
        timer_hardware::start();
    });
}

pub fn enable_interrupts() {
    unsafe { core::arch::asm!("sti", options(nostack, preserves_flags)) }
}

pub fn disable_interrupts() {
    unsafe { core::arch::asm!("cli", options(nostack, preserves_flags)) }
}

fn interrupts_enabled() -> bool {
    let flags: u32;
    unsafe {
        core::arch::asm!("pushfd", "pop {0:e}", out(reg) flags, options(nomem, preserves_flags))
    };
    flags & (1 << 9) != 0
}

pub fn interruptible_idle() {
    unsafe { core::arch::asm!("hlt", options(nomem, nostack)) }
}

pub const fn emergency_machine_profile() -> crate::machine::EmergencyMachineProfile {
    crate::machine::EmergencyMachineProfile {
        halt: crate::machine::EmergencyMachineAvailability::Available,
        reset: crate::machine::EmergencyMachineAvailability::Unavailable,
    }
}

pub fn emergency_halt() -> ! {
    disable_interrupts();
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) }
    }
}

fn interruptible_idle_once() {
    unsafe { core::arch::asm!("sti", "hlt", "cli", options(nomem, nostack)) }
}

fn with_interrupts_masked<T>(operation: impl FnOnce() -> T) -> T {
    let enabled = interrupts_enabled();
    disable_interrupts();
    let result = operation();
    if enabled {
        enable_interrupts();
    }
    result
}

pub fn pop_interrupt() -> Option<InterruptFact> {
    with_interrupts_masked(|| match TIMER_IRQ.pop() {
        Err(_) => Some(InterruptFact::Overflow),
        Ok(Some(0)) => Some(InterruptFact::UnarmedTimer),
        Ok(Some(_)) => Some(InterruptFact::Timer),
        Ok(None) => None,
    })
}

pub fn present(bytes: &[u8]) {
    for byte in bytes {
        unsafe { outb(0xe9, *byte) };
    }
}

pub fn present_legacy_bios_receipt(
    profile_id: &[u8],
    build_id: &[u8],
    image_id: &[u8],
    host_id: &[u8],
    boot_id: &[u8],
) {
    super::ia32_vga_text::present_boot_receipt(profile_id, build_id, image_id, host_id, boot_id);
}

pub fn read_counter() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)) };
    u64::from(low) | (u64::from(high) << 32)
}

unsafe extern "C" {
    fn conduitos_ia32_irq_entry();
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_ia32_irq_handler() {
    TIMER_IRQ.publish();
    unsafe { outb(0x20, 0x20) };
}

#[repr(C, packed)]
struct DescriptorTable {
    limit: u16,
    base: u32,
}

unsafe fn remap_pic() {
    unsafe {
        outb(0x20, 0x11);
        outb(0xa0, 0x11);
        outb(0x21, 0x20);
        outb(0xa1, 0x28);
        outb(0x21, 0x04);
        outb(0xa1, 0x02);
        outb(0x21, 0x01);
        outb(0xa1, 0x01);
        outb(0x21, 0xff);
        outb(0xa1, 0xff);
    }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags))
    };
}

unsafe fn inb(port: u16) -> u8 {
    let value;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nostack, preserves_flags))
    };
    value
}

pub struct Clock(u64);
impl Clock {
    pub const fn new() -> Self {
        Self(0)
    }
}
impl MonotonicClockBase for Clock {
    fn provider_generation(&self) -> Option<u64> {
        Some(1)
    }
    fn now(&mut self) -> u64 {
        self.0 = read_counter().max(self.0);
        self.0
    }
}

pub struct Timer {
    state: TimerState,
    wakes: u32,
}
impl Timer {
    pub const fn new() -> Self {
        Self {
            state: TimerState::new(),
            wakes: 0,
        }
    }
}
impl TimerBase for Timer {
    fn provider_generation(&self) -> Option<u64> {
        Some(1)
    }
    fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError> {
        with_interrupts_masked(|| {
            let token = self
                .state
                .arm(interest)
                .map_err(|error| timer_refusal("arm-slot", error))?;
            // Check mailbox eligibility before touching an existing hardware
            // arm. CPU masking fences this generation until quiesce and the
            // complete new PIT count have both been written.
            let start = TIMER_IRQ
                .start(token.generation)
                .map_err(|error| timer_refusal("arm-irq", error))
                .and_then(|()| {
                    timer_hardware::quiesce().map_err(|error| {
                        TIMER_IRQ.retire();
                        timer_refusal("arm-quiesce", error)
                    })
                });
            if let Err(error) = start {
                // A refused hardware arm must not leave an occupied slot.
                self.state
                    .cancel(token)
                    .map_err(|error| timer_refusal("arm-rollback", error))?;
                return Err(error);
            }
            timer_hardware::start();
            Ok(token)
        })
    }
    fn cancel(&mut self, token: TimerToken) -> Result<KernelInterest, BaseError> {
        with_interrupts_masked(|| {
            // Validate first: a stale cancellation cannot stop the current PIT.
            let interest = self
                .state
                .cancel(token)
                .map_err(|error| timer_refusal("cancel-slot", error))?;
            TIMER_IRQ.retire();
            timer_hardware::quiesce().map_err(|error| timer_refusal("cancel-quiesce", error))?;
            Ok(interest)
        })
    }
    fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError> {
        with_interrupts_masked(|| {
            let Some(generation) = TIMER_IRQ
                .pop()
                .map_err(|error| timer_refusal("wake-overflow", error))?
            else {
                return Ok(None);
            };
            let interest = self
                .state
                .wake(generation)
                .map_err(|error| timer_refusal("wake-slot", error))?;
            TIMER_IRQ.retire();
            timer_hardware::quiesce().map_err(|error| timer_refusal("wake-quiesce", error))?;
            self.wakes = self
                .wakes
                .checked_add(1)
                .ok_or_else(|| timer_refusal("wake-count", BaseError::Unavailable))?;
            Ok(Some(interest))
        })
    }
    fn wake_count(&self) -> u32 {
        self.wakes
    }
}

// Preserve the generic terminal failure while retaining the actual Base fault.
// Static vocabulary only; no allocation, retry or reinterpretation of IRQ facts.
fn timer_refusal(operation: &str, error: BaseError) -> BaseError {
    present(b"CONDUIT_IA32_TIMER_BASE_REFUSAL {\"operation\":\"");
    present(operation.as_bytes());
    present(b"\",\"reason\":\"");
    present(error.as_str().as_bytes());
    present(b"\"}\n");
    error
}

pub struct Serial(u32);
impl Serial {
    pub const fn new() -> Self {
        Self(0)
    }
}
impl SerialBase for Serial {
    fn provider_generation(&self) -> Option<u64> {
        Some(1)
    }
    fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
        present(bytes);
        self.0 = self.0.checked_add(1).ok_or(BaseError::Unavailable)?;
        Ok(())
    }
    fn presentation_count(&self) -> u32 {
        self.0
    }
}

pub struct Interrupts;
impl Interrupts {
    pub const fn new() -> Self {
        Self
    }
}
impl InterruptBase for Interrupts {
    fn enable(&mut self) {
        enable_interrupts();
    }
    fn disable(&mut self) -> InterruptState {
        let state = InterruptState {
            enabled: interrupts_enabled(),
        };
        disable_interrupts();
        state
    }
    fn restore(&mut self, state: InterruptState) {
        if state.enabled {
            enable_interrupts();
        } else {
            disable_interrupts();
        }
    }
    fn is_enabled(&self) -> bool {
        interrupts_enabled()
    }
}

pub struct Idle(u32);
impl Idle {
    pub const fn new() -> Self {
        Self(0)
    }
}
impl IdleBase for Idle {
    fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
        self.0 = self.0.checked_add(1).ok_or(BaseError::Unavailable)?;
        with_interrupts_masked(|| {
            // Delivery may have happened after arm restored an enabled caller.
            // Never sleep past an already captured fact (including overflow).
            if !TIMER_IRQ.has_fact() {
                interruptible_idle_once();
            }
        });
        Ok(())
    }
    fn idle_count(&self) -> u32 {
        self.0
    }
}
