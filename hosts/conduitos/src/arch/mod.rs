//! Narrow bootstrap-only machine operations.
//!
//! Shared ConduitOS code depends on this surface. Architecture facts never flow
//! upward into authored meaning or semantic implementation identity.

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::{
    Clock, HidError, HidKeyboardSession, HidProof, Idle, Interrupts, RebootBase, RebootError,
    Serial, Timer, UsbDevice, XhciReady, boot_entropy, deterministic_exit, early_write,
    enumerate_one_at_epoch, enumerate_usb, feature_basis, finish_boot_keyboard, initialize_machine,
    initialize_xhci, local_reboot_base, prepare_boot_keyboard, receive_boot_keyboard,
    receive_first_boot_keyboard_report, retire_removed_device, run_boot_keyboard,
    wait_for_attachment_state,
};

#[cfg(target_arch = "x86_64")]
pub const ARCHITECTURE: &str = "x86_64";
#[cfg(target_arch = "x86")]
mod ia32;
#[cfg(target_arch = "x86")]
pub use ia32::{
    Clock, Idle, InterruptFact, Interrupts, Serial, Timer, disable_interrupts, enable_interrupts,
    initialize_machine, interruptible_idle, pop_interrupt, present, read_counter, timer_arm,
};
#[cfg(target_arch = "x86")]
pub const ARCHITECTURE: &str = "ia32";
#[cfg(target_arch = "aarch64")]
mod aarch64;

#[cfg(target_arch = "aarch64")]
pub use aarch64::{
    Clock, Idle, InterruptFact, Interrupts, Serial, Timer, disable_interrupts, enable_fp_simd,
    enable_interrupts, initialize_machine, install_low_mmio_map, interruptible_idle,
    mmio_table_addresses, pop_interrupt, present, read_counter, timer_arm,
};

#[cfg(target_arch = "aarch64")]
pub const ARCHITECTURE: &str = "aarch64";
#[cfg(target_arch = "riscv64")]
mod riscv64;
#[cfg(target_arch = "riscv64")]
pub use riscv64::{
    Clock, Idle, InterruptFact, Interrupts, Serial, Timer, disable_interrupts, enable_interrupts,
    initialize_machine, interruptible_idle, pop_interrupt, present, read_counter, timer_arm,
};
#[cfg(target_arch = "riscv64")]
pub const ARCHITECTURE: &str = "riscv64";
#[cfg(target_arch = "loongarch64")]
mod loongarch64;
#[cfg(target_arch = "loongarch64")]
pub use loongarch64::{
    Clock, Idle, InterruptFact, Interrupts, Serial, Timer, disable_interrupts, enable_interrupts,
    initialize_machine, interruptible_idle, pop_interrupt, present, read_counter, timer_arm,
};
#[cfg(target_arch = "loongarch64")]
pub const ARCHITECTURE: &str = "loongarch64";
