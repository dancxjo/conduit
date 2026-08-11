mod cpu;
mod gdt;
mod hid;
mod idt;
mod io;
mod irq;
mod pic;
mod pit;
mod providers;
mod serial;
mod usb;
mod xhci;

pub use cpu::{boot_entropy, deterministic_exit, feature_basis};
pub use hid::{HidProof, run_boot_keyboard};
pub use providers::{Clock, Idle, Interrupts, Serial, Timer, initialize_machine};
pub use serial::early_write;
pub use usb::{UsbDevice, enumerate_one as enumerate_usb};
pub use xhci::initialize_xhci;

pub const TIMER_IRQ_VECTOR: u8 = 0x20;

#[unsafe(no_mangle)]
extern "C" fn conduitos_exception_handler(vector: u64) -> ! {
    serial::early_write(b"CONDUIT_MACHINE_EXCEPTION vector=");
    serial::write_decimal(vector);
    serial::early_write(b"\n");
    cpu::deterministic_exit(false)
}
