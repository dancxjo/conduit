mod cpu;
mod ftdi_line;
mod gdt;
mod hid;
mod hid_pointer;
mod idt;
mod io;
mod irq;
mod opl2;
mod pc_speaker;
mod pic;
mod pit;
#[cfg(feature = "conduitos-isolation-proof")]
mod protected_execution;
mod providers;
mod reboot;
mod serial;
mod usb;
mod xhci;

pub use cpu::{boot_entropy, deterministic_exit, feature_basis};
pub use ftdi_line::{
    FTDI_PACKET_BYTES, FTDI_PAYLOAD_BYTES, FTDI_TRANSFER_TRBS, FtdiLineError, FtdiLineReady,
    FtdiLineSession, prepare_ftdi_line, start_ftdi_line_session,
};
pub use hid::{
    HidError, HidKeyTransition, HidKeyboardSession, HidProof, finish_boot_keyboard,
    prepare_boot_keyboard, receive_boot_keyboard, receive_first_boot_keyboard_report,
    run_boot_keyboard, start_boot_keyboard_session,
};
pub use hid_pointer::{
    HidPointerError, HidPointerReady, HidPointerSession, prepare_boot_pointer,
    start_pointer_session,
};
pub use opl2::Opl2;
pub use pc_speaker::PcSpeaker;
#[cfg(feature = "conduitos-isolation-proof")]
pub use protected_execution::run_isolation_proof;
pub use providers::{Clock, Idle, Interrupts, Serial, Timer, initialize_machine};

pub const fn pc_speaker_input_hz() -> u64 {
    pc_speaker::PIT_INPUT_HZ
}
pub use reboot::{RebootBase, RebootError, local_reboot_base};
pub use serial::early_write;
pub use usb::{
    UsbDevice, enumerate_attached_at_epochs, enumerate_one as enumerate_usb,
    enumerate_one_at_epoch, retire_removed_device, wait_for_attachment_state,
};
pub use xhci::{XhciReady, initialize_xhci};

pub const TIMER_IRQ_VECTOR: u8 = 0x20;

#[unsafe(no_mangle)]
extern "C" fn conduitos_exception_handler(vector: u64) -> ! {
    serial::early_write(b"CONDUIT_MACHINE_EXCEPTION vector=");
    serial::write_decimal(vector);
    serial::early_write(b"\n");
    cpu::deterministic_exit(false)
}
