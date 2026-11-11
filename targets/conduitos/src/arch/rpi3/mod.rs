//! BCM2837B0 mechanisms for direct AArch64 firmware boot on the Pi 3B+.
//! Core 0 owns Root; no secondary-core scheduling or Source isolation is offered.
use super::raspberry_pi::{Board, HubReset};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};
const BOARD: Board = Board::ThreeBPlus;
const PERIPHERAL_BASE: usize = BOARD.peripheral_base();
const GPIO_BASE: usize = PERIPHERAL_BASE + 0x20_0000;
const UART: usize = GPIO_BASE + 0x1000;
const TIMER: usize = PERIPHERAL_BASE + 0x3000;
const INTERRUPT: usize = PERIPHERAL_BASE + 0xb000;
// Keep all firmware scanout candidates uncached, including carveout alignment.
const SCANOUT_START: u32 = 0x3000_0000;
const SCANOUT_END: u32 = 0x3f00_0000;
#[path = "../armv6/audio.rs"]
mod audio;
#[path = "../armv6/entropy.rs"]
mod entropy;
#[path = "../armv6/framebuffer.rs"]
mod framebuffer;
#[path = "../armv6/mailbox.rs"]
mod mailbox;
mod memory;
#[path = "../armv6/providers.rs"]
mod providers;
#[path = "../armv6/usb.rs"]
pub(crate) mod usb;
pub use audio::PwmAudio;
pub use entropy::HardwareEntropy;
pub use framebuffer::FirmwareScanout;
pub use providers::{Clock, Idle, Interrupts, Serial, Timer};
static PENDING: AtomicBool = AtomicBool::new(false);
static OVERFLOW: AtomicBool = AtomicBool::new(false);
#[derive(Clone, Copy)]
pub enum InterruptFact {
    Timer,
    WrongSource,
    Overflow,
}
core::arch::global_asm!(include_str!("../aarch64/board_vectors.S"));
unsafe extern "C" {
    static conduitos_board_vectors: u8;
}

pub fn initialize_machine() {
    disable_interrupts();
    unsafe {
        core::arch::asm!("msr vbar_el1, {0}", "isb", in(reg) core::ptr::addr_of!(conduitos_board_vectors), options(nostack));
        let mut cpacr: u64;
        core::arch::asm!("mrs {0}, cpacr_el1", out(reg) cpacr, options(nostack));
        core::arch::asm!("msr cpacr_el1, {0}", "isb", in(reg) cpacr | (3 << 20), options(nostack));
        // Disconnect UART0's former Bluetooth TX/RX pins before selecting the
        // header pins. GPIO14/15 ALT0 is PL011, independent of mini-UART clocks.
        let value = read32(GPIO_BASE + 0x0c) & !((7 << 6) | (7 << 9));
        write32(GPIO_BASE + 0x0c, value);
        write32(UART + 0x30, 0);
        let value = read32(GPIO_BASE + 4) & !((7 << 12) | (7 << 15));
        write32(GPIO_BASE + 4, value | (4 << 12) | (4 << 15));
        write32(GPIO_BASE + 0x94, 0);
        delay(150);
        write32(GPIO_BASE + 0x98, (1 << 14) | (1 << 15));
        delay(150);
        write32(GPIO_BASE + 0x98, 0);
        write32(UART + 0x24, 1);
        write32(UART + 0x28, 40);
        write32(UART + 0x2c, (1 << 4) | (3 << 5));
        write32(UART + 0x44, 0x7ff);
        write32(UART + 0x30, 1 | (1 << 8) | (1 << 9));
    }
    present(b"CONDUIT_BOOT_STAGE rpi3-uart-ready\n");
    memory::initialize();
    present(b"CONDUIT_BOOT_STAGE rpi3-memory-ready\n");
    unsafe {
        write32(INTERRUPT + 0x21c, u32::MAX);
        write32(INTERRUPT + 0x220, u32::MAX);
        write32(TIMER, 2);
        write32(INTERRUPT + 0x210, 2);
        // BCM2836 local controller routes peripheral IRQs to core 0.
        write32(0x4000_000c, 0);
    }
}
pub fn present(bytes: &[u8]) {
    for &byte in bytes {
        // Diagnostic output cannot permanently hide a subsequent refusal.
        let mut remaining = 1_000_000;
        while unsafe { read32(UART + 0x18) } & (1 << 5) != 0 {
            if remaining == 0 {
                return;
            }
            remaining -= 1;
            core::hint::spin_loop();
        }
        unsafe {
            write32(UART, u32::from(byte));
        }
    }
}
pub fn read_counter() -> u64 {
    loop {
        let high = unsafe { read32(TIMER + 8) };
        let low = unsafe { read32(TIMER + 4) };
        if high == unsafe { read32(TIMER + 8) } {
            return (u64::from(high) << 32) | u64::from(low);
        }
    }
}
pub fn disable_interrupts() {
    unsafe {
        core::arch::asm!("msr daifset, #2", "isb", options(nostack));
    }
}
pub fn enable_interrupts() {
    unsafe {
        core::arch::asm!("msr daifclr, #2", "isb", options(nostack));
    }
}
fn interrupts_enabled() -> bool {
    let value: u64;
    unsafe {
        core::arch::asm!("mrs {0}, daif", out(reg) value, options(nostack));
    }
    value & (1 << 7) == 0
}
pub fn interruptible_idle() {
    // An IRQ handled just before WFI must not leave Root asleep with no future
    // one-shot deadline. Mask delivery across the pending check and WFI; a
    // physical IRQ still wakes WFI, then restoring delivery services it.
    let enabled = interrupts_enabled();
    disable_interrupts();
    if !PENDING.load(Ordering::Acquire) {
        unsafe {
            core::arch::asm!("wfi", options(nostack));
        }
    }
    if enabled {
        enable_interrupts();
    }
}
pub fn timer_arm() {
    unsafe {
        write32(TIMER, 2);
        write32(TIMER + 0x10, (read_counter() as u32).wrapping_add(1000));
    }
}
pub fn pop_interrupt() -> Option<InterruptFact> {
    if OVERFLOW.swap(false, Ordering::AcqRel) {
        return Some(InterruptFact::Overflow);
    }
    PENDING
        .swap(false, Ordering::AcqRel)
        .then_some(InterruptFact::Timer)
}
#[unsafe(no_mangle)]
extern "C" fn conduitos_board_irq() {
    unsafe {
        write32(TIMER, 2);
    }
    if PENDING.swap(true, Ordering::AcqRel) {
        OVERFLOW.store(true, Ordering::Release);
    }
}
#[unsafe(no_mangle)]
extern "C" fn conduitos_board_exception() -> ! {
    use core::fmt::Write;
    let (esr, far, elr): (u64, u64, u64);
    unsafe {
        core::arch::asm!("mrs {0}, esr_el1", "mrs {1}, far_el1", "mrs {2}, elr_el1", out(reg) esr, out(reg) far, out(reg) elr, options(nostack));
    }
    let mut text = crate::sign_format::FixedText::new();
    let _ = writeln!(
        text,
        "CONDUIT_RPI3_EXCEPTION esr={esr:#x} far={far:#x} elr={elr:#x}"
    );
    present(text.as_bytes());
    disable_interrupts();
    loop {
        unsafe {
            core::arch::asm!("wfe", options(nostack));
        }
    }
}
#[repr(C, align(64))]
struct Message<const N: usize>(UnsafeCell<[u32; N]>);
unsafe impl<const N: usize> Sync for Message<N> {}
static REVISION: Message<7> = Message(UnsafeCell::new([0; 7]));
pub fn firmware_board_revision() -> Option<u32> {
    let words = unsafe { &mut *REVISION.0.get() };
    *words = [28, 0, 0x10002, 4, 0, 0, 0];
    mailbox::call(words).ok()?;
    (words[4] == 0x8000_0004).then_some(words[5])
}
fn release_usb_hub() -> Result<(), &'static str> {
    let HubReset::FirmwareGpio(pin) = BOARD.hub_reset() else {
        return Err("rpi3-hub-reset-profile");
    };
    let mut message = Message(UnsafeCell::new([
        32,
        0,
        0x38041,
        8,
        8,
        128 + u32::from(pin),
        1,
        0,
    ]));
    let words = message.0.get_mut();
    mailbox::call(words)?;
    if words[4] != 0x8000_0008 || words[6] != 1 {
        return Err("rpi3-hub-reset-refused");
    }
    Ok(())
}
fn prepare_audio_pins() {
    let [left, right] = BOARD.audio_pins();
    let fsel = GPIO_BASE + usize::from(left / 10) * 4;
    let (left, right) = (u32::from(left % 10) * 3, u32::from(right % 10) * 3);
    unsafe {
        let value = read32(fsel) & !((7 << left) | (7 << right));
        write32(fsel, value | (4 << left) | (4 << right));
    }
}
fn drain_writes() {
    unsafe {
        core::arch::asm!("dsb sy", options(nostack));
    }
}
fn prepare_mailbox(address: usize, bytes: usize) {
    for line in ((address & !63)..address + bytes).step_by(64) {
        unsafe {
            core::arch::asm!("dc cvac, {0}", in(reg) line, options(nostack));
        }
    }
    drain_writes();
}
fn invalidate_mailbox(address: usize, bytes: usize) {
    for line in ((address & !63)..address + bytes).step_by(64) {
        unsafe {
            core::arch::asm!("dc ivac, {0}", in(reg) line, options(nostack));
        }
    }
    drain_writes();
}
fn delay(cycles: u32) {
    for _ in 0..cycles {
        core::hint::spin_loop();
    }
}
unsafe fn read32(address: usize) -> u32 {
    drain_writes();
    let value = unsafe { core::ptr::read_volatile(address as *const u32) };
    drain_writes();
    value
}
unsafe fn write32(address: usize, value: u32) {
    drain_writes();
    unsafe {
        core::ptr::write_volatile(address as *mut u32, value);
    }
    drain_writes();
}
