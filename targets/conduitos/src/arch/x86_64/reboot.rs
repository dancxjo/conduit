//! Minimal local machine-reset Base for the interactive x86_64 profile.
//!
//! Request acceptance is not reboot completion: if the reset command returns,
//! the old boot reports that distinct failure instead of claiming a new boot.

use core::{arch::asm, convert::Infallible};

use super::{
    cpu::{disable_interrupts, enable_interrupts, interrupts_enabled},
    io::{inb, outb},
};

const CONTROLLER_STATUS: u16 = 0x64;
const CONTROLLER_COMMAND: u16 = 0x64;
const INPUT_BUFFER_FULL: u8 = 1 << 1;
const PULSE_CPU_RESET: u8 = 0xfe;
const MAX_COMMAND_POLLS: usize = 65_536;
const RESET_RETURN_SPINS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RebootError {
    ControllerBusy,
    ResetReturned,
}

impl RebootError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ControllerBusy => "machine-reboot-controller-busy",
            Self::ResetReturned => "machine-reboot-returned",
        }
    }
}

/// The admitted local reset mechanism for the interactive x86_64 profile.
///
/// Construction is kept inside the architecture adapter so availability is an
/// explicit Host-composition fact rather than something authored Forms obtain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RebootBase {
    _private: (),
}

impl RebootBase {
    pub(super) const fn available() -> Self {
        Self { _private: () }
    }

    /// Issues exactly one guest reset command after a finite readiness check.
    ///
    /// A successful machine reset never returns. Returning is therefore an
    /// explicit old-boot failure, not evidence that a fresh Boot exists.
    pub fn request(self) -> Result<Infallible, RebootError> {
        let restore_interrupts = interrupts_enabled();
        disable_interrupts();
        let ready = wait_for_command_slot(|| unsafe { inb(CONTROLLER_STATUS) });
        if let Err(error) = ready {
            if restore_interrupts {
                enable_interrupts();
            }
            return Err(error);
        }

        unsafe { outb(CONTROLLER_COMMAND, PULSE_CPU_RESET) };
        for _ in 0..RESET_RETURN_SPINS {
            core::hint::spin_loop();
        }
        if restore_interrupts {
            enable_interrupts();
        }
        Err(RebootError::ResetReturned)
    }
}

pub fn local_reboot_base() -> RebootBase {
    RebootBase::available()
}

fn wait_for_command_slot(mut read_status: impl FnMut() -> u8) -> Result<(), RebootError> {
    for _ in 0..MAX_COMMAND_POLLS {
        if read_status() & INPUT_BUFFER_FULL == 0 {
            return Ok(());
        }
        unsafe { asm!("pause", options(nomem, nostack, preserves_flags)) };
    }
    Err(RebootError::ControllerBusy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_an_immediately_ready_controller() {
        let mut polls = 0;
        assert_eq!(
            wait_for_command_slot(|| {
                polls += 1;
                0
            }),
            Ok(())
        );
        assert_eq!(polls, 1);
    }

    #[test]
    fn readiness_wait_is_finite_and_preserves_busy_failure() {
        let mut polls = 0;
        assert_eq!(
            wait_for_command_slot(|| {
                polls += 1;
                INPUT_BUFFER_FULL
            }),
            Err(RebootError::ControllerBusy)
        );
        assert_eq!(polls, MAX_COMMAND_POLLS);
    }

    #[test]
    fn admits_once_the_controller_becomes_ready() {
        let mut polls = 0;
        assert_eq!(
            wait_for_command_slot(|| {
                polls += 1;
                if polls < 3 { INPUT_BUFFER_FULL } else { 0 }
            }),
            Ok(())
        );
        assert_eq!(polls, 3);
    }

    #[test]
    fn failures_are_machine_readable_and_distinct() {
        assert_eq!(
            RebootError::ControllerBusy.as_str(),
            "machine-reboot-controller-busy"
        );
        assert_eq!(
            RebootError::ResetReturned.as_str(),
            "machine-reboot-returned"
        );
    }
}
